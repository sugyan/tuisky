use crate::backend::types::{FeedSourceInfo, PinnedFeed};
use bsky_sdk::api::app::bsky::feed::defs::{FeedViewPost, PostView, ViewerState};
use bsky_sdk::api::app::bsky::feed::get_post_thread::OutputThreadRefs;
use bsky_sdk::api::types::Union;
use bsky_sdk::BskyAgent;
use std::fmt::{Debug, Formatter, Result};

#[derive(Clone)]
pub enum Action {
    Render,
    NextItem,
    PrevItem,
    Enter,
    Back,
    Refresh,
    NewPost,
    Menu,
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
            Action::Enter => write!(f, "Enter"),
            Action::Back => write!(f, "Back"),
            Action::Refresh => write!(f, "Refresh"),
            Action::NewPost => write!(f, "NewPost"),
            Action::Menu => write!(f, "Menu"),
            Action::Login(_) => write!(f, "Login"),
            Action::Logout => write!(f, "Logout"),
            Action::Update(_) => write!(f, "Update"),
            Action::Transition(arg) => f.debug_tuple("Transition").field(arg).finish(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Data {
    SavedFeeds(Vec<PinnedFeed>),
    Feed(Vec<FeedViewPost>),
    PostThread(Union<OutputThreadRefs>),
    ViewerState(Option<ViewerState>),
}

#[derive(Debug, Clone)]
pub enum Transition {
    Push(Box<View>),
    Pop,
    Replace(Box<View>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    Login,
    Root,
    NewPost,
    Feed(Box<FeedSourceInfo>),
    Post(Box<(PostView, Option<PostView>)>),
}
