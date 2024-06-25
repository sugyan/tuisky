use super::ViewComponent;
use color_eyre::Result;
use ratatui::{layout::Rect, Frame};

pub struct NoopComponent;

impl ViewComponent for NoopComponent {
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        Ok(())
    }
}
