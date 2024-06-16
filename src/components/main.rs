use super::Component;
use color_eyre::Result;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::Block;
use ratatui::Frame;
use tui_logger::TuiLoggerWidget;

pub struct MainComponent {}

impl Component for MainComponent {
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let layout = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(f.size());
        f.render_widget(
            TuiLoggerWidget::default()
                .block(Block::bordered().title("log"))
                .style_error(Style::default().fg(Color::Red))
                .style_warn(Style::default().fg(Color::Yellow))
                .style_info(Style::default().fg(Color::Green)),
            layout[1],
        );
        Ok(())
    }
}
