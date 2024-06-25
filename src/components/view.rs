use super::views::root::RootComponent;
use super::Component;
use crate::backend::Manager;
use crate::types::{Action, IdType, View, ViewData};
use bsky_sdk::{agent::config::Config, BskyAgent};
use color_eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

static COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct ViewComponent {
    pub id: IdType,
    pub manager: Option<Manager>,
    pub component: Option<Box<dyn Component>>,
    action_tx: UnboundedSender<Action>,
}

impl ViewComponent {
    pub fn new(action_tx: UnboundedSender<Action>) -> Self {
        Self {
            id: COUNTER.fetch_add(1, Ordering::SeqCst),
            manager: None,
            component: None,
            action_tx,
        }
    }
    pub fn title(&self) -> String {
        format!("id: {}", self.id)
    }
    pub fn build_agent(&self, config: &Config) {
        let config = config.clone();
        let id = self.id;
        let tx = self.action_tx.clone();
        tokio::spawn(async move {
            let Ok(agent) = BskyAgent::builder().config(config).build().await else {
                return log::error!("failed to build agent from config");
            };
            if let Err(e) = tx.send(Action::Login((id, Box::new(agent)))) {
                log::error!("failed to send transition action: {e}");
            }
        });
    }
}

impl Component for ViewComponent {
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        if let Some(component) = self.component.as_mut() {
            return component.handle_key_events(key);
        }
        Ok(None)
    }
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Login((_, agent)) => {
                let manager = Manager::new(Arc::new(*agent));
                let id = self.id;
                let tx = self.action_tx.clone();
                let mut rx = manager.data.preferences.subscribe();
                tokio::spawn(async move {
                    while rx.changed().await.is_ok() {
                        let preferences = rx.borrow_and_update();
                        if let Err(e) = tx.send(Action::Updated((
                            id,
                            Box::new(ViewData::Preferences(Box::new(preferences.clone()))),
                        ))) {
                            log::error!("failed to send preferences action: {e}");
                        }
                    }
                });
                manager.start();
                self.manager = Some(manager);
                return Ok(Some(Action::Transition((id, View::Root))));
            }
            Action::Transition((_, view)) => {
                if matches!(view, View::Root) {
                    self.component = Some(Box::new(RootComponent::new()));
                }
            }
            _ => {
                if let Some(component) = self.component.as_mut() {
                    return component.update(action);
                }
            }
        }
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        if let Some(component) = self.component.as_mut() {
            component.draw(f, area)
        } else {
            Ok(())
        }
    }
}
