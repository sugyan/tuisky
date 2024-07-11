use super::super::{Watch, Watcher};
use bsky_sdk::api::app::bsky::feed::defs::NotFoundPostData;
use bsky_sdk::api::app::bsky::feed::get_post_thread::OutputThreadRefs;
use bsky_sdk::api::types::Union;
use bsky_sdk::preference::Preferences;
use bsky_sdk::{BskyAgent, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, watch};
use tokio::time;

impl Watcher {
    pub fn post_thread(&self, uri: String) -> impl Watch<Output = Union<OutputThreadRefs>> {
        let (tx, _) = broadcast::channel(1);
        PostThreadWatcher {
            uri,
            agent: self.agent.clone(),
            preferences: self.preferences(),
            period: Duration::from_secs(self.config.intervals.post_thread),
            tx,
        }
    }
}

pub struct PostThreadWatcher<W> {
    uri: String,
    agent: Arc<BskyAgent>,
    preferences: W,
    period: Duration,
    tx: broadcast::Sender<()>,
}

impl<W> Watch for PostThreadWatcher<W>
where
    W: Watch<Output = Preferences>,
{
    type Output = Union<OutputThreadRefs>;

    fn subscribe(&self) -> watch::Receiver<Union<OutputThreadRefs>> {
        let init = Union::Refs(OutputThreadRefs::AppBskyFeedDefsNotFoundPost(Box::new(
            NotFoundPostData {
                not_found: true,
                uri: String::new(),
            }
            .into(),
        )));
        let (watch_tx, watch_rx) = watch::channel(init.clone());
        let (agent, uri) = (self.agent.clone(), self.uri.clone());
        let (mut preferences, mut quit) = (self.preferences.subscribe(), self.tx.subscribe());
        let mut interval = time::interval(self.period);
        tokio::spawn(async move {
            loop {
                let (agent, uri, watch_tx) = (agent.clone(), uri.clone(), watch_tx.clone());
                let tick = interval.tick();
                tokio::select! {
                    changed = preferences.changed() => {
                        if changed.is_ok() {
                            tokio::spawn(async move {
                                update(&agent, &uri, &watch_tx).await;
                            });
                        } else {
                            break log::warn!("preferences channel closed");
                        }
                    }
                    _ = tick => {
                        let (agent, uri, watch_tx) = (agent.clone(), uri.clone(), watch_tx.clone());
                        tokio::spawn(async move {
                            update(&agent, &uri, &watch_tx).await;
                        });
                    }
                    _ = quit.recv() => {
                        break;
                    }
                }
            }
        });
        watch_rx
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

async fn update(agent: &Arc<BskyAgent>, uri: &str, tx: &watch::Sender<Union<OutputThreadRefs>>) {
    match get_post_thread(agent, uri).await {
        Ok(thread) => {
            if let Err(e) = tx.send(thread.clone()) {
                log::warn!("failed to send post thread: {e}");
            }
        }
        Err(e) => {
            log::warn!("failed to get post thread: {e}");
        }
    }
}

async fn get_post_thread(agent: &Arc<BskyAgent>, uri: &str) -> Result<Union<OutputThreadRefs>> {
    Ok(agent
        .api
        .app
        .bsky
        .feed
        .get_post_thread(
            bsky_sdk::api::app::bsky::feed::get_post_thread::ParametersData {
                depth: 10.try_into().ok(),
                parent_height: None,
                uri: uri.into(),
            }
            .into(),
        )
        .await?
        .data
        .thread)
}
