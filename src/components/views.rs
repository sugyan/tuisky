pub mod login;
pub mod root;
pub mod types;

use self::types::Action;
use color_eyre::Result;
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{layout::Rect, Frame};
use tokio::sync::mpsc::UnboundedSender;

pub trait ViewComponent {
    #[allow(unused_variables)]
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        Ok(())
    }
    #[allow(unused_variables)]
    fn init(&mut self, area: Rect) -> Result<()> {
        Ok(())
    }
    #[allow(unused_variables)]
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        Ok(None)
    }
    #[allow(unused_variables)]
    fn handle_mouse_events(&mut self, mouse: MouseEvent) -> Result<Option<Action>> {
        Ok(None)
    }
    #[allow(unused_variables)]
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()>;
}
