use super::types::{Action, Data, Transition, View};
use super::utils::{counts, profile_name, profile_name_as_str};
use super::ViewComponent;
use crate::backend::{Watch, Watcher};
use bsky_sdk::api::agent::Session;
use bsky_sdk::api::app::bsky::actor::defs::ProfileViewBasic;
use bsky_sdk::api::app::bsky::embed::record::{self, ViewRecordRefs};
use bsky_sdk::api::app::bsky::embed::record_with_media::ViewMediaRefs;
use bsky_sdk::api::app::bsky::embed::{external, images};
use bsky_sdk::api::app::bsky::feed::defs::{
    PostView, PostViewData, PostViewEmbedRefs, ThreadViewPostParentRefs, ViewerStateData,
};
use bsky_sdk::api::app::bsky::feed::get_post_thread::OutputThreadRefs;
use bsky_sdk::api::app::bsky::richtext::facet::MainFeaturesItem;
use bsky_sdk::api::records::{KnownRecord, Record};
use bsky_sdk::api::types::string::Datetime;
use bsky_sdk::api::types::Union;
use bsky_sdk::{api, BskyAgent};
use chrono::Local;
use color_eyre::Result;
use indexmap::IndexSet;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, Borders, Cell, List, ListItem, ListState, Padding, Paragraph, Row, Table, TableState,
};
use ratatui::Frame;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;

#[derive(Debug, Clone)]
enum PostAction {
    Profile(ProfileViewBasic),
    Reply,
    Repost,
    Like,
    Unlike(String),
    Delete,
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
            PostAction::Delete => Self::from("Delete").red(),
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
    watcher: Box<dyn Watch<Output = Union<OutputThreadRefs>>>,
    quit: Option<oneshot::Sender<()>>,
    session: Option<Session>,
}

impl PostViewComponent {
    pub fn new(
        action_tx: UnboundedSender<Action>,
        watcher: Arc<Watcher>,
        post_view: PostView,
        reply: Option<PostView>,
        session: Option<Session>,
    ) -> Self {
        let actions = Self::post_view_actions(&post_view, &session);
        let agent = watcher.agent.clone();
        let watcher = Box::new(watcher.post_thread(post_view.uri.clone()));
        Self {
            post_view,
            reply,
            actions,
            table_state: TableState::default(),
            list_state: ListState::default(),
            action_tx,
            agent,
            watcher,
            quit: None,
            session,
        }
    }
    fn post_view_actions(post_view: &PostView, session: &Option<Session>) -> Vec<PostAction> {
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
        if Some(&post_view.author.did) == session.as_ref().map(|s| &s.data.did) {
            actions.push(PostAction::Delete);
        }
        let mut links = IndexSet::new();
        if let Record::Known(KnownRecord::AppBskyFeedPost(record)) = &post_view.record {
            if let Some(facets) = &record.facets {
                for facet in facets {
                    for feature in &facet.features {
                        match feature {
                            Union::Refs(MainFeaturesItem::Mention(_)) => {
                                // TODO
                            }
                            Union::Refs(MainFeaturesItem::Link(link)) => {
                                links.insert(link.uri.as_str());
                            }
                            Union::Refs(MainFeaturesItem::Tag(_)) => {
                                // TODO
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        if let Some(embed) = &post_view.embed {
            match embed {
                Union::Refs(PostViewEmbedRefs::AppBskyEmbedImagesView(images)) => {
                    for image in &images.images {
                        links.insert(image.fullsize.as_str());
                    }
                }
                Union::Refs(PostViewEmbedRefs::AppBskyEmbedExternalView(external)) => {
                    links.insert(external.external.uri.as_str());
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
                                links.insert(image.fullsize.as_str());
                            }
                        }
                        Union::Refs(ViewMediaRefs::AppBskyEmbedExternalView(external)) => {
                            links.insert(external.external.uri.as_str());
                        }
                        _ => {}
                    }
                    actions.extend(Self::record_actions(&record_with_media.record));
                }
                _ => {}
            }
        }
        [
            actions,
            links
                .iter()
                .map(|s| PostAction::Open(s.to_string()))
                .collect::<Vec<_>>(),
        ]
        .concat()
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
        if let Some(display_name) = post_view
            .author
            .display_name
            .as_ref()
            .filter(|s| !s.is_empty())
        {
            author_lines.push(Line::from(display_name.as_str()).bold());
        }
        if let Some(labels) = post_view.author.labels.as_ref().filter(|v| !v.is_empty()) {
            for label in labels {
                let mut spans = vec![Span::from(label.val.as_str()).magenta()];
                if !label.uri.ends_with("/self") {
                    spans.extend([Span::from(" "), format!("by {}", label.src.as_ref()).dim()]);
                }
                author_lines.push(Line::from(spans));
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
                Cell::from(Line::from(counts(post_view, 0))),
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
        if let Some(langs) = record.langs.as_ref().filter(|v| !v.is_empty()) {
            rows.push(Row::new(vec![
                Cell::from("Langs:".gray().into_right_aligned_line()),
                Cell::from(
                    langs
                        .iter()
                        .map(|lang| lang.as_ref().to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            ]));
        }
        if let Some(labels) = post_view.labels.as_ref().filter(|v| !v.is_empty()) {
            let mut lines = Vec::new();
            for label in labels {
                let mut spans = vec![Span::from(label.val.as_str()).magenta()];
                if !label.uri.ends_with("/self") {
                    spans.extend([Span::from(" "), format!("by {}", label.src.as_ref()).dim()]);
                }
                lines.push(Line::from(spans));
            }
            rows.push(Row::default().height(lines.len() as u16).cells(vec![
                Cell::from("Labels:".gray().into_right_aligned_line()),
                Cell::from(lines),
            ]));
        }
        if let Some(facets) = &record.facets {
            let lines = facets
                .iter()
                .map(|f| {
                    Line::from(vec![
                        Span::from(format!("[{}-{}] ", f.index.byte_start, f.index.byte_end))
                            .cyan(),
                        Span::from(
                            f.features
                                .iter()
                                .map(|f| match f {
                                    Union::Refs(MainFeaturesItem::Mention(mention)) => {
                                        format!("Mention({})", mention.did.as_ref())
                                    }
                                    Union::Refs(MainFeaturesItem::Link(link)) => {
                                        format!("Link({})", link.uri)
                                    }
                                    Union::Refs(MainFeaturesItem::Tag(tag)) => {
                                        format!("Tag({})", tag.tag)
                                    }
                                    Union::Unknown(_) => String::from("Unknown"),
                                })
                                .collect::<Vec<_>>()
                                .join(", "),
                        ),
                    ])
                })
                .collect::<Vec<_>>();
            rows.push(Row::default().height(facets.len() as u16).cells(vec![
                Cell::from("Facets".gray().into_right_aligned_line()),
                Cell::from(lines),
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
    fn view(&self) -> View {
        View::Post(Box::new((self.post_view.clone(), self.reply.clone())))
    }
    fn activate(&mut self) -> Result<()> {
        let (tx, mut rx) = (self.action_tx.clone(), self.watcher.subscribe());
        let (quit_tx, mut quit_rx) = oneshot::channel();
        self.quit = Some(quit_tx);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    changed = rx.changed() => {
                        if changed.is_ok() {
                            if let Err(e) = tx.send(Action::Update(Box::new(Data::PostThread(
                                rx.borrow_and_update().clone(),
                            )))) {
                                log::error!("failed to send update action: {e}");
                            }
                        } else {
                            break log::warn!("post thread channel closed");
                        }
                    }
                    _ = &mut quit_rx => {
                        break;
                    }
                }
            }
            log::debug!("subscription finished");
        });
        Ok(())
    }
    fn deactivate(&mut self) -> Result<()> {
        if let Some(tx) = self.quit.take() {
            if tx.send(()).is_err() {
                log::error!("failed to send quit signal");
            }
        }
        self.watcher.unsubscribe();
        Ok(())
    }
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::NextItem => {
                self.list_state.select(Some(
                    self.list_state
                        .selected()
                        .map_or(0, |s| (s + 1) % self.actions.len()),
                ));
                return Ok(Some(Action::Render));
            }
            Action::PrevItem => {
                self.list_state.select(Some(
                    self.list_state
                        .selected()
                        .map_or(0, |s| (s + self.actions.len() - 1) % self.actions.len()),
                ));
                return Ok(Some(Action::Render));
            }
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
                            let record_data = api::app::bsky::feed::like::RecordData {
                                created_at: Datetime::now(),
                                subject: api::com::atproto::repo::strong_ref::MainData {
                                    cid: self.post_view.cid.clone(),
                                    uri: self.post_view.uri.clone(),
                                }
                                .into(),
                            };
                            tokio::spawn(async move {
                                match agent.create_record(record_data).await {
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
                            let at_uri = uri.clone();
                            tokio::spawn(async move {
                                match agent.delete_record(at_uri).await {
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
                        PostAction::Delete => {
                            // TODO: confirmation dialog
                            let (agent, tx) = (self.agent.clone(), self.action_tx.clone());
                            let at_uri = self.post_view.uri.clone();
                            tokio::spawn(async move {
                                match agent.delete_record(at_uri).await {
                                    Ok(_) => {
                                        log::info!("deleted record");
                                        tx.send(Action::Transition(Transition::Pop)).ok();
                                    }
                                    Err(e) => {
                                        log::error!("failed to delete record: {e}");
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
            Action::Refresh => {
                self.watcher.refresh();
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
                self.actions = Self::post_view_actions(&self.post_view, &self.session);
                return Ok(Some(Action::Render));
            }
            Action::Transition(_) => {
                return Ok(Some(action));
            }
            _ => {}
        }
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let widths = [Constraint::Length(11), Constraint::Percentage(100)];
        let width = Layout::horizontal(widths).split(area.inner(Margin::new(1, 0)))[1].width;

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
