use super::types::Transition;
use super::utils::{profile_name, profile_name_as_str};
use super::{types::Action, ViewComponent};
use crate::backend::Watcher;
use bsky_sdk::api::app::bsky::actor::defs::ProfileViewBasic;
use bsky_sdk::api::app::bsky::embed::record::{self, ViewRecordRefs};
use bsky_sdk::api::app::bsky::embed::record_with_media::ViewMediaRefs;
use bsky_sdk::api::app::bsky::embed::{external, images};
use bsky_sdk::api::app::bsky::feed::defs::{
    PostView, PostViewEmbedRefs, ReplyRef, ReplyRefParentRefs,
};
use bsky_sdk::api::records::{KnownRecord, Record};
use bsky_sdk::api::types::Union;
use chrono::Local;
use color_eyre::Result;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, Borders, Cell, List, ListItem, ListState, Padding, Paragraph, Row, Table, TableState,
};
use ratatui::Frame;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

enum PostAction {
    Profile(ProfileViewBasic),
    Reply,
    Repost,
    Like,
    Open(String),
}

impl<'a> From<&'a PostAction> for ListItem<'a> {
    fn from(action: &'a PostAction) -> Self {
        match action {
            PostAction::Profile(profile) => Self::from(Line::from(vec![
                Span::from("Show "),
                Span::from(profile_name_as_str(profile)).bold(),
                Span::from("'s profile"),
            ]))
            .dim(),
            PostAction::Reply => Self::from("Reply").dim(),
            PostAction::Repost => Self::from("Repost").dim(),
            PostAction::Like => Self::from("Like").dim(),
            PostAction::Open(uri) => Self::from(format!("Open {uri}")),
        }
    }
}

pub struct PostViewComponent {
    post_view: PostView,
    reply: Option<ReplyRef>,
    actions: Vec<PostAction>,
    table_state: TableState,
    list_state: ListState,
}

impl PostViewComponent {
    pub fn new(
        action_tx: UnboundedSender<Action>,
        watcher: Arc<Watcher>,
        post_view: PostView,
        reply: Option<ReplyRef>,
    ) -> Self {
        let mut actions = vec![
            PostAction::Profile(post_view.author.clone()),
            PostAction::Reply,
            PostAction::Repost,
            PostAction::Like,
        ];
        if let Some(embed) = &post_view.embed {
            match embed {
                Union::Refs(PostViewEmbedRefs::AppBskyEmbedImagesView(images)) => {
                    for image in &images.images {
                        actions.push(PostAction::Open(image.fullsize.clone()));
                    }
                }
                Union::Refs(PostViewEmbedRefs::AppBskyEmbedExternalView(external)) => {
                    actions.push(PostAction::Open(external.external.uri.clone()));
                }
                Union::Refs(PostViewEmbedRefs::AppBskyEmbedRecordView(record)) => {
                    actions.extend(Self::record_actions(record));
                }
                Union::Refs(PostViewEmbedRefs::AppBskyEmbedRecordWithMediaView(
                    record_with_media,
                )) => {
                    match &record_with_media.media {
                        Union::Refs(ViewMediaRefs::AppBskyEmbedImagesView(images)) => {
                            for image in &images.images {
                                actions.push(PostAction::Open(image.fullsize.clone()));
                            }
                        }
                        Union::Refs(ViewMediaRefs::AppBskyEmbedExternalView(external)) => {
                            actions.push(PostAction::Open(external.external.uri.clone()));
                        }
                        _ => {}
                    }
                    actions.extend(Self::record_actions(&record_with_media.record));
                }
                _ => {}
            }
        }
        Self {
            post_view,
            reply,
            actions,
            table_state: TableState::default(),
            list_state: ListState::default(),
        }
    }
    fn record_actions(record: &record::View) -> Vec<PostAction> {
        let mut actions = Vec::new();
        match &record.record {
            Union::Refs(ViewRecordRefs::ViewRecord(record)) => {
                // TODO
            }
            Union::Refs(ViewRecordRefs::AppBskyFeedDefsGeneratorView(_)) => {
                // TODO
            }
            Union::Refs(ViewRecordRefs::AppBskyGraphDefsListView(_)) => {
                // TODO
            }
            Union::Refs(ViewRecordRefs::AppBskyLabelerDefsLabelerView(_)) => {
                // TODO
            }
            Union::Refs(ViewRecordRefs::AppBskyGraphDefsStarterPackViewBasic(_)) => {
                // TODO
            }
            _ => {}
        }
        actions
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
                    lines.extend(Self::images_lines(images))
                }
                Union::Refs(PostViewEmbedRefs::AppBskyEmbedExternalView(external)) => {
                    lines.push(Line::from("external").yellow());
                    lines.extend(Self::external_lines(external));
                }
                Union::Refs(PostViewEmbedRefs::AppBskyEmbedRecordView(record)) => {
                    lines.push(Line::from("record").yellow());
                    lines.extend(Self::record_lines(record, width));
                }
                Union::Refs(PostViewEmbedRefs::AppBskyEmbedRecordWithMediaView(
                    record_with_media,
                )) => {
                    lines.push(Line::from("recordWithMedia").yellow());
                    match &record_with_media.media {
                        Union::Refs(ViewMediaRefs::AppBskyEmbedImagesView(images)) => {
                            lines.extend(Self::images_lines(images))
                        }
                        Union::Refs(ViewMediaRefs::AppBskyEmbedExternalView(external)) => {
                            lines.extend(Self::external_lines(external));
                        }
                        _ => {}
                    }
                    lines.extend(Self::record_lines(&record_with_media.record, width));
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
    fn images_lines(images: &images::View) -> Vec<Line> {
        images
            .images
            .iter()
            .map(|image| {
                Line::from(vec![
                    Span::from(format!("[{}](", image.alt)),
                    Span::from(image.fullsize.as_str()).underlined(),
                    Span::from(")"),
                ])
            })
            .collect()
    }
    fn external_lines(external: &external::View) -> Vec<Line> {
        vec![
            Line::from(
                Span::from(external.external.uri.as_str())
                    .dim()
                    .underlined(),
            ),
            Line::from(external.external.title.as_str()).bold(),
            Line::from(external.external.description.as_str()),
        ]
    }
    fn record_lines(record: &record::View, width: u16) -> Vec<Line> {
        match &record.record {
            Union::Refs(ViewRecordRefs::ViewRecord(view_record)) => {
                if let Record::Known(KnownRecord::AppBskyFeedPost(record)) = &view_record.value {
                    return [
                        vec![
                            Line::from(
                                view_record
                                    .indexed_at
                                    .as_ref()
                                    .with_timezone(&Local)
                                    .format("%Y-%m-%d %H:%M:%S %z")
                                    .to_string(),
                            )
                            .green(),
                            Line::from(profile_name(&view_record.author)),
                        ],
                        textwrap::wrap(&record.text, usize::from(width))
                            .iter()
                            .map(|s| Line::from(s.to_string()))
                            .collect::<Vec<_>>(),
                    ]
                    .concat();
                }
            }
            Union::Refs(ViewRecordRefs::AppBskyFeedDefsGeneratorView(_)) => {
                // TODO
            }
            Union::Refs(ViewRecordRefs::AppBskyGraphDefsListView(_)) => {
                // TODO
            }
            Union::Refs(ViewRecordRefs::AppBskyLabelerDefsLabelerView(_)) => {
                // TODO
            }
            Union::Refs(ViewRecordRefs::AppBskyGraphDefsStarterPackViewBasic(_)) => {
                // TODO
            }
            _ => {}
        }
        Vec::new()
    }
}

impl ViewComponent for PostViewComponent {
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::NextItem => self.list_state.select(Some(
                self.list_state
                    .selected()
                    .map_or(0, |s| (s + 1) % self.actions.len()),
            )),
            Action::PrevItem => self.list_state.select(Some(
                self.list_state
                    .selected()
                    .map_or(0, |s| (s + self.actions.len() - 1) % self.actions.len()),
            )),
            Action::Back => {
                return Ok(Some(Action::Transition(Transition::Pop)));
            }
            Action::Enter => {
                if let Some(action) = self.list_state.selected().and_then(|i| self.actions.get(i)) {
                    match action {
                        PostAction::Like => {
                            // TODO
                        }
                        PostAction::Open(uri) => {
                            if let Err(e) = open::that(uri) {
                                log::error!("failed to open: {e}");
                            }
                        }
                        _ => {
                            // TODO
                        }
                    }
                }
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
        self.table_state.select(Some(rows.len()));
        if let Some(r) = Self::post_view_rows(&self.post_view, width) {
            rows.extend(r);
        }

        let layout = Layout::vertical([
            Constraint::Length(2),
            Constraint::Percentage(100),
            Constraint::Min(10),
        ])
        .split(area);
        f.render_widget(
            Paragraph::new(self.post_view.uri.as_str()).bold().block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Color::Gray)
                    .padding(Padding::horizontal(1)),
            ),
            layout[0],
        );
        f.render_stateful_widget(Table::new(rows, widths), layout[1], &mut self.table_state);
        f.render_stateful_widget(
            List::new(&self.actions)
                .highlight_style(Style::default().reversed())
                .block(
                    Block::default()
                        .title("Actions")
                        .title_alignment(Alignment::Center)
                        .borders(Borders::TOP)
                        .border_style(Color::Gray)
                        .padding(Padding::horizontal(1)),
                ),
            layout[2],
            &mut self.list_state,
        );
        Ok(())
    }
}
