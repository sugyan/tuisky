use super::types::{Action, View};
use super::ViewComponent;
use bsky_sdk::BskyAgent;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, Padding, Paragraph, Wrap};
use ratatui::Frame;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::TextArea;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Identifier,
    Password,
    Submit,
}

impl Focus {
    fn next(&self) -> Self {
        match self {
            Self::Identifier => Self::Password,
            Self::Password => Self::Submit,
            Self::Submit => Self::Identifier,
        }
    }
    fn prev(&self) -> Self {
        match self {
            Self::Identifier => Self::Submit,
            Self::Password => Self::Identifier,
            Self::Submit => Self::Password,
        }
    }
}

pub struct LoginComponent {
    identifier: TextArea<'static>,
    password: TextArea<'static>,
    focus: Focus,
    error_message: Arc<RwLock<Option<String>>>,
    action_tx: UnboundedSender<Action>,
}

impl LoginComponent {
    pub fn new(action_tx: UnboundedSender<Action>) -> Self {
        let mut identifier = TextArea::default();
        identifier.set_block(
            Block::bordered()
                .title("Identifier")
                .border_style(Style::default()),
        );
        identifier.set_cursor_line_style(Style::default());
        let mut password = TextArea::default();
        password.set_mask_char('*');
        password.set_block(
            Block::bordered()
                .title("Password")
                .border_style(Style::default().dim()),
        );
        password.set_cursor_line_style(Style::default());
        password.set_cursor_style(Style::default());
        Self {
            identifier,
            password,
            focus: Focus::Identifier,
            error_message: Arc::new(RwLock::new(None)),
            action_tx,
        }
    }
    fn current_textarea(&mut self) -> Option<&mut TextArea<'static>> {
        match self.focus {
            Focus::Identifier => Some(&mut self.identifier),
            Focus::Password => Some(&mut self.password),
            Focus::Submit => None,
        }
    }
    fn update_focus(&mut self, focus: Focus) {
        if let Some(textarea) = self.current_textarea() {
            textarea.set_cursor_style(Style::default());
            if let Some(block) = textarea.block().cloned() {
                textarea.set_block(block.border_style(Style::default().dim()));
            }
        }
        self.focus = focus;
        if let Some(textarea) = self.current_textarea() {
            textarea.set_cursor_style(Style::default().reversed());
            if let Some(block) = textarea.block().cloned() {
                textarea.set_block(block.border_style(Style::default()));
            }
        }
    }
    fn login(&self) -> Result<()> {
        let identifier = self.identifier.lines().join("");
        let password = self.password.lines().join("");
        let error_message = Arc::clone(&self.error_message);
        let action_tx = self.action_tx.clone();
        tokio::spawn(async move {
            let Ok(agent) = BskyAgent::builder().build().await else {
                return log::error!("failed to build agent");
            };
            match agent.login(identifier, password).await {
                Ok(session) => {
                    log::info!("login succeeded: {session:?}");
                    if let Err(e) = action_tx.send(Action::Login(Box::new(agent))) {
                        log::error!("failed to send login event: {e}");
                    }
                }
                Err(e) => {
                    log::warn!("login failed: {e}");
                    if let Ok(mut message) = error_message.write() {
                        message.replace(e.to_string());
                    }
                }
            }
            if let Err(e) = action_tx.send(Action::Render) {
                log::error!("failed to send render event: {e}");
            }
        });
        Ok(())
    }
}

impl ViewComponent for LoginComponent {
    fn view(&self) -> View {
        View::Login
    }
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        if let Some(textarea) = self.current_textarea() {
            Ok(match (key.code, key.modifiers) {
                (KeyCode::Enter, _) | (KeyCode::Char('m'), KeyModifiers::CONTROL) => {
                    Some(Action::Enter)
                }
                _ => {
                    if textarea.input(key) {
                        Some(Action::Render)
                    } else {
                        None
                    }
                }
            })
        } else {
            Ok(None)
        }
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
            Action::Enter => {
                match self.focus {
                    Focus::Submit => {
                        self.login()?;
                    }
                    _ => {
                        self.update_focus(self.focus.next());
                    }
                }
                Ok(Some(Action::Render))
            }
            _ => Ok(None),
        }
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let block = Block::default().padding(Padding::proportional(2));
        let layout = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(2),
        ])
        .split(block.inner(area));
        let mut submit = Line::from("Submit").blue().centered();
        if self.focus == Focus::Submit {
            submit = submit.reversed();
        }
        f.render_widget(self.identifier.widget(), layout[0]);
        f.render_widget(self.password.widget(), layout[1]);
        f.render_widget(submit, layout[2]);
        if let Ok(message) = self.error_message.read() {
            if let Some(s) = message.as_ref() {
                f.render_widget(
                    Paragraph::new(s.as_str())
                        .style(Style::default().red())
                        .wrap(Wrap::default()),
                    layout[4],
                );
            }
        }
        Ok(())
    }
}
