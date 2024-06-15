use crate::tui::{io, Tui};
use crate::types::{Action, Event};
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::Block;
use ratatui::{Frame, Terminal};
use tokio::sync::mpsc;
use tui_logger::TuiLoggerWidget;

#[derive(Debug)]
pub struct App {
    frame_rate: f64,
}

impl App {
    pub fn new(frame_rate: f64) -> Self {
        log::debug!("App::new with frame_rate: {frame_rate}");
        Self { frame_rate }
    }
    pub async fn run(&mut self) -> Result<()> {
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        let terminal = Terminal::new(CrosstermBackend::new(io()))?;
        let mut tui = Tui::new(terminal);
        tui.start(self.frame_rate)?;
        let mut should_quit = false;
        loop {
            if let Some(e) = tui.next_event().await {
                match e {
                    Event::Tick => action_tx.send(Action::Tick)?,
                    Event::Render => action_tx.send(Action::Render)?,
                    Event::Key(key_event) => {
                        log::debug!("Key {:?}", (key_event.code, key_event.modifiers));
                        if let Some(action) = self.handle_key_event(key_event) {
                            action_tx.send(action)?;
                        }
                    }
                    Event::Mouse(_) => {}
                    _ => {}
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
                        tui.draw(|frame| self.render_frame(frame))?;
                    }
                    _ => {}
                }
            }
            if should_quit {
                break;
            }
        }
        tui.end()?;
        Ok(())
    }
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Option<Action> {
        match (key_event.code, key_event.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Some(Action::Quit),
            (KeyCode::Char('n'), KeyModifiers::CONTROL) => return Some(Action::NextItem),
            (KeyCode::Char('p'), KeyModifiers::CONTROL) => return Some(Action::PrevItem),
            _ => {}
        }
        None
    }
    fn render_frame(&mut self, frame: &mut Frame) {
        let layout = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(frame.size());
        frame.render_widget(
            TuiLoggerWidget::default()
                .block(Block::bordered().title("log"))
                .style_error(Style::default().fg(Color::Red))
                .style_warn(Style::default().fg(Color::Yellow))
                .style_info(Style::default().fg(Color::Green)),
            layout[1],
        );
    }
}
