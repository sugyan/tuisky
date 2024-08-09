mod embed;
mod embed_images;
pub mod types;

pub use self::embed::EmbedModalComponent;
use self::types::Action;
use super::views::types::Action as ViewsAction;
use color_eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::Frame;

pub trait ModalComponent {
    #[allow(unused_variables)]
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        Ok(None)
    }
    #[allow(unused_variables)]
    fn update(&mut self, action: ViewsAction) -> Result<Option<Action>> {
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()>;
}
