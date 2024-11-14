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
use std::path::PathBuf;
use tui_textarea::TextArea;

pub struct Image {
    pub path: TextArea<'static>,
    pub alt: TextArea<'static>,
}

enum Focus {
    Path,
    Alt,
    Ok,
    Delete,
}

impl Focus {
    fn next(&self, delete: bool) -> Self {
        match self {
            Self::Path => Self::Alt,
            Self::Alt => Self::Ok,
            Self::Ok if delete => Self::Delete,
            Self::Ok => Self::Ok,
            Self::Delete => Self::Delete,
        }
    }
    fn prev(&self, _: bool) -> Self {
        match self {
            Self::Path => Self::Path,
            Self::Alt => Self::Path,
            Self::Ok => Self::Alt,
            Self::Delete => Self::Ok,
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
    index: Option<usize>,
}

impl EmbedImagesModalComponent {
    pub fn new(init: Option<(usize, ImageData)>) -> Self {
        let (mut path, mut alt) = if let Some((_, init)) = &init {
            (
                TextArea::new(vec![init.path.clone()]),
                TextArea::new(init.alt.lines().map(String::from).collect()),
            )
        } else {
            (TextArea::default(), TextArea::default())
        };
        path.set_block(Block::bordered().title("Path"));
        path.set_cursor_line_style(Style::default());
        alt.set_block(Block::bordered().title("Alt").dim());
        alt.set_cursor_line_style(Style::default());
        alt.set_cursor_style(Style::default());
        let image = Image { path, alt };

        let mut ret = Self {
            image,
            focus: Focus::Path,
            state: State::None,
            index: init.map(|(i, _)| i),
        };
        ret.check_path();
        ret
    }
    fn check_path(&mut self) {
        if let Some(block) = self.image.path.block() {
            let block = block.clone();
            let path = PathBuf::from(self.image.path.lines().join(""));
            self.state = if let Ok(metadata) = path.metadata() {
                if metadata.is_file()
                    && metadata.len() <= 1_000_000
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
                    self.check_path();
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
                self.update_focus(self.focus.next(self.index.is_some()));
                Some(Action::Render)
            }
            ViewsAction::PrevItem => {
                self.update_focus(self.focus.prev(self.index.is_some()));
                Some(Action::Render)
            }
            ViewsAction::Enter => match self.focus {
                Focus::Ok => {
                    if let State::Ok = self.state {
                        Some(Action::Ok(Data::Image((
                            ImageData {
                                path: self.image.path.lines().join(""),
                                alt: self.image.alt.lines().join("\n"),
                            },
                            self.index,
                        ))))
                    } else {
                        None
                    }
                }
                Focus::Delete => Some(Action::Delete(self.index)),
                _ => self.update(ViewsAction::NextItem)?,
            },
            ViewsAction::Back => Some(Action::Cancel),
            _ => None,
        })
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let area = area.inner(Margin {
            horizontal: 2,
            vertical: 1,
        });
        let [area] = Layout::vertical([Constraint::Max(11)]).areas(area);

        let block = Block::bordered().title("Embed image");
        let inner = block.inner(area);
        f.render_widget(Clear, area);
        f.render_widget(block, area);

        let mut constraints = vec![
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Length(1),
        ];
        if self.index.is_some() {
            constraints.push(Constraint::Length(1));
        }
        let layout = Layout::vertical(constraints).split(inner);
        let mut line = Line::from("OK").centered();
        line = match self.state {
            State::Ok => line.blue(),
            _ => line.dim(),
        };
        if let Focus::Ok = self.focus {
            line = line.reversed();
        }
        f.render_widget(&self.image.path, layout[0]);
        f.render_widget(&self.image.alt, layout[1]);
        f.render_widget(line, layout[2]);
        if let Some(area) = layout.get(3) {
            f.render_widget(
                Line::from("Delete")
                    .centered()
                    .red()
                    .patch_style(match self.focus {
                        Focus::Delete => Style::default().reversed(),
                        _ => Style::default(),
                    }),
                *area,
            )
        }
        Ok(())
    }
}
