use bsky_sdk::api::app::bsky::actor::defs::SavedFeed;
use bsky_sdk::api::app::bsky::feed::defs::GeneratorView;
use bsky_sdk::api::app::bsky::graph::defs::ListView;

#[derive(Debug, Clone)]
pub struct PinnedFeed {
    pub saved_feed: SavedFeed,
    pub info: FeedSourceInfo,
}

#[derive(Debug, Clone)]
pub enum FeedSourceInfo {
    Feed(Box<GeneratorView>),
    List(Box<ListView>),
    Timeline(String),
}
