use super::super::views::types::Action as ViewsAction;
use super::embed_images::EmbedImagesModalComponent;
use super::types::{Data, EmbedData, ImageData};
use super::{Action, ModalComponent};
use color_eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, BorderType, Clear, List, ListState, Padding};
use ratatui::Frame;

pub struct EmbedModalComponent {
    embeds_state: ListState,
    actions_state: ListState,
    images: Vec<ImageData>,
    child: Option<Box<dyn ModalComponent>>,
}

impl EmbedModalComponent {
    pub fn new(init: Option<EmbedData>) -> Self {
        let images = if let Some(data) = init {
            data.images
        } else {
            Vec::new()
        };
        Self {
            embeds_state: Default::default(),
            actions_state: Default::default(),
            images,
            child: None,
        }
    }
}

impl ModalComponent for EmbedModalComponent {
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        if let Some(child) = self.child.as_mut() {
            child.handle_key_events(key)
        } else {
            Ok(None)
        }
    }
    fn update(&mut self, action: ViewsAction) -> Result<Option<Action>> {
        if let Some(child) = self.child.as_mut() {
            return Ok(match child.update(action)? {
                Some(Action::Ok(data)) => {
                    match data {
                        Data::Image((image, index)) => {
                            if let Some(i) = index {
                                self.images[i] = image;
                            } else {
                                self.images.push(image)
                            }
                        }
                        _ => {
                            // TODO
                        }
                    }
                    self.child = None;
                    Some(Action::Render)
                }
                Some(Action::Delete(index)) => {
                    if let Some(i) = index {
                        self.images.remove(i);
                    }
                    self.child = None;
                    self.embeds_state.select(None);
                    Some(Action::Render)
                }
                Some(Action::Cancel) => {
                    self.child = None;
                    Some(Action::Render)
                }
                action => action,
            });
        }
        Ok(match action {
            ViewsAction::NextItem => {
                match (self.embeds_state.selected(), self.actions_state.selected()) {
                    (Some(i), None) => {
                        if i == self.images.len() - 1 {
                            self.embeds_state.select(None);
                            self.actions_state.select_first();
                        } else {
                            self.embeds_state.select_next();
                        }
                    }
                    (None, Some(i)) => {
                        self.actions_state.select(Some((i + 1).min(3)));
                    }
                    _ => {
                        self.actions_state.select_first();
                    }
                }
                Some(Action::Render)
            }
            ViewsAction::PrevItem => {
                match (self.embeds_state.selected(), self.actions_state.selected()) {
                    (Some(i), None) => {
                        self.embeds_state.select(Some(i.max(1) - 1));
                    }
                    (None, Some(0)) => {
                        if !self.images.is_empty() {
                            self.actions_state.select(None);
                            self.embeds_state.select_last();
                        }
                    }
                    (None, Some(i)) => {
                        self.actions_state.select(Some(i - 1));
                    }
                    _ => {
                        self.actions_state.select_last();
                    }
                }
                Some(Action::Render)
            }
            ViewsAction::Enter => {
                match self.embeds_state.selected() {
                    Some(i) => {
                        self.child = Some(Box::new(EmbedImagesModalComponent::new(Some((
                            i,
                            self.images[i].clone(),
                        )))));
                    }
                    _ => {
                        // TODO
                    }
                }
                match self.actions_state.selected() {
                    Some(0) if self.images.len() < 4 => {
                        self.child = Some(Box::new(EmbedImagesModalComponent::new(None)));
                    }
                    Some(1) => {
                        // Add external
                    }
                    Some(2) => {
                        // Add record
                    }
                    Some(3) => {
                        return Ok(Some(Action::Ok(Data::Embed(EmbedData {
                            images: self.images.clone(),
                        }))));
                    }
                    _ => {}
                }
                Some(Action::Render)
            }
            ViewsAction::Back => Some(Action::Cancel),
            _ => None,
        })
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let area = area.inner(Margin {
            horizontal: 2,
            vertical: 4,
        });
        let [area] = Layout::vertical([Constraint::Max(2 * 4 + 2 + 3 + 1 + 2)]).areas(area);

        let block = Block::bordered().title("Embed");
        let inner = block.inner(area);
        f.render_widget(Clear, area);
        f.render_widget(block, area);

        let [embeds, actions] = Layout::vertical([
            Constraint::Length(2 + 2 * self.images.len() as u16),
            Constraint::Length(4),
        ])
        .areas(inner);

        let mut embed_items = Vec::new();
        for (i, image) in self.images.iter().enumerate() {
            embed_items.push(Text::from(vec![
                Line::from(format!("image{}: {}", i + 1, image.path)),
                Line::from(format!("  {}", image.alt)).dim(),
            ]));
        }
        f.render_stateful_widget(
            List::new(embed_items)
                .block(
                    Block::bordered()
                        .border_type(BorderType::Rounded)
                        .border_style(Color::Yellow),
                )
                .highlight_style(Style::reset().reversed()),
            embeds,
            &mut self.embeds_state,
        );
        f.render_stateful_widget(
            List::new([
                Line::from("Add images"),
                Line::from("Add external"),
                Line::from("Add record"),
                Line::from("OK").centered().blue(),
            ])
            .block(Block::default().padding(Padding::horizontal(1)))
            .highlight_style(Style::default().reversed()),
            actions,
            &mut self.actions_state,
        );
        if let Some(child) = self.child.as_mut() {
            child.draw(f, area)?;
        }
        Ok(())
    }
}
