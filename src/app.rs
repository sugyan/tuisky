use crate::components::main::MainComponent;
use crate::components::Component;
use crate::config::Config;
use crate::tui::{io, Tui};
use crate::types::{Action, Event};
use color_eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

pub struct App {
    config: Config,
    components: Vec<Box<dyn Component>>,
}

impl App {
    pub fn new(config: Config) -> Self {
        log::debug!("App::new({config:?})");
        Self {
            config,
            components: Vec::new(),
        }
    }
    pub async fn run(&mut self) -> Result<()> {
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        // Setup terminal
        let terminal = Terminal::new(CrosstermBackend::new(io()))?;
        log::debug!("terminal size: {}", terminal.size()?);
        let mut tui = Tui::new(terminal);
        tui.start()?;

        // Create main component
        let mut main_component = MainComponent::new(self.config.clone(), action_tx.clone());

        // Setup components
        main_component.register_action_handler(action_tx.clone())?;
        for component in self.components.iter_mut() {
            component.register_action_handler(action_tx.clone())?;
        }
        main_component.register_config_handler(self.config.clone())?;
        for component in self.components.iter_mut() {
            component.register_config_handler(self.config.clone())?;
        }
        main_component.init(tui.size()?)?;
        for component in self.components.iter_mut() {
            component.init(tui.size()?)?;
        }

        // Initial render
        action_tx.send(Action::Render)?;

        // Main loop
        let mut should_quit = false;
        loop {
            if let Some(e) = tui.next_event().await {
                if let Some(action) = self.handle_events(e.clone()) {
                    action_tx.send(action)?;
                }
                if let Some(action) = main_component.handle_events(Some(e.clone()))? {
                    action_tx.send(action)?;
                }
                for component in self.components.iter_mut() {
                    if let Some(action) = component.handle_events(Some(e.clone()))? {
                        action_tx.send(action)?;
                    }
                }
            }
            while let Ok(action) = action_rx.try_recv() {
                if !matches!(action, Action::Tick(_) | Action::Render) {
                    log::info!("Action {action:?}");
                }
                match action {
                    Action::Quit => should_quit = true,
                    Action::Tick(i) => {
                        // TODO
                        if i % 60 == 0 {
                            main_component.save().await?;
                        }
                    }
                    Action::Render => {
                        tui.draw(|f| {
                            // render main components to the left side
                            if let Err(e) = main_component.draw(f, f.size()) {
                                action_tx
                                    .send(Action::Error(format!("failed to draw: {e}")))
                                    .ok();
                            }
                            for component in self.components.iter_mut() {
                                if let Err(e) = component.draw(f, f.size()) {
                                    action_tx
                                        .send(Action::Error(format!("failed to draw: {e}")))
                                        .ok();
                                }
                            }
                        })?;
                    }
                    _ => {
                        if let Some(action) = main_component.update(action.clone())? {
                            action_tx.send(action)?;
                        }
                        for component in self.components.iter_mut() {
                            if let Some(action) = component.update(action.clone())? {
                                action_tx.send(action)?;
                            }
                        }
                    }
                }
            }
            if should_quit {
                break main_component.save().await?;
            }
        }
        tui.end()?;
        Ok(())
    }
    fn handle_events(&mut self, event: Event) -> Option<Action> {
        match event {
            Event::Tick(i) => return Some(Action::Tick(i)),
            Event::Render => return Some(Action::Render),
            Event::Key(key_event) => {
                if let Some(action) = self.handle_key_events(key_event) {
                    return Some(action);
                }
            }
            _ => {}
        }
        None
    }
    fn handle_key_events(&mut self, key_event: KeyEvent) -> Option<Action> {
        self.config
            .keybindings
            .global
            .get(&key_event.into())
            .map(Into::into)
    }
}
