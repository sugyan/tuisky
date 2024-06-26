use super::types::Action;
use super::ViewComponent;
use bsky_sdk::api::agent::Session;
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
enum LoginResult {
    Success(Session),
    Failure(String),
}

enum FocusField {
    Identifier,
    Password,
    None,
}

pub struct LoginComponent {
    identifier: TextState<'static>,
    password: TextState<'static>,
    current: FocusField,
    result: Arc<RwLock<Option<LoginResult>>>,
    action_tx: UnboundedSender<Action>,
}

impl LoginComponent {
    pub fn new(action_tx: UnboundedSender<Action>) -> Self {
        Self {
            identifier: TextState::new(),
            password: TextState::new(),
            current: FocusField::Identifier,
            result: Arc::new(RwLock::new(None)),
            action_tx,
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
        let identifier = self.identifier.value().to_string();
        let password = self.password.value().to_string();
        let result = Arc::clone(&self.result);
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
                    if let Ok(mut result) = result.write() {
                        result.replace(LoginResult::Success(session));
                    }
                }
                Err(e) => {
                    log::warn!("login failed: {e}");
                    if let Ok(mut result) = result.write() {
                        result.replace(LoginResult::Failure(e.to_string()));
                    }
                }
            }
        });
        Ok(())
    }
}

impl ViewComponent for LoginComponent {
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        match (key.code, key.modifiers) {
            (KeyCode::Tab, KeyModifiers::NONE) => Ok(Some(Action::NextInput)),
            (KeyCode::BackTab, KeyModifiers::SHIFT) => Ok(Some(Action::PrevInput)),
            _ => {
                if let Some(current) = self.current() {
                    current.handle_key_event(key);
                }
                Ok(if key.code == KeyCode::Enter {
                    Some(Action::Enter)
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
            Action::Enter => {
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
                    LoginResult::Success(session) => Paragraph::new(format!(
                        "Successfully logged in as {}",
                        session.handle.as_ref()
                    ))
                    .style(Style::default().green())
                    .wrap(Wrap::default()),
                    LoginResult::Failure(e) => Paragraph::new(e.as_str())
                        .style(Style::default().red())
                        .wrap(Wrap::default()),
                };
                f.render_widget(paragraph, layout[4]);
            }
        }
        Ok(())
    }
}
