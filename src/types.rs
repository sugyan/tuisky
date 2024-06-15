use crossterm::event::{Event as CrosstermEvent, KeyEvent, MouseEvent};

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Panic,
    Quit,
    Tick,
}

#[derive(Debug, Clone)]
pub enum Event {
    Tick,
    Key(KeyEvent),
    Mouse(MouseEvent),
    Focus(CrosstermEvent),
    Error(String),
}
