use super::super::views::types::Action as ViewsAction;
use super::embed_images::EmbedImagesModalComponent;
use super::embed_record::EmbedRecordModalComponent;
use super::types::{Data, EmbedData, ImageData};
use super::{Action, ModalComponent};
use bsky_sdk::api::com::atproto::repo::strong_ref;
use color_eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, BorderType, Clear, List, ListState, Padding};
use tokio::sync::mpsc::UnboundedSender;

pub struct EmbedModalComponent {
    action_tx: UnboundedSender<ViewsAction>,
    embeds_state: ListState,
    actions_state: ListState,
    record: Option<strong_ref::Main>,
    images: Vec<ImageData>,
    child: Option<Box<dyn ModalComponent>>,
}

impl EmbedModalComponent {
    pub fn new(action_tx: UnboundedSender<ViewsAction>, init: Option<EmbedData>) -> Self {
        let (record, images) = if let Some(data) = init {
            (data.record, data.images)
        } else {
            (None, Vec::new())
        };
        Self {
            action_tx,
            embeds_state: Default::default(),
            actions_state: Default::default(),
            record,
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
                    match *data {
                        Data::Image((image, index)) => {
                            if let Some(i) = index {
                                self.images[i] = image;
                            } else {
                                self.images.push(image)
                            }
                        }
                        Data::Record(strong_ref) => {
                            self.record = Some(strong_ref);
                        }
                        _ => {
                            // TODO
                        }
                    }
                    self.child = None;
                    Some(Action::Render)
                }
                Some(Action::Delete(index)) => {
                    match index {
                        Some(i) => {
                            self.images.remove(i);
                        }
                        None => self.record = None,
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
                        if i == usize::from(self.record.is_some()) + self.images.len() - 1 {
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
                        if usize::from(self.record.is_some()) + self.images.len() > 0 {
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
                    Some(0) if self.record.is_some() => {
                        if let Some(record) = &self.record {
                            self.child = Some(Box::new(EmbedRecordModalComponent::new(
                                self.action_tx.clone(),
                                Some(record.uri.clone()),
                            )));
                        }
                    }
                    Some(i) => {
                        let i = i - usize::from(self.record.is_some());
                        self.child = Some(Box::new(EmbedImagesModalComponent::new(Some((
                            i,
                            self.images[i].clone(),
                        )))));
                    }
                    None => {}
                }
                match self.actions_state.selected() {
                    Some(0) if self.images.len() < 4 => {
                        self.child = Some(Box::new(EmbedImagesModalComponent::new(None)));
                    }
                    Some(1) => {
                        // TODO: Add external
                    }
                    Some(2) => {
                        self.child = Some(Box::new(EmbedRecordModalComponent::new(
                            self.action_tx.clone(),
                            None,
                        )));
                    }
                    Some(3) => {
                        return Ok(Some(Action::Ok(Box::new(Data::Embed(EmbedData {
                            images: self.images.clone(),
                            record: self.record.clone(),
                        })))));
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
        let [area] = Layout::vertical([Constraint::Max(2 + 2 * 4 + 2 + 3 + 1 + 2)]).areas(area);

        let block = Block::bordered().title("Embed");
        let inner = block.inner(area);
        f.render_widget(Clear, area);
        f.render_widget(block, area);

        let [embeds, actions] = Layout::vertical([
            Constraint::Length(
                2 + 2 * u16::from(self.record.is_some()) + 2 * self.images.len() as u16,
            ),
            Constraint::Length(4),
        ])
        .areas(inner);

        let mut embed_items = Vec::new();
        if let Some(record) = &self.record {
            embed_items.push(Text::from(vec![
                Line::from(format!("record: {}", record.uri)),
                Line::from(format!("  {}", record.cid.as_ref())).dim(),
            ]));
        }
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
                Line::from("Add external").dim(),
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
