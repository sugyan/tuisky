use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Config {
    pub intervals: Intervals,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Intervals {
    pub preferences: u64,
    pub feed_view_posts: u64,
    pub post_thread: u64,
}

impl Default for Intervals {
    fn default() -> Self {
        Self {
            preferences: 60,
            feed_view_posts: 30,
            post_thread: 60,
        }
    }
}
