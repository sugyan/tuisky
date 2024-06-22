use super::bsky::BskyComponent;
use super::login::LoginComponent;
use super::view::{ViewComponent, ViewEvent};
use super::Component;
use crate::backend::Manager;
use crate::types::Action;
use bsky_sdk::agent::config::Config;
use bsky_sdk::BskyAgent;
use color_eyre::eyre::eyre;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use directories::ProjectDirs;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, BorderType};
use ratatui::Frame;
use serde::{Deserialize, Serialize};
use std::fs::{create_dir_all, File};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;

#[derive(Debug, Default, Serialize, Deserialize)]
struct AppData {
    views: Vec<ViewData>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ViewData {
    agent: Option<Config>,
}

#[derive(Default)]
struct State {
    selected: Option<usize>,
}

pub struct MainComponent {
    views: Vec<Arc<RwLock<ViewComponent>>>,
    state: State,
}

impl MainComponent {
    pub fn new() -> Self {
        Self {
            views: Vec::new(),
            state: State { selected: None },
        }
    }
    pub async fn init_async(&mut self, _rect: Rect) -> Result<()> {
        let columns = 2; // TODO
        if let Ok(appdata) = Self::load().await {
            for (_, view) in (0..columns).zip(appdata.views) {
                if let Some(config) = view.agent {
                    let component = Arc::new(RwLock::new(ViewComponent::Loading));
                    self.views.push(component.clone());
                    tokio::spawn(async move {
                        let Ok(agent) = BskyAgent::builder().config(config).build().await else {
                            return log::error!("failed to build agent from config");
                        };
                        let Ok(mut c) = component.write() else {
                            return log::error!("failed to write component");
                        };
                        *c = ViewComponent::Bsky(Box::new(BskyComponent::new(Manager::new(
                            Arc::new(agent),
                        ))));
                    });
                } else {
                    self.add_login_component();
                }
            }
        } else {
            for _ in 0..columns {
                self.add_login_component();
            }
        }
        if !self.views.is_empty() {
            self.state.selected = Some(0);
        }
        Ok(())
    }
    fn add_login_component(&mut self) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let component = Arc::new(RwLock::new(ViewComponent::Login(Box::new(
            LoginComponent::new(tx.clone()),
        ))));
        self.views.push(component.clone());
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    ViewEvent::Login(config) => {
                        let Ok(agent) = BskyAgent::builder().config(config).build().await else {
                            return log::error!("failed to build agent from config");
                        };
                        let Ok(preferences) = agent.get_preferences(true).await else {
                            return log::error!("failed to get preferences");
                        };
                        agent.configure_labelers_from_preferences(&preferences);
                        if let Ok(mut c) = component.write() {
                            *c = ViewComponent::Bsky(Box::new(BskyComponent::new(Manager::new(
                                Arc::new(agent),
                            ))));
                        }
                    }
                }
            }
        });
    }
    pub async fn save(&self) -> Result<()> {
        let mut appdata = AppData {
            views: Vec::with_capacity(self.views.len()),
        };
        let managers = self
            .views
            .iter()
            .map(|view| view.read().ok().and_then(|v| v.get_manager()))
            .collect::<Vec<_>>();
        for manager in managers {
            let config = if let Some(m) = manager {
                Some(m.agent_config().await)
            } else {
                None
            };
            appdata.views.push(ViewData { agent: config });
        }
        let path = Self::appdata_path()?;
        serde_json::to_writer_pretty(File::create(&path)?, &appdata)?;
        log::info!("saved appdata to: {path:?}");
        Ok(())
    }
    async fn load() -> Result<AppData> {
        let path = Self::appdata_path()?;
        let appdata = serde_json::from_reader::<_, AppData>(File::open(&path)?)?;
        log::info!("loaded appdata from {path:?}");
        Ok(appdata)
    }
    fn appdata_path() -> Result<PathBuf> {
        let data_dir = Self::get_data_dir()?;
        create_dir_all(&data_dir)?;
        Ok(data_dir.join("appdata.json"))
    }
    fn get_data_dir() -> Result<PathBuf> {
        if let Some(proj_dir) = ProjectDirs::from("com", "sugyan", "tuisky") {
            Ok(proj_dir.data_dir().to_path_buf())
        } else {
            Err(eyre!("failed to get project directories"))
        }
    }
}

impl Component for MainComponent {
    fn init(&mut self, rect: Rect) -> Result<()> {
        for view in self.views.iter_mut() {
            if let Ok(mut view) = view.write() {
                view.init(rect)?;
            }
        }
        Ok(())
    }
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        if matches!(
            (key.code, key.modifiers),
            (KeyCode::Char('o'), KeyModifiers::CONTROL)
        ) {
            return Ok(Some(Action::NextFocus));
        } else if let Some(selected) = self.state.selected {
            if let Ok(mut view) = self.views[selected].write() {
                return view.handle_key_events(key);
            }
        }
        Ok(None)
    }
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        // TODO: update non-selected views?
        if matches!(action, Action::NextFocus) {
            self.state.selected = Some(self.state.selected.map_or(0, |s| s + 1) % self.views.len());
        } else if let Some(selected) = self.state.selected {
            if let Ok(mut view) = self.views[selected].write() {
                return view.update(action);
            }
        }
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(self.views.iter().map(|_| Constraint::Fill(1)))
            .split(area);
        for (i, (area, view)) in layout.iter().zip(self.views.iter()).enumerate() {
            let mut block = Block::bordered()
                .title(format!("column {i:02}"))
                .title_alignment(Alignment::Center);
            if self.state.selected == Some(i) {
                block = block.border_type(BorderType::Double)
            }
            if let Ok(mut view) = view.write() {
                view.draw(f, block.inner(*area))?;
            }
            f.render_widget(block, *area);
        }
        Ok(())
    }
}
