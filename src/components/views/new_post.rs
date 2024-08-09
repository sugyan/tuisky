use super::super::modals::types::{Action as ModalAction, Data, EmbedData};
use super::super::modals::{EmbedModalComponent, ModalComponent};
use super::types::{Action, Transition, View};
use super::ViewComponent;
use bsky_sdk::api::types::string::Datetime;
use bsky_sdk::api::types::string::Language;
use bsky_sdk::api::types::Union;
use bsky_sdk::rich_text::RichText;
use bsky_sdk::BskyAgent;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use futures_util::future;
use image::{ImageFormat, ImageReader};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Padding};
use ratatui::{layout::Rect, widgets::Paragraph, Frame};
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::num::NonZeroU64;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::TextArea;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Text,
    Embed,
    Langs,
    Submit,
}

impl Focus {
    fn next(&self) -> Self {
        match self {
            Self::Text => Self::Embed,
            Self::Embed => Self::Langs,
            Self::Langs => Self::Submit,
            Self::Submit => Self::Text,
        }
    }
    fn prev(&self) -> Self {
        match self {
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
    fn submit_post(&self) -> Result<()> {
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
            let embed =
                if let Some(data) = embed_data {
                    let mut handles = Vec::new();
                    for image in data.images {
                        if let Ok(mut file) = File::open(&image.path) {
                            let mut buf = Vec::new();
                            if file.read_to_end(&mut buf).is_err() || buf.len() > 1_000_000 {
                                continue;
                            }
                            if let Ok((width, height)) = ImageReader::with_format(
                                BufReader::new(Cursor::new(&buf)),
                                ImageFormat::from_path(&image.path).unwrap(),
                            )
                            .into_dimensions()
                            {
                                let aspect_ratio = Some(
                                    bsky_sdk::api::app::bsky::embed::images::AspectRatioData {
                                        width: NonZeroU64::new(width.into()).unwrap(),
                                        height: NonZeroU64::new(height.into()).unwrap(),
                                    }
                                    .into(),
                                );
                                let agent = agent.clone();
                                handles.push(tokio::spawn(async move {
                                    agent.api.com.atproto.repo.upload_blob(buf).await.map(
                                        |output| {
                                            bsky_sdk::api::app::bsky::embed::images::ImageData {
                                                alt: image.alt,
                                                aspect_ratio,
                                                image: output.data.blob,
                                            }
                                        },
                                    )
                                }));
                            }
                        }
                    }
                    let mut main =
                        bsky_sdk::api::app::bsky::embed::images::MainData { images: Vec::new() };
                    for image in future::join_all(handles)
                        .await
                        .into_iter()
                        .flatten()
                        .flatten()
                    {
                        main.images.push(image.into());
                    }
                    Some(Union::Refs(
                    bsky_sdk::api::app::bsky::feed::post::RecordEmbedRefs::AppBskyEmbedImagesMain(
                        Box::new(main.into()),
                    ),
                ))
                } else {
                    None
                };
            match agent
                .create_record(bsky_sdk::api::app::bsky::feed::post::RecordData {
                    created_at: Datetime::now(),
                    embed,
                    entities: None,
                    facets: None,
                    labels: None,
                    langs,
                    reply: None,
                    tags: None,
                    text,
                })
                .await
            {
                Ok(output) => {
                    log::info!("Post created: {output:?}");
                    tx.send(Action::Transition(Transition::Pop)).ok();
                }
                Err(e) => {
                    log::error!("failed to create post: {e}");
                }
            }
        });
        Ok(())
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
                    if embed != EmbedData::default() {
                        self.embed = Some(embed);
                    }
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
                self.modals = Some(Box::new(EmbedModalComponent::new()));
                Ok(Some(Action::Render))
            }
            Action::Enter if self.focus == Focus::Submit => {
                self.submit_post()?;
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
            Constraint::Length(1 + self.embed.is_some() as u16),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .areas(area);
        let mut embed_lines = vec![Line::from("+ Embed")];
        if let Some(embed) = &self.embed {
            let mut line = Line::from(format!("  {} images", embed.images.len()));
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
        f.render_widget(self.text.widget(), text);
        f.render_widget(embed_text, embed);
        f.render_widget(self.langs.widget(), langs);
        f.render_widget(submit_line, submit);

        for modal in self.modals.iter_mut() {
            modal.draw(f, area)?;
        }
        Ok(())
    }
}
