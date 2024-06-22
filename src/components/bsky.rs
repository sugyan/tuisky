use super::Component;
use crate::backend::Manager;
use color_eyre::Result;
use ratatui::{layout::Rect, widgets::Paragraph, Frame};
use std::sync::Arc;

pub struct BskyComponent {
    pub manager: Arc<Manager>,
}

impl BskyComponent {
    pub fn new(manager: Manager) -> Self {
        Self {
            manager: Arc::new(manager),
        }
    }
}

impl Component for BskyComponent {
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        f.render_widget(Paragraph::new("Welcome to Bluesky!"), area);
        Ok(())
    }
}
