use super::types::{Action, Transition, View};
use super::ViewComponent;
use bsky_sdk::api::records::{KnownRecord, Record};
use bsky_sdk::api::types::string::Datetime;
use bsky_sdk::api::types::{string::Language, Collection};
use bsky_sdk::rich_text::RichText;
use bsky_sdk::BskyAgent;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Padding};
use ratatui::{layout::Rect, widgets::Paragraph, Frame};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::TextArea;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Text(bool),
    Langs,
    Submit,
}

impl Focus {
    fn next(&self) -> Self {
        match self {
            Self::Text(_) => Self::Langs,
            Self::Langs => Self::Submit,
            Self::Submit => Self::Text(false),
        }
    }
    fn prev(&self) -> Self {
        match self {
            Self::Text(_) => Self::Submit,
            Self::Langs => Self::Text(false),
            Self::Submit => Self::Langs,
        }
    }
}

pub struct NewPostViewComponent {
    action_tx: UnboundedSender<Action>,
    agent: Arc<BskyAgent>,
    textarea: TextArea<'static>,
    langs: TextArea<'static>,
    focus: Focus,
    text_len: usize,
}

impl NewPostViewComponent {
    pub fn new(action_tx: UnboundedSender<Action>, agent: Arc<BskyAgent>) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_block(Block::bordered().title("Text"));
        textarea.set_cursor_line_style(Style::default());
        let mut langs = TextArea::default();
        langs.set_block(Block::bordered().title("Langs").dim());
        langs.set_cursor_line_style(Style::default());
        langs.set_cursor_style(Style::default());
        Self {
            action_tx,
            agent,
            textarea,
            langs,
            focus: Focus::Text(true),
            text_len: 0,
        }
    }
    fn current_textarea(&mut self) -> Option<&mut TextArea<'static>> {
        match self.focus {
            Focus::Text(b) => Some(&mut self.textarea).filter(|_| b),
            Focus::Langs => Some(&mut self.langs),
            Focus::Submit => None,
        }
    }
    fn update_focus(&mut self, focus: Focus) {
        let was_text = self.focus == Focus::Text(true);
        if let Some(curr) = self.current_textarea() {
            curr.set_cursor_style(Style::default());
            if let Some(block) = curr.block() {
                if !was_text {
                    curr.set_block(block.clone().dim());
                }
            }
        } else if self.focus == Focus::Text(false) && focus != Focus::Text(true) {
            if let Some(block) = self.textarea.block() {
                self.textarea.set_block(block.clone().dim());
            }
        }
        self.focus = focus;
        if let Some(curr) = self.current_textarea() {
            curr.set_cursor_style(Style::default().reversed());
            if let Some(block) = curr.block() {
                curr.set_block(block.clone().reset());
            }
        } else if self.focus == Focus::Text(false) {
            if let Some(block) = self.textarea.block() {
                self.textarea.set_block(block.clone().reset());
            }
        }
    }
}

impl ViewComponent for NewPostViewComponent {
    fn view(&self) -> View {
        View::NewPost
    }
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        let focus = self.focus;
        if let Some(textarea) = self.current_textarea() {
            if focus == Focus::Text(true) {
                textarea.input(key);
                self.text_len =
                    RichText::new(self.textarea.lines().join("\n"), None).grapheme_len();
                if let Some(block) = self.textarea.block() {
                    let mut block = block.clone();
                    block = match self.text_len {
                        0 => block.border_style(Color::Reset),
                        1..=300 => block.border_style(Color::Green),
                        _ => block.border_style(Color::Red),
                    };
                    self.textarea.set_block(block);
                }
                return Ok(if key.code == KeyCode::Esc {
                    None
                } else {
                    Some(Action::Render)
                });
            } else if matches!(
                (key.code, key.modifiers),
                (KeyCode::Enter, _) | (KeyCode::Char('m'), KeyModifiers::CONTROL)
            ) {
                return Ok(Some(Action::Enter));
            } else if textarea.input(key) {
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
                return Ok(Some(Action::Render));
            }
        }
        Ok(None)
    }
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::NextItem => {
                self.update_focus(self.focus.next());
                Ok(Some(Action::Render))
            }
            Action::PrevItem => {
                self.update_focus(self.focus.prev());
                Ok(Some(Action::Render))
            }
            Action::Enter if self.focus == Focus::Text(false) => {
                self.update_focus(Focus::Text(true));
                Ok(Some(Action::Render))
            }
            Action::Enter if self.focus == Focus::Submit => {
                let tx = self.action_tx.clone();
                let agent = self.agent.clone();
                let text = self.textarea.lines().join("\n");
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
                    let Some(session) = agent.get_session().await else {
                        return;
                    };
                    let result = agent
                        .api
                        .com
                        .atproto
                        .repo
                        .create_record(
                            bsky_sdk::api::com::atproto::repo::create_record::InputData {
                                collection: bsky_sdk::api::app::bsky::feed::Post::nsid(),
                                record: Record::Known(KnownRecord::AppBskyFeedPost(Box::new(
                                    bsky_sdk::api::app::bsky::feed::post::RecordData {
                                        created_at: Datetime::now(),
                                        embed: None,
                                        entities: None,
                                        facets: None,
                                        labels: None,
                                        langs,
                                        reply: None,
                                        tags: None,
                                        text,
                                    }
                                    .into(),
                                ))),
                                repo: session.data.did.into(),
                                rkey: None,
                                swap_commit: None,
                                validate: None,
                            }
                            .into(),
                        )
                        .await;
                    match result {
                        Ok(output) => {
                            log::info!("Post created: {output:?}");
                            tx.send(Action::Transition(Transition::Pop)).ok();
                        }
                        Err(e) => {
                            log::error!("failed to create post: {e}");
                        }
                    }
                });
                Ok(Some(Action::Render))
            }
            Action::Escape if self.focus == Focus::Text(true) => {
                self.update_focus(Focus::Text(false));
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
        let layout = Layout::vertical([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(8),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);

        let mut submit = Line::from("Post").centered().blue();
        if self.focus == Focus::Submit {
            submit = submit.reversed();
        }
        f.render_widget(
            Paragraph::new("New post").bold().block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Color::Gray)
                    .padding(Padding::horizontal(1)),
            ),
            layout[0],
        );
        f.render_widget(
            Line::from(format!("{} ", 300 - self.text_len as isize))
                .right_aligned()
                .gray(),
            layout[1],
        );
        f.render_widget(self.textarea.widget(), layout[2]);
        f.render_widget(self.langs.widget(), layout[3]);
        f.render_widget(submit, layout[4]);
        Ok(())
    }
}
