use {super::config::Config, bsky_sdk::BskyAgent, std::sync::Arc, tokio::sync::watch};

pub trait Watch {
    type Output;

    fn subscribe(&self) -> watch::Receiver<Self::Output>;
    fn unsubscribe(&self);
    fn refresh(&self);
}

pub struct Watcher {
    pub agent: Arc<BskyAgent>,
    pub(crate) config: Config,
}

impl Watcher {
    pub fn new(agent: Arc<BskyAgent>, config: Config) -> Self {
        Self { agent, config }
    }
}
