mod embed;
mod embed_images;
mod embed_record;
pub mod types;

pub use self::embed::EmbedModalComponent;
use {
    self::types::Action,
    super::views::types::Action as ViewsAction,
    color_eyre::Result,
    crossterm::event::KeyEvent,
    ratatui::{layout::Rect, Frame},
};

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
