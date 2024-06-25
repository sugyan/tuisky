use bsky_sdk::preference::Preferences;
use tokio::sync::watch::{self, Receiver, Sender};

#[derive(Debug, Clone)]
pub enum Event {
    Preferences(Preferences),
}

pub struct DataWatcher<T> {
    tx: Sender<T>,
    // rx: Receiver<Event>,
}

impl<T> DataWatcher<T> {
    pub fn new(init: T) -> Self {
        let (tx, _) = watch::channel(init);
        Self { tx }
    }
    pub fn subscribe(&self) -> Receiver<T> {
        self.tx.subscribe()
    }
    // pub fn start(&self) {
    //     let tx = self.tx.clone();
    // }
}
