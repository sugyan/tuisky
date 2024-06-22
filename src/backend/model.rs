use bsky_sdk::{agent::config::Config, BskyAgent};
use std::sync::Arc;

pub struct Manager {
    agent: Arc<BskyAgent>,
    views: Vec<View>,
}

impl Manager {
    pub fn new(agent: Arc<BskyAgent>) -> Self {
        Self {
            agent,
            views: Vec::new(),
        }
    }
    pub async fn agent_config(&self) -> Config {
        self.agent.to_config().await
    }
    pub fn current(&self) -> Option<&View> {
        self.views.last()
    }
    pub fn current_mut(&mut self) -> Option<&mut View> {
        self.views.last_mut()
    }
}

pub enum View {}
