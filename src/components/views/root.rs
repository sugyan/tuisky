use super::ViewComponent;
use crate::types::{Action, ViewData};
use color_eyre::Result;
use ratatui::widgets::List;
use ratatui::{layout::Rect, Frame};

#[derive(Default)]
pub struct RootComponent {
    items: Vec<String>,
}

impl RootComponent {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
}

impl ViewComponent for RootComponent {
    // fn update(&mut self, action: Action) -> Result<Option<Action>> {
    //     if let Action::Updated((_, data)) = action {
    //         match data.as_ref() {
    //             ViewData::Preferences(preferences) => {
    //                 self.items = preferences
    //                     .saved_feeds
    //                     .iter()
    //                     .map(|feed| feed.value.clone())
    //                     .collect()
    //             }
    //         }
    //     }
    //     Ok(None)
    // }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        f.render_widget(
            List::new(self.items.iter().map(String::from).collect::<Vec<_>>()),
            area,
        );
        Ok(())
    }
}
