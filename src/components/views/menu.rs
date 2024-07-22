use super::types::Action;
use color_eyre::Result;
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState};
use ratatui::Frame;
use tokio::sync::mpsc::UnboundedSender;

enum MenuAction {
    NewPost,
    Refresh,
    Back,
}

impl<'a> From<&'a MenuAction> for ListItem<'a> {
    fn from(action: &'a MenuAction) -> Self {
        match action {
            MenuAction::NewPost => Self::from("New Post"),
            MenuAction::Refresh => Self::from("Refresh"),
            MenuAction::Back => Self::from("Back"),
        }
    }
}

pub struct MenuViewComponent {
    action_tx: UnboundedSender<Action>,
    items: Vec<MenuAction>,
    state: ListState,
}

impl MenuViewComponent {
    pub fn new(action_tx: UnboundedSender<Action>) -> Self {
        Self {
            action_tx,
            items: vec![MenuAction::NewPost, MenuAction::Refresh, MenuAction::Back],
            state: ListState::default().with_selected(Some(0)),
        }
    }
    pub fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::NextItem => {
                if let Some(selected) = self.state.selected() {
                    self.state
                        .select(Some((selected + 1).min(self.items.len() - 1)));
                    return Ok(Some(Action::Render));
                }
            }
            Action::PrevItem => {
                if let Some(selected) = self.state.selected() {
                    self.state.select(Some(selected.max(1) - 1));
                    return Ok(Some(Action::Render));
                }
            }
            Action::Enter => {
                if let Some(selected) = self.state.selected() {
                    let action = match self.items[selected] {
                        MenuAction::NewPost => Action::NewPost,
                        MenuAction::Refresh => Action::Refresh,
                        MenuAction::Back => Action::Back,
                    };
                    self.action_tx.send(action).ok();
                    return Ok(Some(Action::Menu));
                }
            }
            Action::Update(_) | Action::Render => {
                return Ok(None);
            }
            _ => {}
        }
        Ok(Some(Action::Menu))
    }
    pub fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let area = Rect::new(area.x, area.y, area.width, self.items.len() as u16 + 2);
        f.render_widget(Clear, area);
        f.render_stateful_widget(
            List::new(&self.items)
                .block(Block::bordered().title("Menu").dim())
                .highlight_style(Style::default().reversed()),
            area,
            &mut self.state,
        );
        Ok(())
    }
}
