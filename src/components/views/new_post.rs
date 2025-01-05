use {
    super::{
        super::modals::{
            types::{Action as ModalAction, Data, EmbedData},
            {EmbedModalComponent, ModalComponent},
        },
        types::{Action, Transition, View},
        ViewComponent,
    },
    bsky_sdk::{
        api::app::bsky::embed::{self, record_with_media},
        api::app::bsky::feed::post::{RecordData, RecordEmbedRefs},
        api::com::atproto::repo::{create_record, strong_ref},
        api::types::string::{Datetime, Language},
        api::types::Union,
        rich_text::RichText,
        BskyAgent,
    },
    color_eyre::Result,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    futures_util::future,
    image::{ImageFormat, ImageReader},
    ratatui::{
        layout::{Constraint, Layout},
        style::{Color, Style, Stylize},
        text::{Line, Text},
        widgets::{Block, Borders, Padding},
        {layout::Rect, widgets::Paragraph, Frame},
    },
    std::{
        fs::File,
        io::{BufReader, Cursor, Read},
        num::NonZeroU64,
        sync::Arc,
    },
    tokio::sync::mpsc::UnboundedSender,
    tui_textarea::TextArea,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    None,
    Text,
    Embed,
    Langs,
    Submit,
}

impl Focus {
    fn next(self) -> Self {
        match self {
            Self::None => Self::Text,
            Self::Text => Self::Embed,
            Self::Embed => Self::Langs,
            Self::Langs => Self::Submit,
            Self::Submit => Self::Text,
        }
    }
    fn prev(self) -> Self {
        match self {
            Self::None => Self::Text,
            Self::Text => Self::Submit,
            Self::Embed => Self::Text,
            Self::Langs => Self::Embed,
            Self::Submit => Self::Langs,
        }
    }
}

pub struct NewPostViewComponent {
    action_tx: UnboundedSender<Action>,
    agent: Arc<BskyAgent>,
    text: TextArea<'static>,
    embed: Option<EmbedData>,
    langs: TextArea<'static>,
    focus: Focus,
    text_len: usize,
    modals: Option<Box<dyn ModalComponent>>,
}

impl NewPostViewComponent {
    pub fn new(action_tx: UnboundedSender<Action>, agent: Arc<BskyAgent>) -> Self {
        let mut text = TextArea::default();
        text.set_block(Block::bordered().title("Text"));
        text.set_cursor_line_style(Style::default());
        let mut langs = TextArea::default();
        langs.set_block(Block::bordered().title("Langs").dim());
        langs.set_cursor_line_style(Style::default());
        langs.set_cursor_style(Style::default());
        Self {
            action_tx,
            agent,
            text,
            embed: None,
            langs,
            focus: Focus::Text,
            text_len: 0,
            modals: None,
        }
    }
    fn current_textarea(&mut self) -> Option<&mut TextArea<'static>> {
        match self.focus {
            Focus::Text => Some(&mut self.text),
            Focus::Langs => Some(&mut self.langs),
            _ => None,
        }
    }
    fn update_focus(&mut self, focus: Focus) {
        if let Some(curr) = self.current_textarea() {
            curr.set_cursor_style(Style::default());
            if let Some(block) = curr.block() {
                curr.set_block(block.clone().dim());
            }
        }
        self.focus = focus;
        if let Some(curr) = self.current_textarea() {
            curr.set_cursor_style(Style::default().reversed());
            if let Some(block) = curr.block() {
                curr.set_block(block.clone().reset());
            }
        }
    }
    fn create_post_record(&self) -> Result<()> {
        let tx = self.action_tx.clone();
        let agent = self.agent.clone();
        let text = self.text.lines().join("\n");
        let embed_data = self.embed.clone();
        let langs = Some(
            self.langs
                .lines()
                .join("")
                .split(',')
                .map(str::trim)
                .filter_map(|s| s.parse::<Language>().ok())
                .collect::<Vec<_>>(),
        )
        .filter(|v| !v.is_empty());
        tokio::spawn(async move {
            match Self::try_create_post_record(&agent, embed_data, langs, text).await {
                Ok(output) => {
                    log::info!("Post created: {output:?}");
                    if let Err(e) = tx.send(Action::Transition(Transition::Pop)) {
                        log::error!("failed to send event: {e}");
                    }
                }
                Err(e) => {
                    // TODO: show error message
                    log::error!("failed to create post: {e}");
                }
            }
        });
        Ok(())
    }
    async fn try_create_post_record(
        agent: &BskyAgent,
        embed_data: Option<EmbedData>,
        langs: Option<Vec<Language>>,
        text: String,
    ) -> Result<create_record::Output> {
        let rich_text = RichText::new_with_detect_facets(text).await?;
        let embed = if let Some(data) = embed_data {
            let mut handles = Vec::new();
            for image in data.images {
                let mut file = File::open(&image.path)?;
                let mut buf = Vec::new();
                file.read_to_end(&mut buf)?;
                if buf.len() > 1_000_000 {
                    log::warn!("image too large: {}", image.path);
                    continue;
                }
                let (width, height) = ImageReader::with_format(
                    BufReader::new(Cursor::new(&buf)),
                    ImageFormat::from_path(&image.path).unwrap(),
                )
                .into_dimensions()?;
                let aspect_ratio = Some(
                    embed::defs::AspectRatioData {
                        width: NonZeroU64::new(width.into()).unwrap(),
                        height: NonZeroU64::new(height.into()).unwrap(),
                    }
                    .into(),
                );
                let agent = agent.clone();
                handles.push(async move {
                    agent
                        .api
                        .com
                        .atproto
                        .repo
                        .upload_blob(buf)
                        .await
                        .map(|output| embed::images::ImageData {
                            alt: image.alt,
                            aspect_ratio,
                            image: output.data.blob,
                        })
                });
            }
            let mut images = embed::images::MainData { images: Vec::new() };
            for image in future::join_all(handles).await {
                images.images.push(image?.into());
            }
            Some(if let Some(record) = data.record {
                let record_data = embed::record::MainData {
                    record: strong_ref::MainData {
                        cid: record.data.cid,
                        uri: record.data.uri,
                    }
                    .into(),
                };
                if images.images.is_empty() {
                    Union::Refs(RecordEmbedRefs::AppBskyEmbedRecordMain(Box::new(
                        record_data.into(),
                    )))
                } else {
                    Union::Refs(RecordEmbedRefs::AppBskyEmbedRecordWithMediaMain(Box::new(
                        embed::record_with_media::MainData {
                            media: Union::Refs(
                                record_with_media::MainMediaRefs::AppBskyEmbedImagesMain(Box::new(
                                    images.into(),
                                )),
                            ),
                            record: record_data.into(),
                        }
                        .into(),
                    )))
                }
            } else {
                Union::Refs(RecordEmbedRefs::AppBskyEmbedImagesMain(Box::new(
                    images.into(),
                )))
            })
        } else {
            None
        };
        Ok(agent
            .create_record(RecordData {
                created_at: Datetime::now(),
                embed,
                entities: None,
                facets: rich_text.facets,
                labels: None,
                langs,
                reply: None,
                tags: None,
                text: rich_text.text,
            })
            .await?)
    }
}

impl ViewComponent for NewPostViewComponent {
    fn view(&self) -> View {
        View::NewPost
    }
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        if let Some(modal) = self.modals.as_mut() {
            return Ok(match modal.handle_key_events(key)? {
                Some(ModalAction::Render) => Some(Action::Render),
                _ => None,
            });
        }
        let focus = self.focus;
        if let Some(textarea) = self.current_textarea() {
            if focus == Focus::Text {
                let cursor = textarea.cursor();
                let result = textarea.input(key) || textarea.cursor() != cursor;
                self.text_len = RichText::new(self.text.lines().join("\n"), None).grapheme_len();
                if let Some(block) = self.text.block() {
                    let mut block = block.clone();
                    block = match self.text_len {
                        0 => block.border_style(Color::Reset),
                        1..=300 => block.border_style(Color::Green),
                        _ => block.border_style(Color::Red),
                    };
                    self.text.set_block(block);
                }
                return Ok(if result { Some(Action::Render) } else { None });
            } else if matches!(
                (key.code, key.modifiers),
                (KeyCode::Enter, _) | (KeyCode::Char('m'), KeyModifiers::CONTROL)
            ) {
                return Ok(Some(Action::Enter));
            } else {
                let cursor = textarea.cursor();
                return Ok(if textarea.input(key) {
                    if self.focus == Focus::Langs {
                        if let Some(block) = self.langs.block() {
                            let mut block = block.clone();
                            if self
                                .langs
                                .lines()
                                .join("")
                                .split(',')
                                .map(str::trim)
                                .all(|s| s.parse::<Language>().is_ok())
                            {
                                block = block.border_style(Color::Green);
                            } else {
                                block = block.border_style(Color::Red);
                            }
                            self.langs.set_block(block);
                        }
                    }
                    Some(Action::Render)
                } else if textarea.cursor() != cursor {
                    Some(Action::Render)
                } else {
                    None
                });
            }
        }
        Ok(None)
    }
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        if let Some(modal) = self.modals.as_mut() {
            return Ok(match modal.update(action)? {
                Some(ModalAction::Ok(Data::Embed(embed))) => {
                    self.embed = if embed == EmbedData::default() {
                        None
                    } else {
                        Some(embed)
                    };
                    self.modals = None;
                    Some(Action::Render)
                }
                Some(ModalAction::Cancel) => {
                    self.modals = None;
                    Some(Action::Render)
                }
                Some(ModalAction::Render) => Some(Action::Render),
                _ => None,
            });
        }
        match action {
            Action::NextItem => {
                self.update_focus(self.focus.next());
                Ok(Some(Action::Render))
            }
            Action::PrevItem => {
                self.update_focus(self.focus.prev());
                Ok(Some(Action::Render))
            }
            Action::Enter if self.focus == Focus::Embed => {
                self.modals = Some(Box::new(EmbedModalComponent::new(
                    self.action_tx.clone(),
                    self.embed.clone(),
                )));
                Ok(Some(Action::Render))
            }
            Action::Enter if self.focus == Focus::Submit => {
                self.focus = Focus::None;
                self.create_post_record()?;
                Ok(Some(Action::Render))
            }
            Action::Back => {
                // TODO: confirm to discard the draft
                Ok(Some(Action::Transition(Transition::Pop)))
            }
            Action::Transition(_) => Ok(Some(action)),
            _ => Ok(None),
        }
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let [paragraph, text_len, text, embed, langs, submit] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(8),
            Constraint::Length(1 + u16::from(self.embed.is_some())),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .areas(area);
        let mut embed_lines = vec![Line::from("+ Embed")];
        if let Some(embed) = &self.embed {
            let mut line = Line::from(match (embed.record.is_some(), embed.images.len()) {
                (true, 0) => "  a record".into(),
                (true, 1) => "  a record with 1 image".into(),
                (true, len) => format!("  a record with {len} images"),
                (false, len) => format!("  {len} images"),
            });
            if self.focus != Focus::Embed {
                line = line.yellow();
            }
            embed_lines.push(line);
        }
        let mut embed_text = Text::from(embed_lines);
        if self.focus == Focus::Embed {
            embed_text = embed_text.reversed();
        }
        let mut submit_line = Line::from("Post").centered().blue();
        if self.focus == Focus::Submit {
            submit_line = submit_line.reversed();
        }
        f.render_widget(
            Paragraph::new("New post").bold().block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Color::Gray)
                    .padding(Padding::horizontal(1)),
            ),
            paragraph,
        );
        f.render_widget(
            Line::from(format!("{} ", 300 - self.text_len as isize))
                .right_aligned()
                .gray(),
            text_len,
        );
        f.render_widget(&self.text, text);
        f.render_widget(embed_text, embed);
        f.render_widget(&self.langs, langs);
        f.render_widget(submit_line, submit);

        if let Some(modal) = &mut self.modals {
            modal.draw(f, area)?;
        }
        Ok(())
    }
}
