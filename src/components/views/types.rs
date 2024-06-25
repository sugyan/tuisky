use bsky_sdk::BskyAgent;
use std::fmt::{Debug, Formatter, Result};

#[derive(Clone)]
pub enum Action {
    NextInput,
    PrevInput,
    Enter,
    Login(Box<BskyAgent>),
}

impl Debug for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Action::NextInput => write!(f, "NextInput"),
            Action::PrevInput => write!(f, "PrevInput"),
            Action::Enter => write!(f, "Enter"),
            Action::Login(_) => write!(f, "Login"),
        }
    }
}
