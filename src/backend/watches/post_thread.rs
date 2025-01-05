use {
    super::super::{Watch, Watcher},
    bsky_sdk::{
        api::{
            app::bsky::feed::{defs::NotFoundPostData, get_post_thread::OutputThreadRefs},
            types::Union,
        },
        preference::Preferences,
        {BskyAgent, Result},
    },
    std::{sync::Arc, time::Duration},
    tokio::{
        sync::{broadcast, watch},
        time,
    },
};

impl Watcher {
    pub fn post_thread(&self, uri: String) -> impl Watch<Output = Union<OutputThreadRefs>> + use<> {
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
        let (tx, rx) = watch::channel(init);
        let updater = Updater {
            agent: self.agent.clone(),
            uri: self.uri.clone(),
            tx: tx.clone(),
        };
        let (mut preferences, mut quit) = (self.preferences.subscribe(), self.tx.subscribe());
        let mut interval = time::interval(self.period);
        tokio::spawn(async move {
            loop {
                let tick = interval.tick();
                tokio::select! {
                    changed = preferences.changed() => {
                        if changed.is_ok() {
                            let updater = updater.clone();
                            tokio::spawn(async move {
                                updater.update().await;
                            });
                        } else {
                            break log::warn!("preferences channel closed");
                        }
                    }
                    _ = tick => {
                        let updater = updater.clone();
                        tokio::spawn(async move {
                            updater.update().await;
                        });
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

#[derive(Clone)]
struct Updater {
    agent: Arc<BskyAgent>,
    uri: String,
    tx: watch::Sender<Union<OutputThreadRefs>>,
}

impl Updater {
    async fn update(&self) {
        match self.get_post_thread().await {
            Ok(thread) => {
                if let Err(e) = self.tx.send(thread.clone()) {
                    log::warn!("failed to send post thread: {e}");
                }
            }
            Err(e) => {
                log::warn!("failed to get post thread: {e}");
            }
        }
    }
    async fn get_post_thread(&self) -> Result<Union<OutputThreadRefs>> {
        Ok(self
            .agent
            .api
            .app
            .bsky
            .feed
            .get_post_thread(
                bsky_sdk::api::app::bsky::feed::get_post_thread::ParametersData {
                    depth: 10.try_into().ok(),
                    parent_height: None,
                    uri: self.uri.clone(),
                }
                .into(),
            )
            .await?
            .data
            .thread)
    }
}
