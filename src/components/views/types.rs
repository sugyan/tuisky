use crate::backend::types::SavedFeed;
use bsky_sdk::BskyAgent;
use std::fmt::{Debug, Formatter, Result};

#[derive(Clone)]
pub enum Action {
    Render,
    NextItem,
    PrevItem,
    NextInput,
    PrevInput,
    Enter,
    Login(Box<BskyAgent>),
    Logout,
    Update(Box<Data>),
}

impl Debug for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Action::Render => write!(f, "Render"),
            Action::NextItem => write!(f, "NextItem"),
            Action::PrevItem => write!(f, "PrevItem"),
            Action::NextInput => write!(f, "NextInput"),
            Action::PrevInput => write!(f, "PrevInput"),
            Action::Enter => write!(f, "Enter"),
            Action::Login(_) => write!(f, "Login"),
            Action::Logout => write!(f, "Logout"),
            Action::Update(_) => write!(f, "Update"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Data {
    SavedFeeds(Vec<SavedFeed>),
    Other,
}
