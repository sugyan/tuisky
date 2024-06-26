use super::types::SavedFeed;
use crate::backend::types::SavedFeedValue;
use bsky_sdk::preference::Preferences;
use bsky_sdk::{agent::config::Config, BskyAgent};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch::{self, Receiver, Sender};
use tokio::time;

pub struct Manager {
    agent: Arc<BskyAgent>,
    views: Vec<View>,
    pub data: Data,
}

#[derive(Debug)]
pub struct Data {
    pub preferences: Sender<Preferences>,
    pub saved_feeds: Sender<Vec<SavedFeed>>,
    _preferences: Receiver<Preferences>,
    _saved_feeds: Receiver<Vec<SavedFeed>>,
}

impl Default for Data {
    fn default() -> Self {
        let (preferences, _preferences) = watch::channel(Preferences::default());
        let (saved_feeds, _saved_feeds) = watch::channel(Vec::default());
        Self {
            preferences,
            saved_feeds,
            _preferences,
            _saved_feeds,
        }
    }
}

impl Manager {
    pub fn new(agent: Arc<BskyAgent>) -> Self {
        Self {
            agent,
            views: Vec::new(),
            data: Data::default(),
        }
    }
    pub fn start(&self) {
        let agent = self.agent.clone();
        let preferences = self.data.preferences.clone();
        let feeds = self.data.saved_feeds.clone();
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(1));
            let mut preferences_interval = time::interval(Duration::from_secs(60));
            loop {
                let tick = interval.tick();
                let preferences_tick = preferences_interval.tick();
                tokio::select! {
                    _ = tick => {}
                    _ = preferences_tick => Self::preferences(&agent, &preferences, &feeds).await,
                }
            }
        });
    }
    pub async fn get_agent_config(&self) -> Config {
        self.agent.to_config().await
    }
    pub fn current(&self) -> Option<&View> {
        self.views.last()
    }
    pub fn current_mut(&mut self) -> Option<&mut View> {
        self.views.last_mut()
    }
    async fn preferences(
        agent: &Arc<BskyAgent>,
        preferences: &Sender<Preferences>,
        saved_feeds: &Sender<Vec<SavedFeed>>,
    ) {
        let Ok(prefs) = agent.get_preferences(true).await else {
            return log::error!("failed to get preferences");
        };

        log::debug!("got preferences");
        if let Err(e) = preferences.send(prefs.clone()) {
            log::error!("failed to send preferences data: {e}");
        }

        let feeds = prefs
            .saved_feeds
            .iter()
            .filter_map(|feed| {
                if feed.r#type == "feed" {
                    Some(feed.value.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let Ok(feed_generators) = agent
            .api
            .app
            .bsky
            .feed
            .get_feed_generators(
                atrium_api::app::bsky::feed::get_feed_generators::ParametersData { feeds }.into(),
            )
            .await
            .map(|output| {
                output
                    .feeds
                    .iter()
                    .map(|feed| (feed.uri.clone(), feed.clone()))
                    .collect::<HashMap<_, _>>()
            })
        else {
            return log::error!("failed to get feeds");
        };
        // TODO: list
        let mut feeds = Vec::new();
        for data in &prefs.saved_feeds {
            match data.r#type.as_str() {
                "feed" => {
                    if let Some(feed) = feed_generators.get(&data.value) {
                        feeds.push(SavedFeed {
                            pinned: data.pinned,
                            value: SavedFeedValue::Feed(Box::new(feed.clone())),
                        });
                    }
                }
                "list" => {
                    // TODO
                }
                "timeline" => {
                    feeds.push(SavedFeed {
                        pinned: data.pinned,
                        value: SavedFeedValue::Timeline(data.value.clone()),
                    });
                }
                _ => {}
            }
        }
        if let Err(e) = saved_feeds.send(feeds) {
            log::error!("failed to send saved feeds: {e}");
        }
    }
}

pub enum View {}
