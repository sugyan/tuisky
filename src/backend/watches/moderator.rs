use super::super::{Watch, Watcher};
use bsky_sdk::moderation::Moderator;
use bsky_sdk::preference::Preferences;
use tokio::sync::{broadcast, watch};

pub struct ModeratorWatcher<W>
where
    W: Watch<Output = Preferences>,
{
    preferences: W,
    tx: broadcast::Sender<()>,
}

impl<W> Watch for ModeratorWatcher<W>
where
    W: Watch<Output = Preferences>,
{
    type Output = Moderator;

    fn subscribe(&self) -> watch::Receiver<Self::Output> {
        let (tx, rx) = watch::channel(Moderator::new(None, Default::default(), Default::default()));
        let mut quit = self.tx.subscribe();
        let mut preferences = self.preferences.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    changed = preferences.changed() => {
                        log::debug!("preferences changed");
                    }
                    _ = quit.recv() => {
                        return log::debug!("quit");
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
    }
    fn refresh(&self) {
        self.preferences.refresh();
    }
}

impl Watcher {
    pub fn moderator(&self) -> impl Watch<Output = Moderator> {
        let (tx, _) = broadcast::channel(1);
        ModeratorWatcher {
            preferences: self.preferences(),
            tx,
        }
    }
}
