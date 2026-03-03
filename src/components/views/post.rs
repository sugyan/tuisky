use super::ViewComponent;
use super::types::{Action, Data, ReplyContext, Transition, View};
use super::utils::{counts, profile_name_as_str};
use crate::backend::{Watch, Watcher};
use bsky_sdk::api::agent::atp_agent::AtpSession;
use bsky_sdk::api::app::bsky::actor::defs::ProfileViewBasic;
use bsky_sdk::api::app::bsky::embed::record::{self, ViewRecordRefs};
use bsky_sdk::api::app::bsky::feed::defs::{
    PostView, PostViewData, PostViewEmbedRefs, ThreadViewPostParentRefs, ThreadViewPostRepliesItem,
    ViewerStateData,
};
use bsky_sdk::api::app::bsky::feed::get_post_thread::OutputThreadRefs;
use bsky_sdk::api::app::bsky::feed::post;
use bsky_sdk::api::app::bsky::richtext::facet::MainFeaturesItem;
use bsky_sdk::api::types::string::Datetime;
use bsky_sdk::api::types::{TryFromUnknown, Union};
use bsky_sdk::{BskyAgent, api};
use chrono::Local;
use color_eyre::Result;
use indexmap::IndexSet;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, Borders, Cell, List, ListItem, ListState, Padding, Paragraph, Row, Table, TableState,
};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;

#[derive(Debug, Clone)]
enum PostAction {
    Profile(Box<ProfileViewBasic>),
    Reply,
    Repost,
    Unrepost(String),
    Like,
    Unlike(String),
    Delete,
    Open(String),
    ViewRecord(Box<record::ViewRecord>),
    ViewImages(Vec<(String, Option<String>)>), // (image_url, alt_text)
}

#[derive(Debug, Default, PartialEq, Eq)]
enum Focus {
    #[default]
    Post,
    Actions,
}

impl<'a> From<&'a PostAction> for ListItem<'a> {
    fn from(action: &'a PostAction) -> Self {
        match action {
            PostAction::Profile(profile) => Self::from(Line::from(vec![
                Span::from("Show "),
                Span::from(profile_name_as_str(profile.as_ref())).bold(),
                Span::from("'s profile"),
            ])),
            PostAction::Reply => Self::from("Reply"),
            PostAction::Repost => Self::from("Repost"),
            PostAction::Unrepost(_) => Self::from("Unrepost"),
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
            PostAction::ViewImages(images) => Self::from(Line::from(vec![
                Span::from("View "),
                Span::from(format!("{} image(s)", images.len())).yellow(),
            ])),
        }
    }
}

pub struct PostViewComponent {
    post_view: PostView,
    reply: Option<PostView>,
    replies: Option<Vec<Union<ThreadViewPostRepliesItem>>>,
    actions: Vec<PostAction>,
    table_state: TableState,
    table_rows_len: usize,
    list_state: ListState,
    focus: Focus,
    action_tx: UnboundedSender<Action>,
    agent: Arc<BskyAgent>,
    watcher: Box<dyn Watch<Output = Union<OutputThreadRefs>>>,
    quit: Option<oneshot::Sender<()>>,
    session: Option<AtpSession>,
}

impl PostViewComponent {
    pub fn new(
        action_tx: UnboundedSender<Action>,
        watcher: Arc<Watcher>,
        post_view: PostView,
        reply: Option<PostView>,
        session: Option<AtpSession>,
    ) -> Self {
        let actions = Self::post_view_actions(&post_view, &session);
        let agent = watcher.agent.clone();
        let watcher = Box::new(watcher.post_thread(post_view.uri.clone()));
        Self {
            post_view,
            reply,
            replies: None,
            actions,
            table_state: TableState::default(),
            table_rows_len: 0,
            list_state: ListState::default(),
            focus: Focus::default(),
            action_tx,
            agent,
            watcher,
            quit: None,
            session,
        }
    }

    fn uri_to_web_url(uri: &str, handle: &str) -> Option<String> {
        let uri = uri.strip_prefix("at://")?;
        let _did = uri.split_once('/')?.0;
        let rkey = uri.rsplit('/').next()?;
        Some(format!("https://bsky.app/profile/{}/post/{}", handle, rkey))
    }

    fn post_view_actions(post_view: &PostView, session: &Option<AtpSession>) -> Vec<PostAction> {
        let mut liked = None;
        let mut reposted = None;
        if let Some(viewer) = &post_view.viewer {
            liked = viewer.like.as_ref();
            reposted = viewer.repost.as_ref();
        }
        let mut actions = vec![
            PostAction::Profile(Box::new(post_view.author.clone())),
            PostAction::Reply,
            if let Some(uri) = reposted { PostAction::Unrepost(uri.clone()) } else { PostAction::Repost },
            if let Some(uri) = liked { PostAction::Unlike(uri.clone()) } else { PostAction::Like },
        ];
        if Some(&post_view.author.did) == session.as_ref().map(|s| &s.data.did) {
            actions.push(PostAction::Delete);
        }
        if let Some(link) = Self::uri_to_web_url(&post_view.uri, &post_view.author.handle) {
            actions.push(PostAction::Open(link));
        }
        
        // Collect links from facets (excluding image CDN links)
        let mut links = IndexSet::new();
        if let Ok(record) = post::Record::try_from_unknown(post_view.record.clone()) {
            if let Some(facets) = &record.facets {
                for facet in facets {
                    for feature in &facet.features {
                        match feature {
                            Union::Refs(MainFeaturesItem::Link(link)) => {
                                log::info!("Facet link found: uri={}", link.uri);
                                // Filter out image CDN links
                                if !link.uri.starts_with("https://cdn.bsky.app/img/") {
                                    log::info!("Adding link to actions: {}", link.uri);
                                    links.insert(link.uri.clone());
                                } else {
                                    log::info!("Filtering out image CDN link: {}", link.uri);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        
        // Collect images for viewing
        let mut images_to_view: Vec<(String, Option<String>)> = Vec::new();
        
        if let Some(embed) = &post_view.embed {
            match embed {
                Union::Refs(PostViewEmbedRefs::AppBskyEmbedImagesView(images)) => {
                    for image in &images.images {
                        // 使用缩略图 URL 加快加载速度
                        // BlueSky CDN 支持：/feed_fullsize, /feed_thumbnail, /fullsize, /thumbnail
                        let thumb_url = if image.fullsize.contains("/feed_thumbnail") || image.fullsize.contains("/thumbnail") {
                            // 已经是缩略图，直接使用
                            log::info!("Image already thumbnail: {}", image.fullsize);
                            image.fullsize.clone()
                        } else {
                            // 替换为 thumbnail (处理 /feed_fullsize 和 /fullsize)
                            let url = image.fullsize
                                .replace("/feed_fullsize", "/feed_thumbnail")
                                .replace("/fullsize", "/thumbnail");
                            log::info!("Image URL: {} -> {}", image.fullsize, url);
                            url
                        };
                        images_to_view.push((
                            thumb_url,
                            Some(image.alt.clone()).filter(|s| !s.is_empty()),
                        ));
                    }
                    if !images_to_view.is_empty() {
                        actions.push(PostAction::ViewImages(images_to_view));
                    }
                }
                Union::Refs(PostViewEmbedRefs::AppBskyEmbedExternalView(external)) => {
                    // Add external link from embed
                    log::info!("External embed link: {}", external.external.uri);
                    if !external.external.uri.starts_with("https://cdn.bsky.app/img/") {
                        links.insert(external.external.uri.clone());
                    }
                }
                Union::Refs(PostViewEmbedRefs::AppBskyEmbedRecordView(record)) => {
                    actions.extend(Self::record_actions(record));
                }
                Union::Refs(PostViewEmbedRefs::AppBskyEmbedRecordWithMediaView(record_with_media)) => {
                    if let Union::Refs(bsky_sdk::api::app::bsky::embed::record_with_media::ViewMediaRefs::AppBskyEmbedImagesView(images)) = &record_with_media.media {
                        for image in &images.images {
                            // 使用缩略图 URL 加快加载速度
                            let thumb_url = if image.fullsize.contains("/feed_thumbnail") || image.fullsize.contains("/thumbnail") {
                                log::info!("Image already thumbnail: {}", image.fullsize);
                                image.fullsize.clone()
                            } else {
                                let url = image.fullsize
                                    .replace("/feed_fullsize", "/feed_thumbnail")
                                    .replace("/fullsize", "/thumbnail");
                                log::info!("Image URL: {} -> {}", image.fullsize, url);
                                url
                            };
                            images_to_view.push((
                                thumb_url,
                                Some(image.alt.clone()).filter(|s| !s.is_empty()),
                            ));
                        }
                        if !images_to_view.is_empty() {
                            actions.push(PostAction::ViewImages(images_to_view));
                        }
                    }
                    actions.extend(Self::record_actions(&record_with_media.record));
                }
                _ => {}
            }
        }
        // Merge actions with collected links (excluding image CDN links)
        [actions, links.into_iter().map(PostAction::Open).collect::<Vec<_>>()].concat()
    }
    fn record_actions(record: &record::View) -> Vec<PostAction> {
        let mut actions = Vec::new();
        match &record.record {
            Union::Refs(ViewRecordRefs::ViewRecord(view_record)) => { actions.push(PostAction::ViewRecord(view_record.clone())); }
            _ => {}
        }
        actions
    }
    fn post_view_rows(post_view: &PostView, width: u16, indent: usize) -> Option<Vec<Row<'_>>> {
        let Ok(record) = post::Record::try_from_unknown(post_view.record.clone()) else { return None; };
        let indent_str = if indent > 0 { format!("{}↳ ", "  ".repeat(indent - 1)) } else { String::new() };
        let mut author_lines = vec![Line::from(vec![
            Span::from(indent_str.clone()).dark_gray(),
            Span::from(post_view.author.handle.as_str()),
        ])];
        if let Some(display_name) = post_view.author.display_name.as_ref().filter(|s| !s.is_empty()) {
            author_lines.push(Line::from(vec![Span::from(indent_str.clone()).dark_gray(), Span::from(display_name.as_str()).bold()]));
        }
        let text_lines = textwrap::wrap(&record.text, usize::from(width));
        let mut rows = vec![
            Row::new(vec![Cell::from("CID:".gray().into_right_aligned_line()), Cell::from(post_view.cid.as_ref().to_string())]),
            Row::new(vec![Cell::from("IndexedAt:".gray().into_right_aligned_line()), Cell::from(post_view.indexed_at.as_ref().with_timezone(&Local).format("%Y-%m-%d %H:%M:%S %z").to_string()).green()]),
            Row::default().height(author_lines.len() as u16).cells(vec![Cell::from("Author:".gray().into_right_aligned_line()), Cell::from(Text::from(author_lines))]),
            Row::new(vec![Cell::from("Counts:".gray().into_right_aligned_line()), Cell::from(Line::from(counts(post_view, 0)))]),
            Row::default().height(text_lines.len() as u16).cells(vec![
                Cell::from("Text:".gray().into_right_aligned_line()),
                Cell::from(text_lines.iter().map(|s| Line::from(vec![Span::from(indent_str.clone()).dark_gray(), Span::from(s.to_string())])).collect::<Vec<_>>()),
            ]),
        ];
        // Add image embed info
        if let Some(embed) = &post_view.embed {
            match embed {
                Union::Refs(PostViewEmbedRefs::AppBskyEmbedImagesView(images)) => {
                    let image_count = images.images.len();
                    rows.push(Row::new(vec![
                        Cell::from("Images:".gray().into_right_aligned_line()),
                        Cell::from(Line::from(format!("{} image(s)", image_count)).yellow()),
                    ]));
                }
                Union::Refs(PostViewEmbedRefs::AppBskyEmbedRecordWithMediaView(record_with_media)) => {
                    if let Union::Refs(bsky_sdk::api::app::bsky::embed::record_with_media::ViewMediaRefs::AppBskyEmbedImagesView(images)) = &record_with_media.media {
                        let image_count = images.images.len();
                        rows.push(Row::new(vec![
                            Cell::from("Images:".gray().into_right_aligned_line()),
                            Cell::from(Line::from(format!("{} image(s)", image_count)).yellow()),
                        ]));
                    }
                }
                _ => {}
            }
        }
        Some(rows)
    }
    fn replies_rows(replies: &Option<Vec<Union<ThreadViewPostRepliesItem>>>, width: u16, indent: usize) -> Vec<Row<'_>> {
        let mut rows = Vec::new();
        if let Some(replies) = replies {
            for reply_item in replies {
                if let Union::Refs(ThreadViewPostRepliesItem::ThreadViewPost(thread_view)) = reply_item {
                    if let Some(r) = Self::post_view_rows(&thread_view.post, width, indent) {
                        rows.push(Row::new([" --------- ".blue()]));
                        rows.extend(r);
                        rows.extend(Self::replies_rows(&thread_view.replies, width, indent + 1));
                    }
                }
            }
        }
        rows
    }
    fn find_in(replies: &Vec<Union<ThreadViewPostRepliesItem>>, target: usize, current: &mut usize) -> Option<PostView> {
        for item in replies {
            if let Union::Refs(ThreadViewPostRepliesItem::ThreadViewPost(v)) = item {
                if let Some(rows) = Self::post_view_rows(&v.post, 80, 0) {
                    if *current == target { return Some(v.post.clone()); }
                    *current += 1; let start = *current; *current += rows.len();
                    if (start..*current).contains(&target) { return Some(v.post.clone()); }
                    if let Some(replies) = &v.replies { if let Some(p) = Self::find_in(replies, target, current) { return Some(p); } }
                }
            }
        }
        None
    }
}

impl ViewComponent for PostViewComponent {
    fn view(&self) -> View { View::Post(Box::new((self.post_view.clone(), self.reply.clone()))) }
    fn handle_key_events(&mut self, key: ratatui::crossterm::event::KeyEvent) -> Result<Option<Action>> {
        if key.code == ratatui::crossterm::event::KeyCode::Esc || key.code == ratatui::crossterm::event::KeyCode::Backspace {
            return Ok(Some(Action::Back));
        }
        if key.code == ratatui::crossterm::event::KeyCode::Tab {
            self.focus = match self.focus { Focus::Post => Focus::Actions, Focus::Actions => Focus::Post };
            return Ok(Some(Action::Render));
        }
        if key.modifiers == ratatui::crossterm::event::KeyModifiers::NONE || key.modifiers == ratatui::crossterm::event::KeyModifiers::SHIFT {
            match key.code {
                ratatui::crossterm::event::KeyCode::Char('o') => { if let Some(link) = Self::uri_to_web_url(&self.post_view.uri, &self.post_view.author.handle) { let _ = open::that(link); } return Ok(Some(Action::Render)); }
                ratatui::crossterm::event::KeyCode::Char('d') | ratatui::crossterm::event::KeyCode::Char('D') => { return Ok(Some(Action::Delete)); }
                _ => {}
            }
        }
        Ok(None)
    }
    fn activate(&mut self) -> Result<()> {
        let (tx, mut rx) = (self.action_tx.clone(), self.watcher.subscribe());
        let (quit_tx, mut quit_rx) = oneshot::channel();
        self.quit = Some(quit_tx);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    changed = rx.changed() => { if changed.is_ok() { let _ = tx.send(Action::Update(Box::new(Data::PostThread(rx.borrow_and_update().clone())))); } else { break; } }
                    _ = &mut quit_rx => break,
                }
            }
        });
        Ok(())
    }
    fn deactivate(&mut self) -> Result<()> { if let Some(tx) = self.quit.take() { let _ = tx.send(()); } self.watcher.unsubscribe(); Ok(()) }
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::NextItem => {
                match self.focus {
                    Focus::Post => { let current = self.table_state.selected().unwrap_or(0); if current < self.table_rows_len.saturating_sub(1) { self.table_state.select(Some(current + 1)); } }
                    Focus::Actions => { self.list_state.select(Some(self.list_state.selected().map_or(0, |s| (s + 1) % self.actions.len().max(1)))); }
                }
                return Ok(Some(Action::Render));
            }
            Action::PrevItem => {
                match self.focus {
                    Focus::Post => { let current = self.table_state.selected().unwrap_or(0); if current > 0 { self.table_state.select(Some(current - 1)); } }
                    Focus::Actions => { self.list_state.select(Some(self.list_state.selected().map_or(0, |s| (s + self.actions.len().max(1) - 1) % self.actions.len().max(1)))); }
                }
                return Ok(Some(Action::Render));
            }
            Action::Enter => {
                if self.focus == Focus::Actions {
                    if let Some(action) = self.list_state.selected().and_then(|i| self.actions.get(i)) {
                        match action {
                            PostAction::Like => {
                                let (agent, tx) = (self.agent.clone(), self.action_tx.clone());
                                let mut viewer = self.post_view.viewer.clone().unwrap_or(ViewerStateData { bookmarked: None, embedding_disabled: None, like: None, pinned: None, reply_disabled: None, repost: None, thread_muted: None }.into());
                                let record_data = api::app::bsky::feed::like::RecordData { created_at: Datetime::now(), subject: api::com::atproto::repo::strong_ref::MainData { cid: self.post_view.cid.clone(), uri: self.post_view.uri.clone() }.into(), via: None };
                                tokio::spawn(async move { if let Ok(output) = agent.create_record(record_data).await { viewer.like = Some(output.uri.clone()); let _ = tx.send(Action::Update(Box::new(Data::ViewerState(Some(viewer))))); } });
                            }
                            PostAction::Unlike(uri) => {
                                let (agent, tx) = (self.agent.clone(), self.action_tx.clone());
                                let mut viewer = self.post_view.viewer.clone();
                                let at_uri = uri.clone();
                                tokio::spawn(async move { if agent.delete_record(at_uri).await.is_ok() { if let Some(v) = viewer.as_mut() { v.like = None; } let _ = tx.send(Action::Update(Box::new(Data::ViewerState(viewer)))); } });
                            }
                            PostAction::Delete => { return Ok(Some(Action::Delete)); }
                            PostAction::ViewRecord(view_record) => {
                                return Ok(Some(Action::Transition(Transition::Push(Box::new(View::Post(Box::new((PostViewData { author: view_record.author.clone(), bookmark_count: None, cid: view_record.cid.clone(), embed: None, indexed_at: view_record.indexed_at.clone(), labels: view_record.labels.clone(), like_count: view_record.like_count, quote_count: view_record.quote_count, record: view_record.value.clone(), reply_count: view_record.reply_count, repost_count: view_record.repost_count, threadgate: None, uri: view_record.uri.clone(), viewer: None }.into(), None))))))));
                            }
                            PostAction::Reply => {
                                let root = if let Ok(record) = post::Record::try_from_unknown(self.post_view.record.clone()) { record.reply.as_ref().map(|r| r.root.clone()).unwrap_or_else(|| api::com::atproto::repo::strong_ref::MainData { cid: self.post_view.cid.clone(), uri: self.post_view.uri.clone() }.into()) } else { api::com::atproto::repo::strong_ref::MainData { cid: self.post_view.cid.clone(), uri: self.post_view.uri.clone() }.into() };
                                let parent = api::com::atproto::repo::strong_ref::MainData { cid: self.post_view.cid.clone(), uri: self.post_view.uri.clone() }.into();
                                return Ok(Some(Action::Transition(Transition::Push(Box::new(View::NewPost(Some(ReplyContext { root, parent, handle: self.post_view.author.handle.to_string() })))))));
                            }
                            PostAction::Repost => {
                                let (agent, tx) = (self.agent.clone(), self.action_tx.clone());
                                let mut viewer = self.post_view.viewer.clone().unwrap_or(ViewerStateData { bookmarked: None, embedding_disabled: None, like: None, pinned: None, reply_disabled: None, repost: None, thread_muted: None }.into());
                                let record_data = api::app::bsky::feed::repost::RecordData { created_at: Datetime::now(), subject: api::com::atproto::repo::strong_ref::MainData { cid: self.post_view.cid.clone(), uri: self.post_view.uri.clone() }.into(), via: None };
                                tokio::spawn(async move { if let Ok(output) = agent.create_record(record_data).await { viewer.repost = Some(output.uri.clone()); let _ = tx.send(Action::Update(Box::new(Data::ViewerState(Some(viewer))))); } });
                            }
                            PostAction::Unrepost(uri) => {
                                let (agent, tx) = (self.agent.clone(), self.action_tx.clone());
                                let mut viewer = self.post_view.viewer.clone();
                                let at_uri = uri.clone();
                                tokio::spawn(async move { if agent.delete_record(at_uri).await.is_ok() { if let Some(v) = viewer.as_mut() { v.repost = None; } let _ = tx.send(Action::Update(Box::new(Data::ViewerState(viewer)))); } });
                            }
                            PostAction::ViewImages(images) => {
                                // Open image gallery view
                                return Ok(Some(Action::Transition(Transition::Push(Box::new(View::ImageGallery(images.clone()))))));
                            }
                            PostAction::Open(uri) => {
                                // Open link in external browser
                                let _ = open::that(uri.clone());
                            }
                            _ => {}
                        }
                    }
                } else if self.focus == Focus::Post {
                    let selected = self.table_state.selected().unwrap_or(0);
                    let mut current = 0;
                    if let Some(reply) = &self.reply {
                        if let Some(r) = Self::post_view_rows(reply, 80, 0) {
                            current += 1; let start = current; current += r.len();
                            if (start..current).contains(&selected) { return Ok(Some(Action::Transition(Transition::Push(Box::new(View::Post(Box::new((reply.clone(), None)))))))); }
                            current += 1;
                        }
                    }
                    if let Some(r) = Self::post_view_rows(&self.post_view, 80, 0) { current += r.len(); }
                    if let Some(replies) = &self.replies {
                        current += 1;
                        if let Some(p) = Self::find_in(replies, selected, &mut current) {
                            return Ok(Some(Action::Transition(Transition::Push(Box::new(View::Post(Box::new((p, None))))))));
                        }
                    }
                }
            }
            Action::Delete => {
                let mut target = self.post_view.clone();
                let mut is_sub_item = false;
                if self.focus == Focus::Post {
                    let selected = self.table_state.selected().unwrap_or(0);
                    let mut current = 0;
                    let mut found = None;
                    if let Some(reply) = &self.reply {
                        if let Some(r) = Self::post_view_rows(reply, 80, 0) {
                            current += 1; let start = current; current += r.len();
                            if (start..current).contains(&selected) { found = Some(reply.clone()); }
                            current += 1;
                        }
                    }
                    if found.is_none() {
                        if let Some(r) = Self::post_view_rows(&self.post_view, 80, 0) {
                            let start = current; current += r.len();
                            if (start..current).contains(&selected) { found = Some(self.post_view.clone()); }
                        }
                    }
                    if found.is_none() {
                        if let Some(replies) = &self.replies {
                            current += 1;
                            found = Self::find_in(replies, selected, &mut current);
                            if found.is_some() { is_sub_item = true; }
                        }
                    }
                    if let Some(p) = found { target = p; }
                }
                
                log::info!("Action::Delete executing. is_sub_item: {}", is_sub_item);
                if Some(&target.author.did) == self.session.as_ref().map(|s| &s.data.did) {
                    let (agent, tx) = (self.agent.clone(), self.action_tx.clone());
                    let at_uri = target.uri.clone();
                    tokio::spawn(async move {
                        log::info!("Delete task started for URI: {}", at_uri);
                        match agent.delete_record(at_uri.clone()).await {
                            Ok(_) => {
                                log::info!("Delete successful. is_sub_item: {}", is_sub_item);
                                if !is_sub_item {
                                    log::info!("Emitting Transition::Pop back to caller");
                                    let _ = tx.send(Action::Transition(Transition::Pop));
                                } else {
                                    log::info!("Emitting Refresh");
                                    let _ = tx.send(Action::Refresh);
                                }
                            }
                            Err(e) => log::error!("Delete API error: {e}"),
                        }
                    });
                }
                return Ok(None);
            }
            Action::Back => return Ok(Some(Action::Transition(Transition::Pop))),
            Action::Refresh => self.watcher.refresh(),
            Action::Update(data) => {
                match data.as_ref() {
                    Data::PostThread(Union::Refs(OutputThreadRefs::AppBskyFeedDefsThreadViewPost(thread_view))) => {
                        self.post_view = thread_view.post.clone();
                        if let Some(Union::Refs(ThreadViewPostParentRefs::ThreadViewPost(parent))) = &thread_view.parent { self.reply = Some(parent.post.clone()); }
                        self.replies = thread_view.replies.clone();
                    }
                    Data::ViewerState(viewer) => { self.post_view.viewer.clone_from(viewer); }
                    _ => return Ok(None),
                }
                self.actions = Self::post_view_actions(&self.post_view, &self.session);
                return Ok(Some(Action::Render));
            }
            _ => {}
        }
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let widths = [Constraint::Length(11), Constraint::Percentage(100)];
        let width = Layout::horizontal(widths).split(area.inner(Margin::new(1, 0)))[1].width;
        let mut rows = Vec::new();
        if let Some(reply) = &self.reply { if let Some(r) = Self::post_view_rows(reply, width, 0) { rows.push(Row::new([" Reply to".blue()])); rows.extend(r); rows.push(Row::new([" --------- ".blue()])); } }
        if self.table_state.selected().is_none() { self.table_state.select(Some(rows.len())); }
        if let Some(r) = Self::post_view_rows(&self.post_view, width, 0) { rows.extend(r); }
        if let Some(replies) = &self.replies { if !replies.is_empty() { rows.push(Row::new([" --------- Replies --------- ".blue()])); rows.extend(Self::replies_rows(&self.replies, width, 1)); } }
        self.table_rows_len = rows.len();
        let layout = Layout::vertical([Constraint::Length(2), Constraint::Percentage(100), Constraint::Min(10)]).split(area);
        f.render_widget(Paragraph::new(self.post_view.uri.as_str()).bold().block(Block::default().borders(Borders::BOTTOM).border_style(Color::Gray).padding(Padding::horizontal(1))), layout[0]);
        let table_highlight = if self.focus == Focus::Post { Style::default().reversed() } else { Style::default() };
        f.render_stateful_widget(Table::new(rows, widths).row_highlight_style(table_highlight), layout[1], &mut self.table_state);
        let actions_border_color = if self.focus == Focus::Actions { Color::Yellow } else { Color::Gray };
        f.render_stateful_widget(List::new(&self.actions).highlight_style(Style::default().reversed()).block(Block::default().title(if self.focus == Focus::Actions { "Actions (Focused)" } else { "Actions" }).title_alignment(Alignment::Center).borders(Borders::TOP).border_style(actions_border_color).padding(Padding::horizontal(1))), layout[2], &mut self.list_state);
        Ok(())
    }
}
