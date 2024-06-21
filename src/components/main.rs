use super::login::LoginComponent;
use super::view::ViewComponent;
use super::Component;
use crate::types::Action;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, BorderType};
use ratatui::Frame;

#[derive(Default)]
struct State {
    selected: Option<usize>,
}

pub struct MainComponent<'a> {
    views: Vec<ViewComponent<'a>>,
    state: State,
}

impl<'a> MainComponent<'a> {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            views: vec![
                ViewComponent::Login(Box::new(LoginComponent::new())),
                ViewComponent::Login(Box::new(LoginComponent::new())),
            ],
            state: State { selected: Some(0) },
        })
    }
}

impl<'a> Component for MainComponent<'a> {
    fn init(&mut self, rect: Rect) -> Result<()> {
        for view in self.views.iter_mut() {
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
            return self.views[selected].handle_key_events(key);
        }
        Ok(None)
    }
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        // TODO: update non-selected views
        if matches!(action, Action::NextFocus) {
            self.state.selected = Some(self.state.selected.map_or(0, |s| s + 1) % self.views.len());
        } else if let Some(selected) = self.state.selected {
            return self.views[selected].update(action);
        }
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(self.views.iter().map(|_| Constraint::Fill(1)))
            .split(area);
        for (i, (area, view)) in layout.iter().zip(self.views.iter_mut()).enumerate() {
            let mut block = Block::bordered()
                .title(format!("column {i:02}"))
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
