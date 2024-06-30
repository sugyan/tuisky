use super::views::feed::FeedViewComponent;
use super::views::login::LoginComponent;
use super::views::root::RootComponent;
use super::views::types::{Action as ViewAction, Transition, View};
use super::views::ViewComponent;
use super::Component;
use crate::backend::Watcher;
use crate::types::{Action, IdType};
use bsky_sdk::agent::config::Config;
use bsky_sdk::api::agent::Session;
use bsky_sdk::BskyAgent;
use color_eyre::{eyre, Result};
use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::{self, UnboundedSender};

static COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct ColumnComponent {
    pub id: IdType,
    pub watcher: Option<Arc<Watcher>>,
    pub views: Vec<Box<dyn ViewComponent>>,
    session: Arc<RwLock<Option<Session>>>,
    action_tx: UnboundedSender<Action>,
    view_tx: UnboundedSender<ViewAction>,
}

impl ColumnComponent {
    pub fn new(action_tx: UnboundedSender<Action>) -> Self {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let (view_tx, mut view_rx) = mpsc::unbounded_channel();
        let tx = action_tx.clone();
        tokio::spawn(async move {
            while let Some(action) = view_rx.recv().await {
                match action {
                    ViewAction::Login(agent) => {
                        if let Err(e) = tx.send(Action::Login((id, agent))) {
                            log::error!("failed to send login action: {e}");
                        }
                    }
                    _ => {
                        if let Err(e) = tx.send(Action::View((id, action))) {
                            log::error!("failed to send view action: {e}");
                        }
                    }
                }
            }
        });
        Self {
            id,
            watcher: None,
            views: Vec::new(),
            session: Arc::new(RwLock::new(None)),
            action_tx,
            view_tx,
        }
    }
    pub fn init_with_config(&mut self, config: &Config) -> Result<()> {
        let config = config.clone();
        let (id, tx) = (self.id, self.action_tx.clone());
        tokio::spawn(async move {
            let Ok(agent) = BskyAgent::builder().config(config).build().await else {
                return log::error!("failed to build agent from config");
            };
            if let Err(e) = tx.send(Action::Login((id, Box::new(agent)))) {
                log::error!("failed to send transition action: {e}");
            }
        });
        Ok(())
    }
    pub fn title(&self) -> String {
        if let Some(session) = self.session.read().ok().as_ref().and_then(|s| s.as_ref()) {
            format!(" {} ", session.handle.as_str())
        } else {
            format!(" id: {} ", self.id)
        }
    }
    fn transition(&mut self, transition: &Transition) -> Result<Option<Action>> {
        match transition {
            Transition::Push(view) => {
                if let Some(current) = self.views.last_mut() {
                    current.deactivate()?;
                }
                let mut next = self.view(view)?;
                next.as_mut().activate()?;
                self.views.push(next);
            }
            Transition::Pop => {
                if let Some(mut view) = self.views.pop() {
                    view.deactivate()?;
                }
                if let Some(current) = self.views.last_mut() {
                    current.activate()?;
                }
            }
            Transition::Replace(view) => {
                if let Some(mut current) = self.views.pop() {
                    current.deactivate()?;
                }
                let mut next = self.view(view)?;
                next.as_mut().activate()?;
                self.views.push(next);
            }
        }
        Ok(Some(Action::Render))
    }
    fn view(&self, view: &View) -> Result<Box<dyn ViewComponent>> {
        let watcher = self
            .watcher
            .as_ref()
            .ok_or_else(|| eyre::eyre!("watcher not initialized"))?;
        Ok(match view {
            View::Root => Box::new(RootComponent::new(self.view_tx.clone(), watcher.clone())),
            View::Feed(feed) => Box::new(FeedViewComponent::new(
                self.view_tx.clone(),
                watcher.clone(),
                feed.as_ref().clone(),
            )),
        })
    }
}

impl Component for ColumnComponent {
    fn init(&mut self, _area: Rect) -> Result<()> {
        self.views = vec![Box::new(LoginComponent::new(self.view_tx.clone()))];
        Ok(())
    }
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        if let Some(view) = self.views.last_mut() {
            view.handle_key_events(key)
                .map(|action| action.map(|a| Action::View((self.id, a))))
        } else {
            Ok(None)
        }
    }
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::View((id, view_action)) if id == self.id => {
                if let ViewAction::Render = view_action {
                    return Ok(Some(Action::Render));
                }
                return if let Some(view) = self.views.last_mut() {
                    let result = view.update(view_action);
                    match &result {
                        Ok(Some(ViewAction::Logout)) => {
                            if let Ok(mut session) = self.session.write() {
                                session.take();
                            }
                            if let Some(watcher) = self.watcher.take() {
                                watcher.stop();
                            }
                            self.views = vec![Box::new(LoginComponent::new(self.view_tx.clone()))];
                            return Ok(Some(Action::Render));
                        }
                        Ok(Some(ViewAction::Transition(transition))) => {
                            return self.transition(transition);
                        }
                        _ => {}
                    }
                    result.map(|action| action.map(|a| Action::View((self.id, a))))
                } else {
                    Ok(None)
                };
            }
            Action::Login((id, agent)) if id == self.id => {
                {
                    let agent = agent.clone();
                    let session = self.session.clone();
                    tokio::spawn(async move {
                        if let Some(output) = agent.get_session().await {
                            if let Ok(mut session) = session.write() {
                                session.replace(output);
                            }
                        }
                    });
                }
                self.watcher = Some(Arc::new(Watcher::new(Arc::new(*agent))));
                return self.transition(&Transition::Replace(Box::new(View::Root)));
            }
            _ => {}
        }
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        if let Some(view) = self.views.last_mut() {
            view.draw(f, area)
        } else {
            Ok(())
        }
    }
}