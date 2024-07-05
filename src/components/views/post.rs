use super::types::Transition;
use super::{types::Action, ViewComponent};
use crate::backend::Watcher;
use bsky_sdk::api::app::bsky::feed::defs::{
    PostView, PostViewEmbedRefs, ReplyRef, ReplyRefParentRefs,
};
use bsky_sdk::api::records::{KnownRecord, Record};
use bsky_sdk::api::types::Union;
use chrono::Local;
use color_eyre::Result;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Padding, Paragraph, Row, Table, TableState};
use ratatui::Frame;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

pub struct PostViewComponent {
    post_view: PostView,
    reply: Option<ReplyRef>,
    state: TableState,
}

impl PostViewComponent {
    pub fn new(
        action_tx: UnboundedSender<Action>,
        watcher: Arc<Watcher>,
        post_view: PostView,
        reply: Option<ReplyRef>,
    ) -> Self {
        Self {
            post_view,
            reply,
            state: TableState::default(),
        }
    }
    fn post_view_rows(post_view: &PostView, width: u16) -> Option<Vec<Row>> {
        let Record::Known(KnownRecord::AppBskyFeedPost(record)) = &post_view.record else {
            return None;
        };
        let mut author_lines = vec![Line::from(post_view.author.handle.as_str())];
        if let Some(display_name) = &post_view.author.display_name {
            author_lines.push(Line::from(display_name.as_str()).bold());
        }
        let text_lines = textwrap::wrap(&record.text, usize::from(width));
        let mut rows = vec![
            Row::new(vec![
                Cell::from("CID:".gray().into_right_aligned_line()),
                Cell::from(post_view.cid.as_ref().to_string()),
            ]),
            Row::new(vec![
                Cell::from("IndexedAt:".gray().into_right_aligned_line()),
                Cell::from(
                    post_view
                        .indexed_at
                        .as_ref()
                        .with_timezone(&Local)
                        .format("%Y-%m-%d %H:%M:%S %z")
                        .to_string(),
                )
                .green(),
            ]),
            Row::default().height(author_lines.len() as u16).cells(vec![
                Cell::from("Author:".gray().into_right_aligned_line()),
                Cell::from(Text::from(author_lines)),
            ]),
            Row::new(vec![
                Cell::from("Counts:".gray().into_right_aligned_line()),
                Cell::from(Line::from(vec![
                    Span::from(post_view.reply_count.unwrap_or_default().to_string()),
                    Span::from(" replies, ").dim(),
                    Span::from(post_view.repost_count.unwrap_or_default().to_string()),
                    Span::from(" reposts, ").dim(),
                    Span::from(post_view.like_count.unwrap_or_default().to_string()),
                    Span::from(" likes").dim(),
                ])),
            ]),
            Row::default().height(text_lines.len() as u16).cells(vec![
                Cell::from("Text:".gray().into_right_aligned_line()),
                Cell::from(
                    text_lines
                        .iter()
                        .map(|s| Line::from(s.to_string()))
                        .collect::<Vec<_>>(),
                ),
            ]),
        ];
        if let Some(labels) = &post_view.labels {
            rows.push(Row::default().height(labels.len() as u16).cells(vec![
                Cell::from("Labels:".gray().into_right_aligned_line()),
                Cell::from(labels.iter().map(|l| Line::from(l.val.as_str())).collect::<Vec<_>>()),
            ]));
        }
        if let Some(embed) = &post_view.embed {
            let mut lines = Vec::new();
            match embed {
                Union::Refs(PostViewEmbedRefs::AppBskyEmbedImagesView(images)) => {
                    lines.push(Line::from("images").yellow());
                    lines.extend(images.images.iter().map(|image| {
                        Line::from(vec![
                            Span::from(format!("[{}](", image.alt)),
                            Span::from(image.fullsize.as_str()).underlined(),
                            Span::from(")"),
                        ])
                    }));
                }
                Union::Refs(PostViewEmbedRefs::AppBskyEmbedExternalView(external)) => {
                    lines.push(Line::from("external").yellow());
                    lines.extend([
                        Line::from(
                            Span::from(external.external.uri.as_str())
                                .dim()
                                .underlined(),
                        ),
                        Line::from(external.external.title.as_str()).bold(),
                        Line::from(external.external.description.as_str()),
                    ]);
                }
                Union::Refs(PostViewEmbedRefs::AppBskyEmbedRecordView(record)) => {
                    lines.push(Line::from("record").yellow());
                    // TODO
                }
                Union::Refs(PostViewEmbedRefs::AppBskyEmbedRecordWithMediaView(
                    record_with_media,
                )) => {
                    lines.push(Line::from("recordWithMedia").yellow());
                    // TODO
                }
                _ => {}
            }
            rows.push(Row::default().height(lines.len() as u16).cells(vec![
                Cell::from("Embed:".gray().into_right_aligned_line()),
                Cell::from(lines),
            ]))
        }
        Some(rows)
    }
}

impl ViewComponent for PostViewComponent {
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::NextItem | Action::PrevItem => {}
            Action::Back => {
                return Ok(Some(Action::Transition(Transition::Pop)));
            }
            _ => {}
        }
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let widths = [Constraint::Length(11), Constraint::Percentage(100)];
        let width = Layout::horizontal(widths).split(area.inner(&Margin::new(1, 0)))[1].width;

        let mut rows = Vec::new();
        if let Some(reply) = &self.reply {
            if let Union::Refs(ReplyRefParentRefs::PostView(post_view)) = &reply.parent {
                if let Some(r) = Self::post_view_rows(post_view, width) {
                    rows.push(Row::new([" Reply to".blue()]));
                    rows.extend(r);
                    rows.push(Row::new([" --------- ".blue()]));
                }
            }
        }
        if let Some(r) = Self::post_view_rows(&self.post_view, width) {
            rows.extend(r);
        }

        let layout =
            Layout::vertical([Constraint::Length(2), Constraint::Percentage(100)]).split(area);
        f.render_widget(
            Paragraph::new(self.post_view.uri.as_str()).bold().block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Color::Gray)
                    .padding(Padding::horizontal(1)),
            ),
            layout[0],
        );
        f.render_stateful_widget(Table::new(rows, widths), layout[1], &mut self.state);
        Ok(())
    }
}
