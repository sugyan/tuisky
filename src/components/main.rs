use super::login::LoginComponent;
use super::view::ViewComponent;
use super::Component;
use crate::types::Action;
use bsky_sdk::agent::config::Config;
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
use tokio::sync::mpsc::UnboundedSender;

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
    columns: Vec<ViewComponent>,
    state: State,
    action_tx: Option<UnboundedSender<Action>>,
}

impl MainComponent {
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            state: State { selected: None },
            action_tx: None,
        }
    }
    pub async fn save(&self) -> Result<()> {
        let mut appdata = AppData {
            views: Vec::with_capacity(self.columns.len()),
        };
        for view in &self.columns {
            let config = if let Some(m) = &view.manager {
                Some(m.get_agent_config().await)
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
    fn load() -> Result<AppData> {
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
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.action_tx = Some(tx);
        Ok(())
    }
    fn init(&mut self, rect: Rect) -> Result<()> {
        let columns = 2; // TODO
        let appdata = if let Ok(appdata) = Self::load() {
            appdata
        } else {
            log::warn!("failed to load appdata, using default");
            AppData::default()
        };
        for i in 0..columns {
            let action_tx = self
                .action_tx
                .clone()
                .ok_or_else(|| eyre!("failed to get action tx"))?;
            let mut component = ViewComponent::new(action_tx.clone());
            component.component = Some(Box::new(LoginComponent::new(component.id, action_tx)));
            if let Some(view) = appdata.views.get(i) {
                if let Some(config) = &view.agent {
                    component.build_agent(config);
                }
            }
            self.columns.push(component);
        }
        if !self.columns.is_empty() {
            self.state.selected = Some(0);
        }
        for view in self.columns.iter_mut() {
            view.init(rect)?;
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
            return self.columns[selected].handle_key_events(key);
        }
        Ok(None)
    }
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        // TODO: update non-selected views?
        match action {
            Action::NextFocus => {
                self.state.selected =
                    Some(self.state.selected.map_or(0, |s| s + 1) % self.columns.len());
            }
            Action::Login((id, _)) | Action::Transition((id, _)) => {
                if let Some(view) = self.columns.iter_mut().find(|v| v.id == id) {
                    return view.update(action);
                }
            }
            _ => {
                if let Some(selected) = self.state.selected {
                    return self.columns[selected].update(action);
                }
            }
        }
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(self.columns.iter().map(|_| Constraint::Fill(1)))
            .split(area);
        for (i, (area, view)) in layout.iter().zip(self.columns.iter_mut()).enumerate() {
            let mut block = Block::bordered()
                .title(view.title())
                .title_alignment(Alignment::Center);
            if self.state.selected == Some(i) {
                block = block.border_type(BorderType::Double)
            }
            view.draw(f, block.inner(*area))?;
            f.render_widget(block, *area);
        }
        Ok(())
    }
}
