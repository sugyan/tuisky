use crate::types::Event;
use color_eyre::Result;
use crossterm::event::{Event as CrosstermEvent, EventStream, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute};
use futures_util::{FutureExt, StreamExt};
use ratatui::backend::Backend;
use ratatui::Terminal;
use std::io::{stdout, Write};
use std::ops::{Deref, DerefMut};
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use tokio::time;

pub fn io() -> impl Write {
    stdout()
}

pub struct Tui<B>
where
    B: Backend,
{
    terminal: Terminal<B>,
    task: Option<JoinHandle<()>>,
    event_tx: UnboundedSender<Event>,
    event_rx: UnboundedReceiver<Event>,
}

impl<B> Tui<B>
where
    B: Backend,
{
    pub fn new(terminal: Terminal<B>) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        Self {
            terminal,
            task: None,
            event_tx,
            event_rx,
        }
    }
    pub fn start(&mut self) -> Result<()> {
        init()?;
        let event_tx = self.event_tx.clone();
        self.task = Some(tokio::spawn(async move {
            let mut reader = EventStream::new();
            let mut tick_interval = time::interval(Duration::from_secs(1));
            let mut tick = 0;
            loop {
                let event = reader.next().fuse();
                let tick_tick = tick_interval.tick();
                tokio::select! {
                    e = event => Self::handle_crossterm_event(e, &event_tx),
                    _ = tick_tick => {
                        tick += 1;
                        if let Err(e) = event_tx.send(Event::Tick(tick)) {
                            log::error!("failed to send tick event: {e}");
                        }
                    },
                }
            }
        }));
        Ok(())
    }
    pub fn end(&mut self) -> Result<()> {
        restore()?;
        Ok(())
    }
    pub async fn next_event(&mut self) -> Option<Event> {
        self.event_rx.recv().await
    }
    fn handle_crossterm_event(
        event: Option<std::io::Result<CrosstermEvent>>,
        tx: &UnboundedSender<Event>,
    ) {
        match event {
            Some(Ok(event)) => match event {
                CrosstermEvent::Mouse(mouse) => {
                    tx.send(Event::Mouse(mouse)).unwrap();
                }
                CrosstermEvent::Key(key) if key.kind != KeyEventKind::Release => {
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

impl<B> Deref for Tui<B>
where
    B: Backend,
{
    type Target = Terminal<B>;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

impl<B> DerefMut for Tui<B>
where
    B: Backend,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terminal
    }
}

/// Initialize the terminal
fn init() -> Result<()> {
    execute!(io(), EnterAlternateScreen, cursor::Hide)?;
    enable_raw_mode()?;
    Ok(())
}

/// Restore the terminal to its original state
pub(crate) fn restore() -> Result<()> {
    execute!(io(), LeaveAlternateScreen, cursor::Show)?;
    disable_raw_mode()?;
    Ok(())
}
