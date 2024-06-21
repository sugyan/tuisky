use bsky_sdk::agent::config::Config;
use crossterm::event::{KeyEvent, MouseEvent};

#[derive(Debug, Clone)]
pub enum Action {
    Error(String),
    Quit,
    Tick,
    Render,
    NextItem,
    PrevItem,
    NextInput,
    PrevInput,
    NextFocus,
    PrevFocus,
    Submit,
    Login(Config),
}

#[derive(Debug, Clone)]
pub enum Event {
    Tick,
    Render,
    Key(KeyEvent),
    Mouse(MouseEvent),
    Error(String),
}
