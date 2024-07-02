use super::column::ColumnComponent;
use super::Component;
use crate::types::Action;
use crate::utils::get_data_dir;
use bsky_sdk::agent::config::Config;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
    num_columns: usize,
    columns: Vec<ColumnComponent>,
    state: State,
    action_tx: UnboundedSender<Action>,
}

impl MainComponent {
    pub fn new(num_columns: usize, action_tx: UnboundedSender<Action>) -> Self {
        Self {
            num_columns,
            columns: Vec::new(),
            state: State { selected: None },
            action_tx,
        }
    }
    pub async fn save(&self) -> Result<()> {
        let mut appdata = AppData {
            views: Vec::with_capacity(self.columns.len()),
        };
        for view in &self.columns {
            let config = if let Some(m) = &view.watcher {
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
        let data_dir = get_data_dir()?;
        create_dir_all(&data_dir)?;
        Ok(data_dir.join("appdata.json"))
    }
}

impl Component for MainComponent {
    fn init(&mut self, rect: Rect) -> Result<()> {
        let appdata = if let Ok(appdata) = Self::load() {
            appdata
        } else {
            log::warn!("failed to load appdata, using default");
            AppData::default()
        };
        for i in 0..self.num_columns {
            let action_tx = self.action_tx.clone();
            let mut column = ColumnComponent::new(action_tx.clone());
            if let Some(config) = appdata.views.get(i).and_then(|view| view.agent.as_ref()) {
                column.init_with_config(config)?;
            } else {
                column.init(rect)?;
            }
            self.columns.push(column);
        }
        if !self.columns.is_empty() {
            self.state.selected = Some(0);
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
        match action {
            Action::NextFocus => {
                self.state.selected =
                    Some(self.state.selected.map_or(0, |s| s + 1) % self.columns.len());
                return Ok(Some(Action::Render));
            }
            _ => {
                for column in self.columns.iter_mut() {
                    if let Some(action) = column.update(action.clone())? {
                        return Ok(Some(action));
                    }
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
