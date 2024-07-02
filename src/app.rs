use crate::components::main::MainComponent;
use crate::components::Component;
use crate::config::Config;
use crate::tui::{io, Tui};
use crate::types::{Action, Event};
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::Block;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tui_logger::{TuiLoggerLevelOutput, TuiLoggerWidget};

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

        let terminal = Terminal::new(CrosstermBackend::new(io()))?;
        log::debug!("terminal size: {}", terminal.size()?);
        let mut tui = Tui::new(terminal);
        tui.start(if self.config.dev { Some(10.0) } else { None })?;

        let auto_num = usize::from(tui.size()?.width - if self.config.dev { 75 } else { 0 }) / 80;
        let mut main_component = MainComponent::new(
            self.config
                .num_columns
                .map_or(auto_num, |n| n.min(auto_num)),
            action_tx.clone(),
        );

        main_component.register_action_handler(action_tx.clone())?;
        for component in self.components.iter_mut() {
            component.register_action_handler(action_tx.clone())?;
        }
        // TODO: config handler?
        main_component.init(tui.size()?)?;
        for component in self.components.iter_mut() {
            component.init(tui.size()?)?;
        }

        let mut constraints = vec![Constraint::Percentage(100)];
        if self.config.dev {
            constraints.push(Constraint::Min(75));
        }
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
                            // split horizontally, the right side is for log view
                            let layout = Layout::horizontal(&constraints).split(f.size());
                            // render main components to the left side
                            if let Err(e) = main_component.draw(f, layout[0]) {
                                if let Err(e) =
                                    action_tx.send(Action::Error(format!("failed to draw: {e:?}")))
                                {
                                    log::error!("failed to send error: {e}");
                                }
                            }
                            // render log components to the right side
                            if self.config.dev {
                                f.render_widget(
                                    TuiLoggerWidget::default()
                                        .block(Block::bordered().title("log"))
                                        .output_level(Some(TuiLoggerLevelOutput::Abbreviated))
                                        .style_error(Style::default().fg(Color::Red))
                                        .style_warn(Style::default().fg(Color::Yellow))
                                        .style_info(Style::default().fg(Color::Green))
                                        .style_debug(Style::default().fg(Color::Gray)),
                                    layout[1],
                                );
                            }
                            // other components?
                            for component in self.components.iter_mut() {
                                if let Err(e) = component.draw(f, layout[0]) {
                                    if let Err(e) = action_tx
                                        .send(Action::Error(format!("failed to draw: {e:?}")))
                                    {
                                        log::error!("failed to send error: {e}");
                                    }
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
        if matches!(key_event.code, KeyCode::Char('c' | 'q'))
            && key_event.modifiers == KeyModifiers::CONTROL
        {
            return Some(Action::Quit);
        }
        None
    }
}
