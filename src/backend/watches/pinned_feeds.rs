use super::super::types::{FeedSourceInfo, PinnedFeed};
use super::super::{Watch, Watcher};
use bsky_sdk::api::app::bsky::actor::defs::SavedFeed;
use bsky_sdk::preference::Preferences;
use bsky_sdk::{BskyAgent, Result};
use futures_util::future;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::watch::Sender;
use tokio::sync::{broadcast, watch};

impl Watcher {
    pub fn pinned_feeds(&self) -> impl Watch<Output = Vec<PinnedFeed>> {
        let (tx, _) = broadcast::channel(1);
        PinnedFeedsWatcher {
            agent: self.agent.clone(),
            preferences: self.preferences(),
            tx,
        }
    }
}

pub struct PinnedFeedsWatcher<W> {
    agent: Arc<BskyAgent>,
    preferences: W,
    tx: broadcast::Sender<()>,
}

impl<W> Watch for PinnedFeedsWatcher<W>
where
    W: Watch<Output = Preferences>,
{
    type Output = Vec<PinnedFeed>;

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

async fn update(agent: &BskyAgent, saved_feeds: &[SavedFeed], tx: &Sender<Vec<PinnedFeed>>) {
    match collect_feeds(agent, saved_feeds).await {
        Ok(feeds) => {
            tx.send(feeds).ok();
        }
        Err(e) => {
            log::error!("failed to collect feeds: {e}");
        }
    }
}

async fn collect_feeds(agent: &BskyAgent, saved_feeds: &[SavedFeed]) -> Result<Vec<PinnedFeed>> {
    let (mut feeds, mut lists) = (Vec::new(), Vec::new());
    for feed in saved_feeds.iter().filter(|feed| feed.pinned) {
        match feed.r#type.as_str() {
            "feed" => feeds.push(feed.value.clone()),
            "list" => lists.push(feed.value.clone()),
            _ => {}
        }
    }
    let mut resolved = HashMap::new();
    // resolve feeds
    if !feeds.is_empty() {
        for feed_generator in agent
            .api
            .app
            .bsky
            .feed
            .get_feed_generators(
                bsky_sdk::api::app::bsky::feed::get_feed_generators::ParametersData { feeds }
                    .into(),
            )
            .await?
            .data
            .feeds
        {
            resolved.insert(
                feed_generator.uri.clone(),
                FeedSourceInfo::Feed(Box::new(feed_generator)),
            );
        }
    }
    // resolve lists
    let mut handles = Vec::new();
    for list in lists {
        let agent = agent.clone();
        handles.push(tokio::spawn(async move {
            agent
                .api
                .app
                .bsky
                .graph
                .get_list(
                    bsky_sdk::api::app::bsky::graph::get_list::ParametersData {
                        cursor: None,
                        limit: 1.try_into().ok(),
                        list,
                    }
                    .into(),
                )
                .await
        }));
    }
    for result in future::join_all(handles).await.into_iter().flatten() {
        let list_view = result?.data.list;
        resolved.insert(
            list_view.data.uri.clone(),
            FeedSourceInfo::List(Box::new(list_view)),
        );
    }

    let mut ret = Vec::new();
    for saved_feed in saved_feeds {
        match saved_feed.r#type.as_str() {
            "feed" | "list" => {
                if let Some(info) = resolved.remove(&saved_feed.value) {
                    ret.push(PinnedFeed {
                        saved_feed: saved_feed.clone(),
                        info,
                    });
                }
            }
            "timeline" => {
                ret.push(PinnedFeed {
                    saved_feed: saved_feed.clone(),
                    info: FeedSourceInfo::Timeline(saved_feed.value.clone()),
                });
            }
            _ => {}
        }
    }
    Ok(ret)
}
