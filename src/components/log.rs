use super::Component;
use color_eyre::Result;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Block;
use ratatui::Frame;
use tui_logger::TuiLoggerWidget;

pub struct LogComponent;

impl Component for LogComponent {
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        f.render_widget(
            TuiLoggerWidget::default()
                .block(Block::bordered().title("log"))
                .style_error(Style::default().fg(Color::Red))
                .style_warn(Style::default().fg(Color::Yellow))
                .style_info(Style::default().fg(Color::Green)),
            area,
        );
        Ok(())
    }
}
