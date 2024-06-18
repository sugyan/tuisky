use super::views::{feed::FeedView, View};
use super::Component;
use crate::types::Action;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::Frame;

pub struct MainComponent {
    views: Vec<View>,
}

impl MainComponent {
    pub fn new() -> Self {
        Self {
            views: vec![View::Feed(FeedView::new())],
        }
    }
}

impl Component for MainComponent {
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('n'), KeyModifiers::CONTROL) => return Ok(Some(Action::NextItem)),
            (KeyCode::Char('p'), KeyModifiers::CONTROL) => return Ok(Some(Action::PrevItem)),
            (KeyCode::Down, KeyModifiers::NONE) => return Ok(Some(Action::NextItem)),
            (KeyCode::Up, KeyModifiers::NONE) => return Ok(Some(Action::PrevItem)),
            _ => {}
        }
        Ok(None)
    }
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        if let Some(top) = self.views.last_mut() {
            if matches!(action, Action::NextItem | Action::PrevItem) {
                return top.update(action);
            }
        }
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        if let Some(top) = self.views.last_mut() {
            top.draw(f, area)?;
        }
        Ok(())
    }
}
