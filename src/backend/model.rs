use bsky_sdk::preference::Preferences;
use bsky_sdk::{agent::config::Config, BskyAgent};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch::{self, Sender};
use tokio::time;

pub struct Manager {
    agent: Arc<BskyAgent>,
    views: Vec<View>,
    pub data: Data,
}

#[derive(Debug)]
pub struct Data {
    pub preferences: Sender<Preferences>,
}

impl Default for Data {
    fn default() -> Self {
        let (tx, _) = watch::channel(Preferences::default());
        Self { preferences: tx }
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
        let tx = self.data.preferences.clone();
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(1));
            let mut preferences_interval = time::interval(Duration::from_secs(60));
            loop {
                let tick = interval.tick();
                let preferences_tick = preferences_interval.tick();
                tokio::select! {
                    _ = tick => {
                        log::debug!("manager tick");
                    }
                    _ = preferences_tick => {
                        if let Ok(preferences) = agent.get_preferences(true).await  {
                            log::debug!("got preferences");
                            if let Err(e) = tx.send(preferences) {
                                log::error!("failed to send preferences event: {e}");
                            }
                        } else {
                            log::error!("failed to get preferences");
                        }
                    }
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
}

pub enum View {}
