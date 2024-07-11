use bsky_sdk::api::app::bsky::feed::defs::GeneratorView;

#[derive(Debug, Clone)]
pub struct SavedFeed {
    pub pinned: bool,
    pub value: FeedDescriptor,
}

#[derive(Debug, Clone)]
pub enum FeedDescriptor {
    Feed(Box<GeneratorView>),
    List, // TODO
    Timeline(String),
}
