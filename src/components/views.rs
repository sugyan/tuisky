pub mod feed;

use super::super::components::Component;
use crate::types::Action;
use color_eyre::Result;
use feed::FeedView;
use ratatui::{layout::Rect, Frame};

pub enum View {
    Feed(FeedView),
}

impl Component for View {
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match self {
            View::Feed(feed) => feed.update(action),
        }
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        match self {
            View::Feed(feed) => feed.draw(f, area),
        }
    }
}
