use super::types::Action;
use super::ViewComponent;
use crate::backend::types::{SavedFeed, SavedFeedValue};
use crate::components::views::types::Data;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, List, ListState, Padding};
use ratatui::{layout::Rect, Frame};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::watch::Receiver;
use tokio::task::JoinHandle;

pub struct RootComponent {
    items: Vec<SavedFeed>,
    state: ListState,
    handle: JoinHandle<()>,
}

impl RootComponent {
    pub fn new(action_tx: UnboundedSender<Action>, mut rx: Receiver<Vec<SavedFeed>>) -> Self {
        let tx = action_tx.clone();
        let handle = tokio::spawn(async move {
            while rx.changed().await.is_ok() {
                if let Err(e) = tx.send(Action::Update(Box::new(Data::SavedFeeds(
                    rx.borrow_and_update().clone(),
                )))) {
                    log::error!("failed to send update action: {e}");
                }
            }
        });
        Self {
            items: Vec::new(),
            state: ListState::default(),
            handle,
        }
    }
}

impl ViewComponent for RootComponent {
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('n'), KeyModifiers::CONTROL) | (KeyCode::Down, KeyModifiers::NONE) => {
                Ok(Some(Action::NextItem))
            }
            (KeyCode::Char('p'), KeyModifiers::CONTROL) | (KeyCode::Up, KeyModifiers::NONE) => {
                Ok(Some(Action::PrevItem))
            }
            (KeyCode::Enter, _) => Ok(Some(Action::Enter)),
            _ => Ok(None),
        }
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
                        self.handle.abort();
                        return Ok(Some(Action::Logout));
                    }
                    if let Some(feed) = self.items.get(index) {
                        // return Ok(Some(Action::ViewFeed(feed.clone())));
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
                        generator_view
                            .creator
                            .display_name
                            .clone()
                            .unwrap_or(generator_view.creator.handle.as_ref().to_string())
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
