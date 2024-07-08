use super::types::{Action, Transition, View};
use super::utils::profile_name_as_str;
use super::ViewComponent;
use crate::backend::types::{SavedFeed, SavedFeedValue};
use crate::backend::Watcher;
use crate::components::views::types::Data;
use color_eyre::Result;
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, List, ListState, Padding};
use ratatui::{layout::Rect, Frame};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

pub struct RootComponent {
    items: Vec<SavedFeed>,
    state: ListState,
    action_tx: UnboundedSender<Action>,
    watcher: Arc<Watcher>,
    handle: Option<JoinHandle<()>>,
}

impl RootComponent {
    pub fn new(action_tx: UnboundedSender<Action>, watcher: Arc<Watcher>) -> Self {
        Self {
            items: Vec::new(),
            state: ListState::default(),
            action_tx,
            watcher,
            handle: None,
        }
    }
}

impl ViewComponent for RootComponent {
    fn activate(&mut self) -> Result<()> {
        let (tx, mut rx) = (
            self.action_tx.clone(),
            self.watcher.saved_feeds(self.items.clone()),
        );
        self.handle = Some(tokio::spawn(async move {
            while rx.changed().await.is_ok() {
                if let Err(e) = tx.send(Action::Update(Box::new(Data::SavedFeeds(
                    rx.borrow_and_update().clone(),
                )))) {
                    log::error!("failed to send update action: {e}");
                }
            }
        }));
        Ok(())
    }
    fn deactivate(&mut self) -> Result<()> {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
        Ok(())
    }
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::NextItem if !self.items.is_empty() => {
                self.state.select(Some(
                    self.state
                        .selected()
                        .map(|s| (s + 1).min(self.items.len()))
                        .unwrap_or_default(),
                ));
                return Ok(Some(Action::Render));
            }
            Action::PrevItem if !self.items.is_empty() => {
                self.state.select(Some(
                    self.state
                        .selected()
                        .map(|s| s.max(1) - 1)
                        .unwrap_or_default(),
                ));
                return Ok(Some(Action::Render));
            }
            Action::Enter if !self.items.is_empty() => {
                if let Some(index) = self.state.selected() {
                    if index == self.items.len() {
                        if let Some(handle) = self.handle.take() {
                            handle.abort();
                        }
                        return Ok(Some(Action::Logout));
                    }
                    if let Some(feed) = self.items.get(index) {
                        return Ok(Some(Action::Transition(Transition::Push(Box::new(
                            View::Feed(Box::new(feed.value.clone())),
                        )))));
                    }
                }
            }
            Action::Update(data) => {
                let Data::SavedFeeds(feeds) = data.as_ref() else {
                    return Ok(None);
                };
                self.items.clone_from(feeds);
                if self.state.selected().is_none() && !self.items.is_empty() {
                    self.state.select(Some(0));
                }
                return Ok(Some(Action::Render));
            }
            _ => {}
        }
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let mut items = self
            .items
            .iter()
            .filter_map(|feed| match &feed.value {
                SavedFeedValue::Feed(generator_view) => Some(Text::from(vec![Line::from(vec![
                    Span::from(generator_view.display_name.clone()).bold(),
                    Span::from(" "),
                    Span::from(format!(
                        "by {}",
                        profile_name_as_str(&generator_view.creator)
                    ))
                    .dim(),
                ])])),
                SavedFeedValue::List => None,
                SavedFeedValue::Timeline(value) => {
                    Some(Text::from(vec![Line::from(value.clone()).bold()]))
                }
            })
            .collect::<Vec<_>>();
        if !items.is_empty() {
            items.push(Text::from("Sign out").red());
        }
        f.render_stateful_widget(
            List::new(items)
                .block(Block::default().padding(Padding::uniform(1)))
                .highlight_style(Style::default().reversed()),
            area,
            &mut self.state,
        );
        Ok(())
    }
}
