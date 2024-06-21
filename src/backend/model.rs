use bsky_sdk::BskyAgent;
use std::sync::Arc;

pub struct Views {
    agent: Arc<BskyAgent>,
    views: Vec<View>,
}

impl Views {
    pub fn new(agent: Arc<BskyAgent>) -> Self {
        Self {
            agent,
            views: Vec::new(),
        }
    }
    pub fn current(&self) -> Option<&View> {
        self.views.last()
    }
    pub fn current_mut(&mut self) -> Option<&mut View> {
        self.views.last_mut()
    }
}

pub enum View {}
