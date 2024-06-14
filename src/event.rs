use crossterm::event::{Event as CrosstermEvent, EventStream, KeyEvent, MouseEvent};
use futures_util::{FutureExt, StreamExt};
use std::io;
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use tokio::time;

#[derive(Debug, Clone)]
pub enum Event {
    Tick,
    Key(KeyEvent),
    Mouse(MouseEvent),
    Focus(CrosstermEvent),
    Error(String),
}

pub struct EventHandler {
    task: JoinHandle<()>,
    tx: UnboundedSender<Event>,
    rx: UnboundedReceiver<Event>,
}

impl EventHandler {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let event_tx = tx.clone();
        let task = tokio::spawn(async move {
            let mut reader = EventStream::new();
            let mut tick = time::interval(Duration::from_secs(1));
            loop {
                let event = reader.next().fuse();
                let tick = tick.tick();
                tokio::select! {
                    e = event => Self::handle_crossterm_event(e, &event_tx),
                    _ = tick => event_tx.send(Event::Tick).unwrap(),
                }
            }
        });
        Self { task, tx, rx }
    }
    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
    fn handle_crossterm_event(
        event: Option<io::Result<CrosstermEvent>>,
        tx: &UnboundedSender<Event>,
    ) {
        match event {
            Some(Ok(event)) => match event {
                CrosstermEvent::FocusGained | CrosstermEvent::FocusLost => {
                    tx.send(Event::Focus(event)).unwrap();
                }
                CrosstermEvent::Mouse(mouse) => {
                    tx.send(Event::Mouse(mouse)).unwrap();
                }
                CrosstermEvent::Key(key) => {
                    tx.send(Event::Key(key)).unwrap();
                }
                _ => {
                    // TODO
                }
            },
            Some(Err(err)) => {
                tx.send(Event::Error(err.to_string())).unwrap();
            }
            _ => {}
        }
    }
}
