use super::types::Action;
use super::ViewComponent;
use crate::backend::types::{SavedFeed, SavedFeedValue};
use crate::components::views::types::Data;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{List, ListState};
use ratatui::{layout::Rect, Frame};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::watch::Receiver;

pub struct RootComponent {
    items: Vec<Text<'static>>,
    state: ListState,
    action_tx: UnboundedSender<Action>,
}

impl RootComponent {
    pub fn new(action_tx: UnboundedSender<Action>, mut rx: Receiver<Vec<SavedFeed>>) -> Self {
        let tx = action_tx.clone();
        tokio::spawn(async move {
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
            action_tx,
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
            _ => Ok(None),
        }
    }
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::NextItem if !self.items.is_empty() => {
                self.state.select(Some(
                    self.state
                        .selected()
                        .map(|s| (s + 1).min(self.items.len() - 1))
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
            Action::Update(data) => {
                let Data::SavedFeeds(feeds) = data.as_ref() else {
                    return Ok(None);
                };
                self.items = feeds
                    .iter()
                    .filter_map(|feed| match &feed.value {
                        SavedFeedValue::Feed(generator_view) => Some(Text::from(vec![
                            Line::from(vec![
                                Span::from(generator_view.display_name.clone()).bold(),
                                Span::from(" "),
                                Span::from(format!(
                                    "by {}",
                                    generator_view.creator.display_name.clone().unwrap_or(
                                        generator_view.creator.handle.as_ref().to_string()
                                    )
                                ))
                                .dim(),
                            ]),
                            // Line::from(format!(
                            //     "  {}",
                            //     generator_view.description.clone().unwrap_or_default()
                            // )),
                            // Line::from(format!("  {}", generator_view.uri)).gray(),
                        ])),
                        SavedFeedValue::List => None,
                        SavedFeedValue::Timeline(value) => Some(Text::from(vec![
                            Line::from(value.clone()).bold(),
                            // Line::from(String::new()),
                            // Line::from(String::new()),
                        ])),
                    })
                    .collect();
                if self.state.selected().is_none() && !self.items.is_empty() {
                    self.state.select(Some(0));
                }
            }
            _ => {}
        }
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        f.render_stateful_widget(
            List::new(self.items.to_vec()).highlight_style(Style::default().reversed()),
            area,
            &mut self.state,
        );
        Ok(())
    }
}
