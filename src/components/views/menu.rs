use {
    super::types::Action,
    crate::config::{ColumnAction, Key, Keybindings},
    color_eyre::Result,
    ratatui::{
        layout::Rect,
        style::{Style, Stylize},
        text::{Line, Span},
        widgets::{Block, Clear, List, ListItem, ListState},
        Frame,
    },
    tokio::sync::mpsc::UnboundedSender,
};

enum MenuAction {
    NewPost(Vec<String>),
    Refresh(Vec<String>),
    Back(Vec<String>),
}

impl<'a> From<&'a MenuAction> for ListItem<'a> {
    fn from(action: &'a MenuAction) -> Self {
        match action {
            MenuAction::NewPost(v) if !v.is_empty() => Self::from(Line::from(vec![
                Span::from("New Post ").reset(),
                Span::from(format!("({})", v.join(", "))).dim(),
            ])),
            MenuAction::NewPost(_) => Self::from("New Post".reset()),
            MenuAction::Refresh(v) if !v.is_empty() => Self::from(Line::from(vec![
                Span::from("Refresh ").reset(),
                Span::from(format!("({})", v.join(", "))).dim(),
            ])),
            MenuAction::Refresh(_) => Self::from("Refresh".reset()),
            MenuAction::Back(v) if !v.is_empty() => Self::from(Line::from(vec![
                Span::from("Back ").reset(),
                Span::from(format!("({})", v.join(", "))).dim(),
            ])),
            MenuAction::Back(_) => Self::from("Back".reset()),
        }
    }
}

pub struct MenuViewComponent {
    action_tx: UnboundedSender<Action>,
    items: Vec<MenuAction>,
    state: ListState,
}

impl MenuViewComponent {
    pub fn new(action_tx: UnboundedSender<Action>, keybindings: &Keybindings) -> Self {
        let mut keys = vec![Vec::new(); 3];
        for (k, v) in &keybindings.column {
            match v {
                ColumnAction::NewPost => keys[0].push(k),
                ColumnAction::Refresh => keys[1].push(k),
                ColumnAction::Back => keys[2].push(k),
                _ => {}
            }
        }
        keys.iter_mut().for_each(|v| v.sort());
        let to_string = |v: &[&Key]| {
            v.iter()
                .filter_map(|k| serde_json::to_string(k).ok())
                .map(|s| s.trim_matches('"').to_string())
                .collect::<Vec<_>>()
        };
        Self {
            action_tx,
            items: vec![
                MenuAction::NewPost(to_string(&keys[0])),
                MenuAction::Refresh(to_string(&keys[1])),
                MenuAction::Back(to_string(&keys[2])),
            ],
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
                        MenuAction::NewPost(_) => Action::NewPost,
                        MenuAction::Refresh(_) => Action::Refresh,
                        MenuAction::Back(_) => Action::Back,
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
