use super::types::{Action, Data, Transition};
use super::utils::profile_name;
use super::ViewComponent;
use crate::backend::types::SavedFeedValue;
use crate::backend::Watcher;
use bsky_sdk::api::app::bsky::feed::defs::{
    FeedViewPost, FeedViewPostReasonRefs, PostViewEmbedRefs, ReplyRefParentRefs,
};
use bsky_sdk::api::records::{KnownRecord, Record};
use bsky_sdk::api::types::Union;
use chrono::Local;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListState, Padding, Paragraph};
use ratatui::Frame;
use std::sync::Arc;
use textwrap::Options;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

pub struct FeedViewComponent {
    items: Vec<FeedViewPost>,
    state: ListState,
    action_tx: UnboundedSender<Action>,
    watcher: Arc<Watcher>,
    feed: SavedFeedValue,
    handle: Option<JoinHandle<()>>,
}

impl FeedViewComponent {
    pub fn new(
        action_tx: UnboundedSender<Action>,
        watcher: Arc<Watcher>,
        feed: SavedFeedValue,
    ) -> Self {
        Self {
            items: Vec::new(),
            state: ListState::default(),
            action_tx,
            watcher,
            feed,
            handle: None,
        }
    }
}

impl ViewComponent for FeedViewComponent {
    fn activate(&mut self) -> Result<()> {
        let (tx, mut rx) = (
            self.action_tx.clone(),
            self.watcher.feed_views(Vec::new(), &self.feed),
        );
        self.handle = Some(tokio::spawn(async move {
            while rx.changed().await.is_ok() {
                if let Err(e) = tx.send(Action::Update(Box::new(Data::FeedViews(
                    rx.borrow_and_update().clone(),
                )))) {
                    log::error!("failed to send update action: {e}");
                }
            }
        }));
        Ok(())
    }
    fn deactivate(&mut self) -> Result<()> {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
        Ok(())
    }
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('n'), KeyModifiers::CONTROL) | (KeyCode::Down, KeyModifiers::NONE) => {
                Ok(Some(Action::NextItem))
            }
            (KeyCode::Char('p'), KeyModifiers::CONTROL) | (KeyCode::Up, KeyModifiers::NONE) => {
                Ok(Some(Action::PrevItem))
            }
            (KeyCode::Char('b'), KeyModifiers::CONTROL) => Ok(Some(Action::Back)),
            _ => Ok(None),
        }
    }
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::NextItem if !self.items.is_empty() => {
                self.state.select(Some(
                    self.state
                        .selected()
                        .map(|s| (s + 1).min(self.items.len() - 1))
                        .unwrap_or_default(),
                ));
                return Ok(Some(Action::Render));
            }
            Action::PrevItem if !self.items.is_empty() => {
                self.state.select(Some(
                    self.state
                        .selected()
                        .map(|s| s.max(1) - 1)
                        .unwrap_or_default(),
                ));
                return Ok(Some(Action::Render));
            }
            Action::Back => return Ok(Some(Action::Transition(Transition::Pop))),
            Action::Update(data) => {
                let Data::FeedViews(feed_views) = data.as_ref() else {
                    return Ok(None);
                };
                log::info!("update feed views ({} items)", feed_views.len());
                self.items.clone_from(feed_views);
                return Ok(Some(Action::Render));
            }
            _ => {}
        }
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let mut items = Vec::new();
        for feed_view_post in self.items.iter().rev() {
            let Record::Known(KnownRecord::AppBskyFeedPost(record)) = &feed_view_post.post.record
            else {
                continue;
            };
            let mut lines = vec![Line::from(
                [
                    vec![
                        Span::from(
                            feed_view_post
                                .post
                                .indexed_at
                                .as_ref()
                                .with_timezone(&Local)
                                .format("%Y-%m-%d %H:%M:%S %z")
                                .to_string(),
                        )
                        .green(),
                        Span::from(": "),
                    ],
                    profile_name(&feed_view_post.post.author),
                ]
                .concat(),
            )];
            if let Some(Union::Refs(FeedViewPostReasonRefs::ReasonRepost(repost))) =
                &feed_view_post.reason
            {
                lines.push(
                    Line::from(format!(
                        "  Reposted by {}",
                        repost
                            .by
                            .display_name
                            .as_ref()
                            .filter(|s| !s.is_empty())
                            .unwrap_or(&repost.by.handle.as_str().to_string())
                    ))
                    .blue(),
                );
            }
            if let Some(reply) = &feed_view_post.reply {
                if let Union::Refs(ReplyRefParentRefs::PostView(post_view)) = &reply.parent {
                    lines.push(Line::from(
                        [
                            vec![
                                Span::from("  Reply to ").blue(),
                                Span::from(
                                    post_view
                                        .indexed_at
                                        .as_ref()
                                        .with_timezone(&Local)
                                        .format("%H:%M:%S %z")
                                        .to_string(),
                                )
                                .green(),
                                Span::from(": "),
                            ],
                            profile_name(&post_view.author),
                        ]
                        .concat(),
                    ));
                    if let Record::Known(KnownRecord::AppBskyFeedPost(record)) = &post_view.record {
                        lines.extend(
                            textwrap::wrap(
                                &record.text,
                                Options::new(area.width as usize)
                                    .initial_indent("    ")
                                    .subsequent_indent("    "),
                            )
                            .iter()
                            .map(|s| Line::from(s.to_string())),
                        );
                    }
                }
            }
            lines.extend(
                textwrap::wrap(
                    &record.text,
                    Options::new(area.width as usize)
                        .initial_indent("  ")
                        .subsequent_indent("  "),
                )
                .iter()
                .map(|s| Line::from(s.to_string())),
            );
            if let Some(embed) = &feed_view_post.post.embed {
                match embed {
                    Union::Refs(PostViewEmbedRefs::AppBskyEmbedImagesView(images)) => {
                        lines.push(
                            Line::from(format!("  embedded {} images", images.images.len()))
                                .yellow(),
                        );
                    }
                    Union::Refs(PostViewEmbedRefs::AppBskyEmbedExternalView(_)) => {
                        lines.push(Line::from("  embedded external content").yellow());
                    }
                    Union::Refs(PostViewEmbedRefs::AppBskyEmbedRecordView(_)) => {
                        lines.push(Line::from("  embedded record").yellow());
                    }
                    Union::Refs(PostViewEmbedRefs::AppBskyEmbedRecordWithMediaView(_)) => {
                        lines.push(Line::from("  embedded record with media").yellow());
                    }
                    _ => {}
                }
            }
            lines.push(
                Line::from(format!(
                    "  {} replies, {} reposts, {} likes",
                    feed_view_post.post.reply_count.unwrap_or_default(),
                    feed_view_post.post.repost_count.unwrap_or_default(),
                    feed_view_post.post.like_count.unwrap_or_default()
                ))
                .dim(),
            );
            items.push(Text::from(lines));
        }
        let header = Paragraph::new(match &self.feed {
            SavedFeedValue::Feed(generator_view) => Line::from(vec![
                Span::from(generator_view.display_name.clone()).bold(),
                Span::from(" "),
                Span::from(format!(
                    "by {}",
                    generator_view
                        .creator
                        .display_name
                        .clone()
                        .unwrap_or(generator_view.creator.handle.as_ref().to_string())
                ))
                .dim(),
            ]),
            SavedFeedValue::List => Line::from(""),
            SavedFeedValue::Timeline(value) => Line::from(value.as_str()),
        })
        .bold()
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Color::Gray)
                .padding(Padding::horizontal(1)),
        );

        let layout =
            Layout::vertical([Constraint::Length(2), Constraint::Percentage(100)]).split(area);
        f.render_widget(header, layout[0]);
        f.render_stateful_widget(
            List::new(items).highlight_style(Style::default().reset().reversed()),
            layout[1],
            &mut self.state,
        );
        Ok(())
    }
}
