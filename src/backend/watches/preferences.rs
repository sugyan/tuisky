use {
    super::super::{Watch, Watcher},
    bsky_sdk::{preference::Preferences, BskyAgent},
    std::{sync::Arc, time::Duration},
    tokio::{
        sync::{broadcast, watch},
        time,
    },
};

impl Watcher {
    pub fn preferences(&self) -> impl Watch<Output = Preferences> + use<> {
        let (tx, _) = broadcast::channel(1);
        PreferencesWatcher {
            agent: self.agent.clone(),
            period: Duration::from_secs(self.config.intervals.preferences),
            tx,
        }
    }
}

#[derive(Debug, Clone)]
enum Command {
    Quit,
    Refresh,
}

struct PreferencesWatcher {
    agent: Arc<BskyAgent>,
    period: Duration,
    tx: broadcast::Sender<Command>,
}

impl Watch for PreferencesWatcher {
    type Output = Preferences;

    fn subscribe(&self) -> watch::Receiver<Self::Output> {
        let agent = self.agent.clone();
        let mut command = self.tx.subscribe();
        let mut interval = time::interval(self.period);
        let (tx, rx) = watch::channel(Preferences::default());
        tokio::spawn(async move {
            loop {
                let tick = interval.tick();
                let (agent, tx) = (agent.clone(), tx.clone());
                tokio::select! {
                    Ok(command) = command.recv() => {
                        match command {
                            Command::Refresh => {
                                tokio::spawn(async move {
                                    update(&agent, &tx).await;
                                });
                            }
                            Command::Quit => {
                                break;
                            }
                        }
                    }
                    _ = tick => {
                        tokio::spawn(async move {
                            update(&agent, &tx).await;
                        });
                    }
                }
            }
        });
        rx
    }
    fn unsubscribe(&self) {
        if let Err(e) = self.tx.send(Command::Quit) {
            log::error!("failed to send quit command: {e}");
        }
    }
    fn refresh(&self) {
        if let Err(e) = self.tx.send(Command::Refresh) {
            log::error!("failed to send refresh command: {e}");
        }
    }
}

async fn update(agent: &BskyAgent, tx: &watch::Sender<Preferences>) {
    if let Ok(preferences) = agent.get_preferences(true).await {
        agent.configure_labelers_from_preferences(&preferences);
        tx.send(preferences).ok();
    }
}
