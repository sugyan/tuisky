use super::Component;
use crate::types::{Action, IdType};
use atrium_api::agent::Session;
use bsky_sdk::BskyAgent;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Wrap};
use ratatui::Frame;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::UnboundedSender;
use tui_prompts::{State, TextPrompt, TextRenderStyle, TextState};

#[derive(Debug)]
enum LoginEvent {
    Success(Session),
    Failure(String),
}

enum FocusField {
    Identifier,
    Password,
    None,
}

pub struct LoginComponent {
    id: IdType,
    identifier: TextState<'static>,
    password: TextState<'static>,
    current: FocusField,
    result: Arc<RwLock<Option<LoginEvent>>>,
    action_tx: UnboundedSender<Action>,
}

impl LoginComponent {
    pub fn new(id: IdType, action_tx: UnboundedSender<Action>) -> Self {
        Self {
            id,
            identifier: TextState::new(),
            password: TextState::new(),
            current: FocusField::Identifier,
            result: Arc::new(RwLock::new(None)),
            action_tx, // event_handler,
        }
    }
    fn current(&mut self) -> Option<&mut TextState<'static>> {
        match self.current {
            FocusField::Identifier => Some(&mut self.identifier),
            FocusField::Password => Some(&mut self.password),
            FocusField::None => None,
        }
    }
    fn login(&self) -> Result<()> {
        let id = self.id;
        let identifier = self.identifier.value().to_string();
        let password = self.password.value().to_string();
        let result = Arc::clone(&self.result);
        let action_tx = self.action_tx.clone();
        tokio::spawn(async move {
            let agent = BskyAgent::builder()
                .build()
                .await
                .expect("failed to build agent");
            match agent.login(identifier, password).await {
                Ok(session) => {
                    log::info!("login succeeded: {session:?}");
                    action_tx
                        .send(Action::Login((id, Box::new(agent))))
                        .expect("failed to send login event");
                    if let Ok(mut result) = result.write() {
                        result.replace(LoginEvent::Success(session));
                    }
                }
                Err(e) => {
                    log::warn!("login failed: {e}");
                    if let Ok(mut result) = result.write() {
                        result.replace(LoginEvent::Failure(e.to_string()));
                    }
                }
            }
        });
        Ok(())
    }
}

impl Component for LoginComponent {
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        match (key.code, key.modifiers) {
            (KeyCode::Tab, KeyModifiers::NONE) => Ok(Some(Action::NextInput)),
            (KeyCode::BackTab, KeyModifiers::SHIFT) => Ok(Some(Action::PrevInput)),
            _ => {
                if let Some(current) = self.current() {
                    current.handle_key_event(key);
                }
                Ok(if key.code == KeyCode::Enter {
                    Some(Action::Submit)
                } else {
                    None
                })
            }
        }
    }
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::NextInput | Action::PrevInput => {
                self.current = match self.current {
                    FocusField::Identifier => FocusField::Password,
                    FocusField::Password => FocusField::Identifier,
                    FocusField::None => FocusField::Identifier,
                };
            }
            Action::Submit => {
                if self.identifier.is_finished() && self.password.is_finished() {
                    self.current = FocusField::None;
                    self.login()?;
                } else {
                    return Ok(Some(Action::NextInput));
                }
            }
            _ => {}
        }
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let block = Block::default().padding(Padding::proportional(2));
        let block_padding = Block::default().padding(Padding::bottom(1));
        let block_border = Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::Gray).dim());
        let layout = Layout::vertical([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(2),
        ])
        .split(block.inner(area));
        let mut identifier = TextPrompt::from("Identifier").with_block(block_padding.clone());
        let mut password = TextPrompt::from("Password  ")
            .with_render_style(TextRenderStyle::Password)
            .with_block(block_padding.clone());
        match self.current {
            FocusField::Identifier => identifier = identifier.with_block(block_border.clone()),
            FocusField::Password => password = password.with_block(block_border.clone()),
            _ => {}
        }
        f.render_stateful_widget(identifier, layout[0], &mut self.identifier);
        f.render_stateful_widget(password, layout[2], &mut self.password);
        if let Ok(result) = self.result.read() {
            if let Some(event) = result.as_ref() {
                let paragraph = match event {
                    LoginEvent::Success(session) => Paragraph::new(format!(
                        "Successfully logged in as {}",
                        session.handle.as_ref()
                    ))
                    .style(Style::default().green())
                    .wrap(Wrap::default()),
                    LoginEvent::Failure(e) => Paragraph::new(e.as_str())
                        .style(Style::default().red())
                        .wrap(Wrap::default()),
                };
                f.render_widget(paragraph, layout[4]);
            }
        }
        Ok(())
    }
}
