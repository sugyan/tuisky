use super::{login::LoginComponent, Component};
use crate::{backend::Views, types::Action};
use bsky_sdk::{agent::config::Config, BskyAgent};
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{layout::Rect, Frame};
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum ViewEvent {
    Login(Config),
}

pub enum ViewComponent<'a> {
    None,
    Login(Box<LoginComponent<'a>>),
    Views(Box<Views>),
}

impl<'a> Component for ViewComponent<'a> {
    fn init(&mut self, _rect: Rect) -> Result<()> {
        match self {
            Self::None => {}
            Self::Login(ref mut login) => {
                let (tx, mut rx) = mpsc::unbounded_channel();
                login.register_view_event_handler(tx);
                tokio::spawn(async move {
                    while let Some(event) = rx.recv().await {
                        match event {
                            ViewEvent::Login(config) => {
                                let Ok(agent) = BskyAgent::builder().config(config).build().await
                                else {
                                    return log::error!("failed to build agent from config");
                                };
                                let Ok(preferences) = agent.get_preferences(true).await else {
                                    return log::error!("failed to get preferences");
                                };
                                agent.configure_labelers_from_preferences(&preferences);
                                log::info!("Done");
                            }
                        }
                    }
                });
            }
            Self::Views(_) => {
                // TODO
            }
        }
        Ok(())
    }
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        match self {
            Self::None => {}
            Self::Login(ref mut login) => {
                return login.handle_key_events(key);
            }
            Self::Views(_) => {
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
        if matches!(action, Action::Tick | Action::Render) {
            return Ok(None);
        }
        match self {
            Self::None => {}
            Self::Login(ref mut login) => match action {
                Action::NextInput | Action::PrevInput | Action::Submit => {
                    return login.update(action);
                }
                Action::Login(config) => {
                    // log::debug!("Login: {config:?}");
                    // let agent = tokio::runtime::Runtime::new()?.block_on(async move {
                    //     log::debug!("create agent from config");
                    //     let result = BskyAgent::builder().config(config).build().await;
                    //     log::debug!("return result");
                    //     result
                    // });
                    // log::debug!("Agent: {:?}", agent.is_ok());
                }
                _ => {}
            },
            Self::Views(_) => {
                // TODO
            }
        }
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        // if let Some(_current) = self.views.current_mut() {
        //     // TODO
        // } else {
        //     self.login.draw(f, area)?;
        // }
        match self {
            Self::None => {}
            Self::Login(ref mut login) => {
                login.draw(f, area)?;
            }
            Self::Views(_) => {
                // TODO
            }
        }
        Ok(())
    }
}
