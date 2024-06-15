use crossterm::event::{Event as CrosstermEvent, KeyEvent, MouseEvent};

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Quit,
    Tick,
    Render,
    NextItem,
    PrevItem,
}

#[derive(Debug, Clone)]
pub enum Event {
    Tick,
    Render,
    Key(KeyEvent),
    Mouse(MouseEvent),
    Focus(CrosstermEvent),
    Error(String),
}
