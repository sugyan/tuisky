use crossterm::event::{KeyEvent, MouseEvent};

#[derive(Debug, Clone)]
pub enum Action {
    Error(String),
    Quit,
    Tick(usize),
    Render,
    NextItem,
    PrevItem,
    NextInput,
    PrevInput,
    NextFocus,
    PrevFocus,
    Submit,
}

#[derive(Debug, Clone)]
pub enum Event {
    Tick(usize),
    Render,
    Key(KeyEvent),
    Mouse(MouseEvent),
    Error(String),
}
