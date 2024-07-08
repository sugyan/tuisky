use super::Watcher;
use bsky_sdk::api::app::bsky::feed::defs::NotFoundPostData;
use bsky_sdk::api::app::bsky::feed::get_post_thread::OutputThreadRefs;
use bsky_sdk::api::types::Union;
use bsky_sdk::{BskyAgent, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, watch};
use tokio::time;

#[derive(Debug, Clone)]
enum Command {
    Refresh,
    Quit,
}

pub struct PostThreadWatcher {
    agent: Arc<BskyAgent>,
    tx: broadcast::Sender<Command>,
    rx: broadcast::Receiver<Command>,
    period: Duration,
    uri: String,
}

impl PostThreadWatcher {
    pub fn subscribe(&self) -> watch::Receiver<Union<OutputThreadRefs>> {
        let init = Union::Refs(OutputThreadRefs::AppBskyFeedDefsNotFoundPost(Box::new(
            NotFoundPostData {
                not_found: true,
                uri: String::new(),
            }
            .into(),
        )));
        let (watch_tx, watch_rx) = watch::channel(init.clone());
        let (agent, uri) = (self.agent.clone(), self.uri.clone());
        let mut event_rx = self.rx.resubscribe();
        let mut interval = time::interval(self.period);
        tokio::spawn(async move {
            loop {
                let tick = interval.tick();
                tokio::select! {
                    Ok(command) = event_rx.recv() => {
                        match command {
                            Command::Refresh => {
                                let (agent, uri, watch_tx) = (agent.clone(), uri.clone(), watch_tx.clone());
                                tokio::spawn(async move {
                                    update(&agent, &uri, &watch_tx).await;
                                });
                            }
                            Command::Quit => {
                                return log::debug!("quit");
                            }
                        }
                    }
                    _ = tick => {
                        let (agent, uri, watch_tx) = (agent.clone(), uri.clone(), watch_tx.clone());
                        tokio::spawn(async move {
                            update(&agent, &uri, &watch_tx).await;
                        });
                    }
                    _ = watch_tx.closed() => {
                        return log::warn!("post thread channel closed");
                    }
                }
            }
        });
        watch_rx
    }
    pub fn refresh(&self) {
        if let Err(e) = self.tx.send(Command::Refresh) {
            log::warn!("failed to send post thread channel event: {e}");
        }
    }
    pub fn unsubscribe(&self) {
        if let Err(e) = self.tx.send(Command::Quit) {
            log::warn!("failed to send post thread channel event: {e}");
        }
    }
}

impl Watcher {
    pub fn post_thread(&self, uri: String) -> PostThreadWatcher {
        let (tx, rx) = broadcast::channel(1);
        let uri = uri.clone();
        PostThreadWatcher {
            agent: self.agent.clone(),
            tx,
            rx,
            period: Duration::from_secs(self.config.intervals.post_thread),
            uri,
        }
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
