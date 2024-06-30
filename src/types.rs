use crate::components::views::types::Action as ViewAction;
use bsky_sdk::BskyAgent;
use crossterm::event::{KeyEvent, MouseEvent};
use std::fmt::{Debug, Formatter, Result};

pub type IdType = u32;

#[derive(Clone)]
pub enum Action {
    Error(String),
    Quit,
    Tick(usize),
    Render,
    NextFocus,
    PrevFocus,
    View((IdType, ViewAction)),
    Login((IdType, Box<BskyAgent>)),
}

impl Debug for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Error(arg) => f.debug_tuple("Error").field(arg).finish(),
            Self::Quit => write!(f, "Quit"),
            Self::Tick(arg) => f.debug_tuple("Tick").field(arg).finish(),
            Self::Render => write!(f, "Render"),
            Self::View(arg) => f.debug_tuple("View").field(arg).finish(),
            Self::NextFocus => write!(f, "NextFocus"),
            Self::PrevFocus => write!(f, "PrevFocus"),
            Self::Login((arg, _)) => f.debug_tuple("Login").field(arg).finish(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Tick(usize),
    Render,
    Key(KeyEvent),
    Mouse(MouseEvent),
    Error(String),
}
