mod feed;
mod login;
mod menu;
mod new_post;
mod post;
mod root;
pub mod types;
mod utils;

use self::types::{Action, View};
pub use self::{
    feed::FeedViewComponent, login::LoginComponent, menu::MenuViewComponent,
    new_post::NewPostViewComponent, post::PostViewComponent, root::RootComponent,
};
use {
    color_eyre::Result,
    crossterm::event::KeyEvent,
    ratatui::{layout::Rect, Frame},
};

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
