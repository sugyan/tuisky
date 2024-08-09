use super::super::views::types::Action as ViewsAction;
use super::types::{Action, Data, ImageData};
use super::ModalComponent;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use image::ImageReader;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear};
use ratatui::Frame;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use tui_textarea::TextArea;

pub struct Image {
    pub path: TextArea<'static>,
    pub alt: TextArea<'static>,
}

impl Default for Image {
    fn default() -> Self {
        let mut path = TextArea::default();
        path.set_block(Block::bordered().title("Path"));
        path.set_cursor_line_style(Style::default());
        let mut alt = TextArea::default();
        alt.set_block(Block::bordered().title("Alt").dim());
        alt.set_cursor_line_style(Style::default());
        alt.set_cursor_style(Style::default());
        Self { path, alt }
    }
}

enum Focus {
    Path,
    Alt,
    Ok,
}

impl Focus {
    fn next(&self) -> Self {
        match self {
            Self::Path => Self::Alt,
            Self::Alt => Self::Ok,
            Self::Ok => Self::Ok,
        }
    }
    fn prev(&self) -> Self {
        match self {
            Self::Path => Self::Path,
            Self::Alt => Self::Path,
            Self::Ok => Self::Alt,
        }
    }
}

enum State {
    None,
    Ok,
    Error,
}

pub struct EmbedImagesModalComponent {
    image: Image,
    focus: Focus,
    state: State,
}

impl EmbedImagesModalComponent {
    pub fn new() -> Self {
        Self {
            image: Image::default(),
            focus: Focus::Path,
            state: State::None,
        }
    }
    fn current_textarea(&mut self) -> Option<&mut TextArea<'static>> {
        match self.focus {
            Focus::Path => Some(&mut self.image.path),
            Focus::Alt => Some(&mut self.image.alt),
            _ => None,
        }
    }
    fn update_focus(&mut self, focus: Focus) {
        if let Some(curr) = self.current_textarea() {
            curr.set_cursor_style(Style::default());
            if let Some(block) = curr.block() {
                curr.set_block(block.clone().dim());
            }
        }
        self.focus = focus;
        if let Some(curr) = self.current_textarea() {
            curr.set_cursor_style(Style::default().reversed());
            if let Some(block) = curr.block() {
                curr.set_block(block.clone().reset());
            }
        }
    }
}

impl ModalComponent for EmbedImagesModalComponent {
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        match self.focus {
            Focus::Path => {
                if matches!(
                    (key.code, key.modifiers),
                    (KeyCode::Enter, _) | (KeyCode::Char('m'), KeyModifiers::CONTROL)
                ) {
                    return Ok(None);
                }
                let cursor = self.image.path.cursor();
                return Ok(if self.image.path.input(key) {
                    if let Some(block) = self.image.path.block() {
                        let block = block.clone();
                        let path = PathBuf::from(self.image.path.lines().join(""));
                        self.state = if let Ok(metadata) = path.metadata() {
                            if metadata.is_file()
                                && metadata.size() <= 1_000_000
                                && ImageReader::open(path)
                                    .ok()
                                    .and_then(|reader| reader.decode().ok())
                                    .is_some()
                            {
                                State::Ok
                            } else {
                                State::Error
                            }
                        } else {
                            State::None
                        };
                        self.image.path.set_block(match self.state {
                            State::None => block.border_style(Color::Reset),
                            State::Ok => block.border_style(Color::Green),
                            State::Error => block.border_style(Color::Red),
                        });
                    }
                    Some(Action::Render)
                } else if self.image.path.cursor() != cursor {
                    Some(Action::Render)
                } else {
                    None
                });
            }
            Focus::Alt => {
                let cursor = self.image.alt.cursor();
                return Ok(
                    if self.image.alt.input(key) || self.image.alt.cursor() != cursor {
                        Some(Action::Render)
                    } else {
                        None
                    },
                );
            }
            _ => {}
        }
        Ok(None)
    }
    fn update(&mut self, action: ViewsAction) -> Result<Option<Action>> {
        Ok(match action {
            ViewsAction::NextItem => {
                self.update_focus(self.focus.next());
                Some(Action::Render)
            }
            ViewsAction::PrevItem => {
                self.update_focus(self.focus.prev());
                Some(Action::Render)
            }
            ViewsAction::Enter if matches!(self.focus, Focus::Ok) => match self.state {
                State::Ok => Some(Action::Ok(Data::Image(ImageData {
                    path: self.image.path.lines().join(""),
                    alt: self.image.alt.lines().join("\n"),
                }))),
                _ => None,
            },
            ViewsAction::Enter => self.update(ViewsAction::NextItem)?,
            ViewsAction::Back => Some(Action::Cancel),
            _ => None,
        })
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let area = area.inner(Margin {
            horizontal: 2,
            vertical: 1,
        });
        let [area] = Layout::vertical([Constraint::Length(10)]).areas(area);

        let block = Block::bordered().title("Embed image");
        let inner = block.inner(area);
        f.render_widget(Clear, area);
        f.render_widget(block, area);

        let [path, alt, ok] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Length(1),
        ])
        .areas(inner);
        let mut line = Line::from("OK").centered();
        if matches!(self.state, State::Ok) {
            line = line.blue();
        } else {
            line = line.dim();
        }
        if matches!(self.focus, Focus::Ok) {
            line = line.reversed();
        }
        f.render_widget(self.image.path.widget(), path);
        f.render_widget(self.image.alt.widget(), alt);
        f.render_widget(line, ok);
        Ok(())
    }
}
