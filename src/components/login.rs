use super::view::ViewEvent;
use super::Component;
use crate::types::Action;
use bsky_sdk::agent::config::Config;
use bsky_sdk::BskyAgent;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Wrap};
use ratatui::Frame;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::{self, UnboundedSender};
use tui_prompts::{State, TextPrompt, TextRenderStyle, TextState};

#[derive(Debug)]
enum LoginEvent {
    Success(Config),
    Failure(String),
}

enum FocusField {
    Identifier,
    Password,
    None,
}

pub struct LoginComponent<'a> {
    identifier: TextState<'a>,
    password: TextState<'a>,
    current: FocusField,
    result: Arc<RwLock<Option<LoginEvent>>>,
    event_handler: Option<UnboundedSender<ViewEvent>>,
}

impl<'a> LoginComponent<'a> {
    pub fn new() -> Self {
        Self {
            identifier: TextState::new(),
            password: TextState::new(),
            current: FocusField::Identifier,
            result: Arc::new(RwLock::new(None)),
            event_handler: None,
        }
    }
    pub fn register_view_event_handler(&mut self, tx: UnboundedSender<ViewEvent>) {
        self.event_handler = Some(tx);
    }
    fn current(&mut self) -> Option<&mut TextState<'a>> {
        match self.current {
            FocusField::Identifier => Some(&mut self.identifier),
            FocusField::Password => Some(&mut self.password),
            FocusField::None => None,
        }
    }
    fn login(&self) -> Result<()> {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let identifier = self.identifier.value().to_string();
        let password = self.password.value().to_string();
        let result = Arc::clone(&self.result);
        if let Some(event_handler) = &self.event_handler {
            let event_tx = event_handler.clone();
            tokio::spawn(async move {
                while let Some(event) = rx.recv().await {
                    match &event {
                        LoginEvent::Success(config) => {
                            log::info!("login succeeded: {config:?}");
                            event_tx
                                .send(ViewEvent::Login(config.clone()))
                                .expect("failed to send login event")
                        }
                        LoginEvent::Failure(e) => {
                            log::warn!("login failed: {e}");
                        }
                    }
                    if let Ok(mut result) = result.write() {
                        result.replace(event);
                    }
                }
            });
            tokio::spawn(async move { Self::async_login(&identifier, &password, tx).await });
        }
        Ok(())
    }
    async fn async_login(
        identifier: &str,
        password: &str,
        tx: UnboundedSender<LoginEvent>,
    ) -> Result<()> {
        let agent = BskyAgent::builder().build().await?;
        match agent.login(identifier, password).await {
            Ok(_) => tx.send(LoginEvent::Success(agent.to_config().await))?,
            Err(e) => {
                tx.send(LoginEvent::Failure(e.to_string()))?;
            }
        }
        Ok(())
    }
}

impl Component for LoginComponent<'_> {
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
                    LoginEvent::Success(config) => Paragraph::new(format!(
                        "Successfully logged in as {}",
                        config.session.as_ref().unwrap().data.handle.as_ref()
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
