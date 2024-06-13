use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, List, ListState, Paragraph, StatefulWidget, Widget};
use ratatui::{buffer::Buffer, Frame};
use std::io::Result;

use crate::tui::Tui;

#[derive(Debug, Default)]
pub struct App {
    list: StatefulList,
    exit: bool,
}

#[derive(Debug, Default)]
struct StatefulList {
    state: ListState,
    items: Vec<String>,
}

impl App {
    pub fn run(&mut self, terminal: &mut Tui) -> Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.render_frame(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }
    fn render_frame(&mut self, frame: &mut Frame) {
        frame.render_widget(self, frame.size());
    }
    fn handle_events(&mut self) -> Result<()> {
        if let Event::Key(key_event) = event::read()? {
            if key_event.kind == KeyEventKind::Press {
                self.handle_key_event(key_event);
            }
        }
        Ok(())
    }
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('a') => self
                .list
                .items
                .push(format!("item {:02}", self.list.items.len())),
            KeyCode::Char('q' | 'Q') => self.exit = true,
            KeyCode::Down => self.list.state.select(if self.list.items.is_empty() {
                None
            } else {
                Some(
                    self.list
                        .state
                        .selected()
                        .map_or(0, |selected| (selected + 1) % self.list.items.len()),
                )
            }),
            KeyCode::Up => self.list.state.select(if self.list.items.is_empty() {
                None
            } else {
                Some(self.list.state.selected().map_or(0, |selected| {
                    (selected + (self.list.items.len() - 1)) % self.list.items.len()
                }))
            }),
            KeyCode::Char('p') => panic!(),
            _ => {}
        }
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);
        StatefulWidget::render(
            self.list
                .items
                .iter()
                .map(String::as_str)
                .collect::<List>()
                .block(Block::bordered())
                .style(Style::default())
                .highlight_style(Style::default().fg(Color::Black).bg(Color::Gray)),
            layout[0],
            buf,
            &mut self.list.state,
        );
        Widget::render(
            Paragraph::new("right").block(Block::bordered()),
            layout[1],
            buf,
        );
    }
}
