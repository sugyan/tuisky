use bsky_sdk::api::app::bsky::{
    actor::defs::SavedFeed, feed::defs::GeneratorView, graph::defs::ListView,
};

#[derive(Debug, Clone)]
pub struct PinnedFeed {
    #[allow(dead_code)]
    pub saved_feed: SavedFeed,
    pub info: FeedSourceInfo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedSourceInfo {
    Feed(Box<GeneratorView>),
    List(Box<ListView>),
    Timeline(String),
}
