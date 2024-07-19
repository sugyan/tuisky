pub mod feed;
pub mod login;
pub mod new_post;
pub mod post;
pub mod root;
pub mod types;
mod utils;

use self::types::{Action, View};
use color_eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};

pub trait ViewComponent {
    fn view(&self) -> View;
    fn activate(&mut self) -> Result<()> {
        Ok(())
    }
    fn deactivate(&mut self) -> Result<()> {
        Ok(())
    }
    #[allow(unused_variables)]
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        Ok(None)
    }
    #[allow(unused_variables)]
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()>;
}
