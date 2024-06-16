use crate::components::main::MainComponent;
use crate::components::Component;
use crate::tui::{io, Tui};
use crate::types::{Action, Event};
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

pub struct App {
    frame_rate: f64,
    components: Vec<Box<dyn Component>>,
}

impl App {
    pub fn new(frame_rate: f64) -> Self {
        log::debug!("App::new with frame_rate: {frame_rate}");
        Self {
            frame_rate,
            components: vec![Box::new(MainComponent {})],
        }
    }
    pub async fn run(&mut self) -> Result<()> {
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        let terminal = Terminal::new(CrosstermBackend::new(io()))?;
        let mut tui = Tui::new(terminal);
        tui.start(self.frame_rate)?;

        for component in self.components.iter_mut() {
            component.register_action_handler(action_tx.clone())?;
        }
        // TODO: config?
        for component in self.components.iter_mut() {
            component.init(tui.size()?)?;
        }

        let mut should_quit = false;
        loop {
            if let Some(e) = tui.next_event().await {
                if let Some(action) = self.handle_events(e.clone()) {
                    action_tx.send(action)?;
                }
                for component in self.components.iter_mut() {
                    if let Some(action) = component.handle_events(Some(e.clone()))? {
                        action_tx.send(action)?;
                    }
                }
            }
            while let Ok(action) = action_rx.try_recv() {
                if !matches!(action, Action::Tick | Action::Render) {
                    log::info!("Action {action:?}");
                }
                match action {
                    Action::Quit => should_quit = true,
                    Action::Tick => {}
                    Action::Render => {
                        tui.draw(|f| {
                            for component in self.components.iter_mut() {
                                if let Err(e) = component.draw(f, f.size()) {
                                    action_tx
                                        .send(Action::Error(format!("failed to draw: {e:?}")))
                                        .expect("failed to send error");
                                }
                            }
                        })?;
                    }
                    _ => {}
                }
                for component in self.components.iter_mut() {
                    if let Some(action) = component.update(action.clone())? {
                        action_tx.send(action)?;
                    }
                }
            }
            if should_quit {
                break;
            }
        }
        tui.end()?;
        Ok(())
    }
    fn handle_events(&mut self, event: Event) -> Option<Action> {
        match event {
            Event::Tick => return Some(Action::Tick),
            Event::Render => return Some(Action::Render),
            Event::Key(key_event) => {
                log::debug!("Key {:?}", (key_event.code, key_event.modifiers));
                if let Some(action) = self.handle_key_events(key_event) {
                    return Some(action);
                }
            }
            _ => {}
        }
        None
    }
    fn handle_key_events(&mut self, key_event: KeyEvent) -> Option<Action> {
        match (key_event.code, key_event.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Some(Action::Quit),
            (KeyCode::Char('n'), KeyModifiers::CONTROL) => return Some(Action::NextItem),
            (KeyCode::Char('p'), KeyModifiers::CONTROL) => return Some(Action::PrevItem),
            _ => {}
        }
        None
    }
}
