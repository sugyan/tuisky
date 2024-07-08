use super::column::ColumnComponent;
use super::Component;
use crate::config::Config;
use crate::types::Action;
use crate::utils::get_data_dir;
use bsky_sdk::agent::config::Config as AgentConfig;
use color_eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Style, Stylize};
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
    agent: Option<AgentConfig>,
}

#[derive(Default)]
struct State {
    selected: Option<usize>,
}

pub struct MainComponent {
    config: Config,
    action_tx: UnboundedSender<Action>,
    columns: Vec<ColumnComponent>,
    state: State,
}

impl MainComponent {
    pub fn new(config: Config, action_tx: UnboundedSender<Action>) -> Self {
        Self {
            config,
            action_tx,
            columns: Vec::new(),
            state: State { selected: None },
        }
    }
    pub async fn save(&self) -> Result<()> {
        let mut appdata = AppData {
            views: Vec::with_capacity(self.columns.len()),
        };
        for view in &self.columns {
            let config = if let Some(w) = &view.watcher {
                Some(w.agent.to_config().await)
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

        let auto_num = usize::from(rect.width) / 80;
        let num_columns = self
            .config
            .num_columns
            .map_or(auto_num, |n| n.min(auto_num));

        for i in 0..num_columns {
            let mut column = ColumnComponent::new(self.config.clone(), self.action_tx.clone());
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
        if let Some(selected) = self.state.selected {
            self.columns[selected].handle_key_events(key)
        } else {
            Ok(None)
        }
    }
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::NextFocus => {
                self.state.selected =
                    Some(self.state.selected.map_or(0, |s| s + 1) % self.columns.len());
                return Ok(Some(Action::Render));
            }
            Action::PrevFocus => {
                self.state.selected = Some(
                    self.state
                        .selected
                        .map_or(0, |s| s + self.columns.len() - 1)
                        % self.columns.len(),
                );
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
                block = block
                    .border_type(BorderType::Double)
                    .border_style(Style::default().reset().bold());
            }
            view.draw(f, block.inner(*area))?;
            f.render_widget(block, *area);
        }
        Ok(())
    }
}
