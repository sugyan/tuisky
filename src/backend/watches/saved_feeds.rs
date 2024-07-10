use super::super::types::{SavedFeed, SavedFeedValue};
use super::super::{Watch, Watcher};
use bsky_sdk::api::app::bsky::actor::defs::SavedFeedData;
use bsky_sdk::api::types::Object;
use bsky_sdk::preference::Preferences;
use bsky_sdk::{BskyAgent, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::watch::Sender;
use tokio::sync::{broadcast, watch};

impl Watcher {
    pub fn saved_feeds(&self) -> impl Watch<Output = Vec<SavedFeed>> {
        let (tx, _) = broadcast::channel(1);
        SavedFeedsWatcher {
            agent: self.agent.clone(),
            preferences: self.preferences(),
            tx,
        }
    }
}

pub struct SavedFeedsWatcher<W> {
    agent: Arc<BskyAgent>,
    preferences: W,
    tx: broadcast::Sender<()>,
}

impl<W> Watch for SavedFeedsWatcher<W>
where
    W: Watch<Output = Preferences>,
{
    type Output = Vec<SavedFeed>;

    fn subscribe(&self) -> tokio::sync::watch::Receiver<Self::Output> {
        let (tx, rx) = watch::channel(Default::default());
        let agent = self.agent.clone();
        let mut quit = self.tx.subscribe();
        let mut preferences = self.preferences.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    changed = preferences.changed() => {
                        if changed.is_ok() {
                            let saved_feeds = preferences.borrow_and_update().saved_feeds.clone();
                            let (agent, tx) = (agent.clone(), tx.clone());
                            tokio::spawn(async move {
                                update(&agent, &saved_feeds, &tx).await;
                            });
                        } else {
                            break log::warn!("preferences channel closed");
                        }
                    }
                    _ = quit.recv() => {
                        break;
                    }
                }
            }
        });
        rx
    }
    fn unsubscribe(&self) {
        if let Err(e) = self.tx.send(()) {
            log::error!("failed to send quit: {e}");
        }
        self.preferences.unsubscribe();
    }
    fn refresh(&self) {
        self.preferences.refresh();
    }
}

async fn update(
    agent: &BskyAgent,
    saved_feeds: &[Object<SavedFeedData>],
    tx: &Sender<Vec<SavedFeed>>,
) {
    match collect_feeds(agent, saved_feeds).await {
        Ok(feeds) => {
            tx.send(feeds).ok();
        }
        Err(e) => {
            log::error!("failed to collect feeds: {e}");
        }
    }
}

async fn collect_feeds(
    agent: &BskyAgent,
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
