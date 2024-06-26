use bsky_sdk::api::app::bsky::feed::defs::GeneratorView;

#[derive(Debug, Clone)]
pub struct SavedFeed {
    pub pinned: bool,
    pub value: SavedFeedValue,
}

#[derive(Debug, Clone)]
pub enum SavedFeedValue {
    Feed(Box<GeneratorView>),
    List, // TODO
    Timeline(String),
}
