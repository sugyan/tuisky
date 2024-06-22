use super::{bsky::BskyComponent, login::LoginComponent, Component};
use crate::{backend::Manager, types::Action};
use bsky_sdk::agent::config::Config;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Rect};
use ratatui::widgets::{Block, Padding, Paragraph};
use ratatui::Frame;
use std::sync::Arc;

#[derive(Debug)]
pub enum ViewEvent {
    Login(Config),
}

pub enum ViewComponent {
    None,
    Loading,
    Login(Box<LoginComponent>),
    Bsky(Box<BskyComponent>),
}

impl ViewComponent {
    pub fn get_manager(&self) -> Option<Arc<Manager>> {
        match self {
            Self::Bsky(bsky) => Some(Arc::clone(&bsky.manager)),
            _ => None,
        }
    }
}

impl Component for ViewComponent {
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        match self {
            Self::None | Self::Loading => {}
            Self::Login(ref mut login) => {
                return login.handle_key_events(key);
            }
            Self::Bsky(_) => {
                // TODO
            }
        }
        match (key.code, key.modifiers) {
            (KeyCode::Char('n'), KeyModifiers::CONTROL) => return Ok(Some(Action::NextItem)),
            (KeyCode::Char('p'), KeyModifiers::CONTROL) => return Ok(Some(Action::PrevItem)),
            (KeyCode::Down, KeyModifiers::NONE) => return Ok(Some(Action::NextItem)),
            (KeyCode::Up, KeyModifiers::NONE) => return Ok(Some(Action::PrevItem)),
            (KeyCode::Char('o'), KeyModifiers::CONTROL) => return Ok(Some(Action::NextFocus)),
            _ => {}
        }
        Ok(None)
    }
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match self {
            Self::None | Self::Loading => {}
            Self::Login(ref mut login) => match action {
                Action::NextInput | Action::PrevInput | Action::Submit => {
                    return login.update(action);
                }
                _ => {}
            },
            Self::Bsky(_) => {
                // TODO
            }
        }
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        match self {
            Self::None => {}
            Self::Loading => {
                f.render_widget(
                    Paragraph::new("Loading...")
                        .alignment(Alignment::Center)
                        .block(Block::default().padding(Padding::proportional(2))),
                    area,
                );
            }
            Self::Login(login) => {
                login.draw(f, area)?;
            }
            Self::Bsky(bsky) => {
                bsky.draw(f, area)?;
            }
        }
        Ok(())
    }
}
