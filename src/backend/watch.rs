use super::types::SavedFeed;
use crate::backend::types::SavedFeedValue;
use bsky_sdk::api::app::bsky::actor::defs::SavedFeedData;
use bsky_sdk::api::app::bsky::feed::defs::FeedViewPost;
use bsky_sdk::api::types::Object;
use bsky_sdk::preference::Preferences;
use bsky_sdk::Result;
use bsky_sdk::{agent::config::Config, BskyAgent};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch::{self, Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::time;

pub struct Watcher {
    agent: Arc<BskyAgent>,
    preferences: Sender<Preferences>,
    handles: Vec<JoinHandle<()>>,
}

impl Watcher {
    pub fn new(agent: Arc<BskyAgent>) -> Self {
        let (preferences, mut rx) = watch::channel(Preferences::default());
        let mut handles = Vec::new();
        {
            handles.push(tokio::spawn(async move {
                while rx.changed().await.is_ok() {
                    let _ = rx.borrow_and_update();
                    log::info!("Preferences updated");
                }
            }));
            let (agent, tx) = (agent.clone(), preferences.clone());
            handles.push(tokio::spawn(async move {
                let mut interval = time::interval(Duration::from_secs(1));
                let mut preferences_interval = time::interval(Duration::from_secs(60));
                loop {
                    let tick = interval.tick();
                    let preferences_tick = preferences_interval.tick();
                    tokio::select! {
                        _ = tick => {}
                        _ = preferences_tick => {
                            if let Ok(prefs) = agent.get_preferences(true).await {
                                if let Err(e) = tx.send(prefs.clone()) {
                                    log::error!("failed to send preferences data: {e}");
                                }
                            } else {
                                log::warn!("failed to get preferences");
                            }
                        }
                    }
                }
            }));
        }
        Self {
            agent,
            preferences,
            handles,
        }
    }
    pub fn stop(&self) {
        for handle in &self.handles {
            handle.abort();
        }
    }
    pub async fn get_agent_config(&self) -> Config {
        self.agent.to_config().await
    }
    pub fn saved_feeds(&self, init: Vec<SavedFeed>) -> Receiver<Vec<SavedFeed>> {
        let (tx, rx) = watch::channel(init);
        let agent = self.agent.clone();
        let mut preferences = self.preferences.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    changed = preferences.changed() => {
                        if changed.is_err()  {
                            return log::warn!("preferences channel closed");
                        }
                        let saved_feeds = preferences.borrow_and_update().saved_feeds.clone();
                        match collect_feeds(&agent, &saved_feeds).await {
                            Ok(feeds) => {
                                if let Err(e) = tx.send(feeds) {
                                    log::error!("failed to send saved feeds: {e}");
                                }
                            }
                            Err(e) => {
                                log::warn!("failed to collect feeds {e:?}");
                            }
                        }
                    }
                    _ = tx.closed() => {
                        return log::warn!("saved feeds channel closed");
                    }
                }
            }
        });
        rx
    }
    pub fn feed_views(
        &self,
        init: Vec<FeedViewPost>,
        feed: &SavedFeedValue,
    ) -> Receiver<Vec<FeedViewPost>> {
        let (tx, rx) = watch::channel(init);
        let agent = self.agent.clone();
        let feed = feed.clone();
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(30));
            loop {
                let tick = interval.tick();
                tokio::select! {
                    _ = tick => {
                        match fetch_feed_views(&agent, &feed).await {
                            Ok(feed_vies) => {
                                if let Err(e) = tx.send(feed_vies) {
                                    log::error!("failed to send feed views: {e}");
                                }
                            }
                            Err(e) => {
                                log::warn!("failed to fetch feed views {e:?}");
                            }
                        }
                    }
                    _ = tx.closed() => {
                        return log::warn!("feed views channel closed");
                    }
                }
            }
        });
        rx
    }
}

async fn collect_feeds(
    agent: &Arc<BskyAgent>,
    saved_feeds: &[Object<SavedFeedData>],
) -> Result<Vec<SavedFeed>> {
    let feeds = saved_feeds
        .iter()
        .filter_map(|feed| {
            if feed.r#type == "feed" {
                Some(feed.value.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let feed_generators = agent
        .api
        .app
        .bsky
        .feed
        .get_feed_generators(
            bsky_sdk::api::app::bsky::feed::get_feed_generators::ParametersData { feeds }.into(),
        )
        .await
        .map(|output| {
            output
                .feeds
                .iter()
                .map(|feed| (feed.uri.clone(), feed.clone()))
                .collect::<HashMap<_, _>>()
        })?;
    // TODO: list
    let mut feeds = Vec::new();
    for data in saved_feeds {
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
    Ok(feeds)
}

async fn fetch_feed_views(
    agent: &Arc<BskyAgent>,
    feed: &SavedFeedValue,
) -> Result<Vec<FeedViewPost>> {
    match feed {
        SavedFeedValue::Feed(generator_view) => Ok(agent
            .api
            .app
            .bsky
            .feed
            .get_feed(
                bsky_sdk::api::app::bsky::feed::get_feed::ParametersData {
                    cursor: None,
                    feed: generator_view.uri.clone(),
                    limit: 30.try_into().ok(),
                }
                .into(),
            )
            .await?
            .data
            .feed
            .into_iter()
            .rev()
            .collect()),
        SavedFeedValue::List => Ok(Vec::new()),
        SavedFeedValue::Timeline(_) => Ok(agent
            .api
            .app
            .bsky
            .feed
            .get_timeline(
                bsky_sdk::api::app::bsky::feed::get_timeline::ParametersData {
                    algorithm: None,
                    cursor: None,
                    limit: 30.try_into().ok(),
                }
                .into(),
            )
            .await?
            .data
            .feed
            .into_iter()
            .rev()
            .collect()),
    }
}
