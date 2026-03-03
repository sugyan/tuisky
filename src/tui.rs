use crate::types::Event;
use color_eyre::Result;
use crossterm::event::{poll, read, Event as CrosstermEvent, KeyEventKind};
use ratatui::crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::crossterm::{cursor, execute};
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
            let mut tick_interval = time::interval(Duration::from_secs(1));
            let mut tick = 0;
            loop {
                let poll_future = tokio::task::spawn_blocking(|| {
                    poll(Duration::from_millis(100))
                });
                let tick_tick = tick_interval.tick();
                tokio::select! {
                    result = poll_future => {
                        match result {
                            Ok(Ok(true)) => {
                                // Read the event
                                if let Ok(event) = read() {
                                    Self::handle_crossterm_event(Some(Ok(event)), &event_tx);
                                }
                            }
                            Ok(Ok(false)) => {} // No event available
                            Ok(Err(e)) => {
                                event_tx.send(Event::Error(e.to_string())).unwrap();
                            }
                            Err(e) => {
                                log::error!("Failed to poll events: {e}");
                            }
                        }
                    }
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
    #[cfg(not(windows))]
    pub fn suspend(&mut self) -> Result<()> {
        restore()?;
        signal_hook::low_level::raise(signal_hook::consts::signal::SIGTSTP)?;
        Ok(())
    }
    #[cfg(not(windows))]
    pub fn clear(&mut self) -> Result<()> {
        self.terminal.clear().map_err(|e| color_eyre::eyre::eyre!("{e}"))?;
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
                CrosstermEvent::Key(key) if key.kind != KeyEventKind::Release => {
                    // Convert crossterm 0.28 KeyEvent to ratatui::crossterm 0.29 KeyEvent
                    let ratatui_key = ratatui::crossterm::event::KeyEvent {
                        code: match key.code {
                            crossterm::event::KeyCode::Char(c) => ratatui::crossterm::event::KeyCode::Char(c),
                            crossterm::event::KeyCode::Backspace => ratatui::crossterm::event::KeyCode::Backspace,
                            crossterm::event::KeyCode::Enter => ratatui::crossterm::event::KeyCode::Enter,
                            crossterm::event::KeyCode::Left => ratatui::crossterm::event::KeyCode::Left,
                            crossterm::event::KeyCode::Right => ratatui::crossterm::event::KeyCode::Right,
                            crossterm::event::KeyCode::Up => ratatui::crossterm::event::KeyCode::Up,
                            crossterm::event::KeyCode::Down => ratatui::crossterm::event::KeyCode::Down,
                            crossterm::event::KeyCode::Home => ratatui::crossterm::event::KeyCode::Home,
                            crossterm::event::KeyCode::End => ratatui::crossterm::event::KeyCode::End,
                            crossterm::event::KeyCode::PageUp => ratatui::crossterm::event::KeyCode::PageUp,
                            crossterm::event::KeyCode::PageDown => ratatui::crossterm::event::KeyCode::PageDown,
                            crossterm::event::KeyCode::Tab => ratatui::crossterm::event::KeyCode::Tab,
                            crossterm::event::KeyCode::BackTab => ratatui::crossterm::event::KeyCode::BackTab,
                            crossterm::event::KeyCode::Delete => ratatui::crossterm::event::KeyCode::Delete,
                            crossterm::event::KeyCode::Insert => ratatui::crossterm::event::KeyCode::Insert,
                            crossterm::event::KeyCode::F(n) => ratatui::crossterm::event::KeyCode::F(n),
                            crossterm::event::KeyCode::Null => ratatui::crossterm::event::KeyCode::Null,
                            crossterm::event::KeyCode::CapsLock => ratatui::crossterm::event::KeyCode::CapsLock,
                            crossterm::event::KeyCode::ScrollLock => ratatui::crossterm::event::KeyCode::ScrollLock,
                            crossterm::event::KeyCode::NumLock => ratatui::crossterm::event::KeyCode::NumLock,
                            crossterm::event::KeyCode::PrintScreen => ratatui::crossterm::event::KeyCode::PrintScreen,
                            crossterm::event::KeyCode::Pause => ratatui::crossterm::event::KeyCode::Pause,
                            crossterm::event::KeyCode::Menu => ratatui::crossterm::event::KeyCode::Menu,
                            crossterm::event::KeyCode::KeypadBegin => ratatui::crossterm::event::KeyCode::KeypadBegin,
                            crossterm::event::KeyCode::Esc => ratatui::crossterm::event::KeyCode::Esc,
                            crossterm::event::KeyCode::Media(_) => return, // Skip media keys
                            crossterm::event::KeyCode::Modifier(_) => return, // Skip modifier keys
                        },
                        modifiers: ratatui::crossterm::event::KeyModifiers::from_bits_retain(key.modifiers.bits()),
                        kind: match key.kind {
                            crossterm::event::KeyEventKind::Press => ratatui::crossterm::event::KeyEventKind::Press,
                            crossterm::event::KeyEventKind::Repeat => ratatui::crossterm::event::KeyEventKind::Repeat,
                            crossterm::event::KeyEventKind::Release => ratatui::crossterm::event::KeyEventKind::Release,
                        },
                        state: ratatui::crossterm::event::KeyEventState::NONE,
                    };
                    tx.send(Event::Key(ratatui_key)).unwrap();
                }
                _ => {
                    // TODO: Handle mouse events and other events
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
