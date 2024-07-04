use crate::backend::types::{SavedFeed, SavedFeedValue};
use bsky_sdk::api::app::bsky::feed::defs::FeedViewPost;
use bsky_sdk::api::types::string::Cid;
use bsky_sdk::BskyAgent;
use indexmap::IndexMap;
use std::fmt::{Debug, Formatter, Result};

#[derive(Clone)]
pub enum Action {
    Render,
    NextItem,
    PrevItem,
    NextInput,
    PrevInput,
    Enter,
    Back,
    Login(Box<BskyAgent>),
    Logout,
    Update(Box<Data>),
    Transition(Transition),
}

impl Debug for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Action::Render => write!(f, "Render"),
            Action::NextItem => write!(f, "NextItem"),
            Action::PrevItem => write!(f, "PrevItem"),
            Action::Back => write!(f, "Back"),
            Action::NextInput => write!(f, "NextInput"),
            Action::PrevInput => write!(f, "PrevInput"),
            Action::Enter => write!(f, "Enter"),
            Action::Login(_) => write!(f, "Login"),
            Action::Logout => write!(f, "Logout"),
            Action::Update(_) => write!(f, "Update"),
            Action::Transition(arg) => f.debug_tuple("Transition").field(arg).finish(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Data {
    SavedFeeds(Vec<SavedFeed>),
    FeedViews(IndexMap<Cid, FeedViewPost>),
}

#[derive(Debug, Clone)]
pub enum Transition {
    Push(Box<View>),
    Pop,
    Replace(Box<View>),
}

#[derive(Debug, Clone)]
pub enum View {
    Root,
    Feed(Box<SavedFeedValue>),
}
