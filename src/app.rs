use crate::action::Action;
use crate::event::Event;
use crate::tui::{io, Tui};
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

#[derive(Debug, Default)]
pub struct App {}

impl App {
    pub fn new() -> Self {
        Self::default()
    }
    pub async fn run(&mut self) -> Result<()> {
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        let terminal = Terminal::new(CrosstermBackend::new(io()))?;
        let mut tui = Tui::new(terminal);
        let mut events = tui.start()?;
        let mut should_quit = false;
        loop {
            if let Some(e) = events.next().await {
                match e {
                    Event::Tick => action_tx.send(Action::Tick)?,
                    Event::Key(key_event) => match key_event.code {
                        KeyCode::Char('p') => action_tx.send(Action::Panic)?,
                        KeyCode::Char('q') => action_tx.send(Action::Quit)?,
                        _ => {}
                    },
                    Event::Mouse(_) => {}
                    _ => {}
                }
            }
            while let Ok(action) = action_rx.try_recv() {
                match action {
                    Action::Quit => should_quit = true,
                    Action::Panic => panic!(),
                    Action::Tick => {}
                }
            }
            if should_quit {
                break;
            }
        }
        tui.end()?;
        Ok(())
    }
    // fn render_frame(&mut self, frame: &mut Frame) {
    //     frame.render_widget(self, frame.size());
    // }
    // fn handle_events(&mut self) -> Result<()> {
    //     if let Event::Key(key_event) = event::read()? {
    //         if key_event.kind == KeyEventKind::Press {
    //             self.handle_key_event(key_event);
    //         }
    //     }
    //     Ok(())
    // }
    // fn handle_key_event(&mut self, key_event: KeyEvent) {
    //     match key_event.code {
    //         KeyCode::Char('a') => self
    //             .list
    //             .items
    //             .push(format!("item {:02}", self.list.items.len())),
    //         KeyCode::Down => self.list.state.select(if self.list.items.is_empty() {
    //             None
    //         } else {
    //             Some(
    //                 self.list
    //                     .state
    //                     .selected()
    //                     .map_or(0, |selected| (selected + 1) % self.list.items.len()),
    //             )
    //         }),
    //         KeyCode::Up => self.list.state.select(if self.list.items.is_empty() {
    //             None
    //         } else {
    //             Some(self.list.state.selected().map_or(0, |selected| {
    //                 (selected + (self.list.items.len() - 1)) % self.list.items.len()
    //             }))
    //         }),
    //         KeyCode::Char('p') => panic!(),
    //         _ => {}
    //     }
    // }
}

// impl Widget for &mut App {
//     fn render(self, area: Rect, buf: &mut Buffer) {
//         let layout = Layout::default()
//             .direction(ratatui::layout::Direction::Horizontal)
//             .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
//             .split(area);
//         StatefulWidget::render(
//             self.list
//                 .items
//                 .iter()
//                 .map(String::as_str)
//                 .collect::<List>()
//                 .block(Block::bordered())
//                 .style(Style::default())
//                 .highlight_style(Style::default().fg(Color::Black).bg(Color::Gray)),
//             layout[0],
//             buf,
//             &mut self.list.state,
//         );
//         Widget::render(
//             Paragraph::new("right").block(Block::bordered()),
//             layout[1],
//             buf,
//         );
//     }
// }
