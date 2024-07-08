use super::types::{Transition, View};
use super::utils::{profile_name, profile_name_as_str};
use super::{types::Action, ViewComponent};
use crate::backend::{PostThreadWatcher, Watcher};
use crate::components::views::types::Data;
use bsky_sdk::api::app::bsky::actor::defs::ProfileViewBasic;
use bsky_sdk::api::app::bsky::embed::record::{self, ViewRecordRefs};
use bsky_sdk::api::app::bsky::embed::record_with_media::ViewMediaRefs;
use bsky_sdk::api::app::bsky::embed::{external, images};
use bsky_sdk::api::app::bsky::feed::defs::{
    PostView, PostViewData, PostViewEmbedRefs, ThreadViewPostParentRefs, ViewerStateData,
};
use bsky_sdk::api::app::bsky::feed::get_post_thread::OutputThreadRefs;
use bsky_sdk::api::records::{KnownRecord, Record};
use bsky_sdk::api::types::string::Datetime;
use bsky_sdk::api::types::{Collection, Union};
use bsky_sdk::{api, BskyAgent};
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
    Unlike(String),
    Open(String),
    ViewRecord(record::ViewRecord),
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
            PostAction::Like => Self::from("Like"),
            PostAction::Unlike(_) => Self::from("Unlike"),
            PostAction::Open(uri) => Self::from(format!("Open {uri}")),
            PostAction::ViewRecord(view_record) => Self::from(Line::from(vec![
                Span::from("Show "),
                Span::from("embedded record").yellow(),
                Span::from(" "),
                Span::from(view_record.uri.as_str()).underlined(),
            ])),
        }
    }
}

pub struct PostViewComponent {
    post_view: PostView,
    reply: Option<PostView>,
    actions: Vec<PostAction>,
    table_state: TableState,
    list_state: ListState,
    action_tx: UnboundedSender<Action>,
    agent: Arc<BskyAgent>,
    watcher: Arc<PostThreadWatcher>,
}

impl PostViewComponent {
    pub fn new(
        action_tx: UnboundedSender<Action>,
        watcher: Arc<Watcher>,
        post_view: PostView,
        reply: Option<PostView>,
    ) -> Self {
        let actions = Self::post_view_actions(&post_view);
        let agent = watcher.agent.clone();
        let watcher = Arc::new(watcher.post_thread(post_view.uri.clone()));
        Self {
            post_view,
            reply,
            actions,
            table_state: TableState::default(),
            list_state: ListState::default(),
            action_tx,
            agent,
            watcher,
        }
    }
    fn post_view_actions(post_view: &PostView) -> Vec<PostAction> {
        let mut liked = None;
        if let Some(viewer) = &post_view.viewer {
            liked = viewer.like.as_ref();
        }
        let mut actions = vec![
            PostAction::Profile(post_view.author.clone()),
            PostAction::Reply,
            PostAction::Repost,
            if let Some(uri) = liked {
                PostAction::Unlike(uri.clone())
            } else {
                PostAction::Like
            },
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
        actions
    }
    fn record_actions(record: &record::View) -> Vec<PostAction> {
        let mut actions = Vec::new();
        match &record.record {
            Union::Refs(ViewRecordRefs::ViewRecord(view_record)) => {
                actions.push(PostAction::ViewRecord(view_record.as_ref().clone()));
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
        let mut liked = false;
        if let Some(viewer) = &post_view.viewer {
            if viewer.like.is_some() {
                liked = true;
            }
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
                    Span::from(post_view.like_count.unwrap_or_default().to_string()).style(
                        if liked {
                            Style::default().fg(Color::Red)
                        } else {
                            Style::default()
                        },
                    ),
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
    fn activate(&mut self) -> Result<()> {
        let (tx, mut rx) = (self.action_tx.clone(), self.watcher.subscribe());
        tokio::spawn(async move {
            while rx.changed().await.is_ok() {
                if let Err(e) = tx.send(Action::Update(Box::new(Data::PostThread(
                    rx.borrow_and_update().clone(),
                )))) {
                    log::error!("failed to send update action: {e}");
                }
            }
            log::debug!("subscription finished");
        });
        Ok(())
    }
    fn deactivate(&mut self) -> Result<()> {
        self.watcher.unsubscribe();
        Ok(())
    }
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
            Action::Enter => {
                if let Some(action) = self.list_state.selected().and_then(|i| self.actions.get(i)) {
                    match action {
                        PostAction::Like => {
                            let (agent, tx) = (self.agent.clone(), self.action_tx.clone());
                            let mut viewer = self.post_view.viewer.clone().unwrap_or(
                                ViewerStateData {
                                    like: None,
                                    reply_disabled: None,
                                    repost: None,
                                    thread_muted: None,
                                }
                                .into(),
                            );
                            let record = Record::Known(KnownRecord::AppBskyFeedLike(Box::new(
                                api::app::bsky::feed::like::RecordData {
                                    created_at: Datetime::now(),
                                    subject: api::com::atproto::repo::strong_ref::MainData {
                                        cid: self.post_view.cid.clone(),
                                        uri: self.post_view.uri.clone(),
                                    }
                                    .into(),
                                }
                                .into(),
                            )));
                            tokio::spawn(async move {
                                let Some(session) = agent.get_session().await else {
                                    return;
                                };
                                match agent
                                    .api
                                    .com
                                    .atproto
                                    .repo
                                    .create_record(
                                        api::com::atproto::repo::create_record::InputData {
                                            collection: api::app::bsky::feed::Like::nsid(),
                                            record,
                                            repo: session.data.did.into(),
                                            rkey: None,
                                            swap_commit: None,
                                            validate: None,
                                        }
                                        .into(),
                                    )
                                    .await
                                {
                                    Ok(output) => {
                                        log::info!("created like record: {}", output.cid.as_ref());
                                        viewer.like = Some(output.uri.clone());
                                        tx.send(Action::Update(Box::new(Data::ViewerState(Some(
                                            viewer,
                                        )))))
                                        .ok();
                                    }
                                    Err(e) => {
                                        log::error!("failed to create like record: {e}");
                                    }
                                }
                            });
                        }
                        PostAction::Unlike(uri) => {
                            let (agent, tx) = (self.agent.clone(), self.action_tx.clone());
                            let mut viewer = self.post_view.viewer.clone();
                            let Some(rkey) = uri.split('/').last().map(String::from) else {
                                log::error!("failed to get rkey from uri: {uri}");
                                return Ok(None);
                            };
                            tokio::spawn(async move {
                                let Some(session) = agent.get_session().await else {
                                    return;
                                };
                                match agent
                                    .api
                                    .com
                                    .atproto
                                    .repo
                                    .delete_record(
                                        api::com::atproto::repo::delete_record::InputData {
                                            collection: api::app::bsky::feed::Like::nsid(),
                                            repo: session.data.did.into(),
                                            rkey,
                                            swap_commit: None,
                                            swap_record: None,
                                        }
                                        .into(),
                                    )
                                    .await
                                {
                                    Ok(_) => {
                                        log::info!("deleted like record");
                                        if let Some(viewer) = viewer.as_mut() {
                                            viewer.like = None;
                                        }
                                        tx.send(Action::Update(Box::new(Data::ViewerState(
                                            viewer,
                                        ))))
                                        .ok();
                                    }
                                    Err(e) => {
                                        log::error!("failed to create like record: {e}");
                                    }
                                }
                            });
                        }
                        PostAction::Open(uri) => {
                            if let Err(e) = open::that(uri) {
                                log::error!("failed to open: {e}");
                            }
                        }
                        PostAction::ViewRecord(view_record) => {
                            return Ok(Some(Action::Transition(Transition::Push(Box::new(
                                View::Post(Box::new((
                                    PostViewData {
                                        author: view_record.author.clone(),
                                        cid: view_record.cid.clone(),
                                        embed: None,
                                        indexed_at: view_record.indexed_at.clone(),
                                        labels: view_record.labels.clone(),
                                        like_count: view_record.like_count,
                                        record: view_record.value.clone(),
                                        reply_count: view_record.reply_count,
                                        repost_count: view_record.repost_count,
                                        threadgate: None,
                                        uri: view_record.uri.clone(),
                                        viewer: None,
                                    }
                                    .into(),
                                    None,
                                ))),
                            )))));
                        }
                        _ => {
                            // TODO
                        }
                    }
                }
            }
            Action::Back => {
                return Ok(Some(Action::Transition(Transition::Pop)));
            }
            Action::Update(data) => {
                match data.as_ref() {
                    Data::PostThread(Union::Refs(
                        OutputThreadRefs::AppBskyFeedDefsThreadViewPost(thread_view),
                    )) => {
                        self.post_view = thread_view.post.clone();
                        if let Some(Union::Refs(ThreadViewPostParentRefs::ThreadViewPost(parent))) =
                            &thread_view.parent
                        {
                            self.reply = Some(parent.post.clone());
                        }
                    }
                    Data::ViewerState(viewer) => {
                        let diff = i64::from(
                            viewer
                                .as_ref()
                                .map(|v| v.like.is_some())
                                .unwrap_or_default(),
                        ) - i64::from(
                            self.post_view
                                .viewer
                                .as_ref()
                                .map(|v| v.like.is_some())
                                .unwrap_or_default(),
                        );
                        self.post_view.like_count =
                            Some(self.post_view.like_count.unwrap_or_default() + diff);
                        self.post_view.viewer.clone_from(viewer);
                    }
                    _ => return Ok(None),
                }
                self.actions = Self::post_view_actions(&self.post_view);
                return Ok(Some(Action::Render));
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
            if let Some(r) = Self::post_view_rows(reply, width) {
                rows.push(Row::new([" Reply to".blue()]));
                rows.extend(r);
                rows.push(Row::new([" --------- ".blue()]));
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
