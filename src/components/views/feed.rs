use crate::components::Component;
use crate::types::Action;
use crate::widgets::feed_view_post::FeedViewPostWidget;
use atrium_api::app::bsky::feed::defs::FeedViewPost;
use atrium_api::records::{KnownRecord, Record};
use chrono::Local;
use color_eyre::Result;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, List, ListState};
use ratatui::Frame;
use std::sync::{Arc, RwLock};

pub struct FeedView {
    posts: Arc<RwLock<Vec<FeedViewPost>>>,
    state: ListState,
}

impl FeedView {
    pub fn new() -> Self {
        let posts = Arc::new(RwLock::new(Vec::new()));
        Self {
            posts,
            state: ListState::default(),
        }
    }
}

impl Component for FeedView {
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        let len = self.posts.read().unwrap().len();
        match action {
            Action::NextItem => {
                if len == 0 {
                    return Ok(None);
                }
                self.state
                    .select(Some(if let Some(selected) = self.state.selected() {
                        (selected + 1).min(len - 1)
                    } else {
                        0
                    }));
                return Ok(Some(Action::Render));
            }
            Action::PrevItem => {
                if len == 0 {
                    return Ok(None);
                }
                self.state
                    .select(Some(if let Some(selected) = self.state.selected() {
                        selected.max(1) - 1
                    } else {
                        0
                    }));
                return Ok(Some(Action::Render));
            }
            _ => {}
        }
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let mut items = Vec::new();
        if let Ok(posts) = self.posts.read() {
            for post in posts.iter().rev() {
                let Record::Known(KnownRecord::AppBskyFeedPost(record)) = &post.post.record else {
                    continue;
                };
                let mut header_spans = vec![
                    Span::from(
                        post.post
                            .indexed_at
                            .as_ref()
                            .with_timezone(&Local)
                            .format("%Y-%m-%d %H:%M:%S %z")
                            .to_string(),
                    ),
                    Span::from(": "),
                ];
                if let Some(display_name) = &post.post.author.display_name {
                    header_spans
                        .push(Span::from(display_name.to_string()).style(Style::default().bold()));
                    header_spans.push(Span::from(" "));
                }
                header_spans.push(
                    Span::from(format!("@{}", post.post.author.handle.as_str()))
                        .style(Style::default().fg(Color::Gray)),
                );
                items.push(Text::from(vec![
                    Line::from(header_spans),
                    Line::from(format!("  {}", record.text)),
                ]));
            }
        }

        let layout = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);
        f.render_stateful_widget(
            List::new(items)
                .block(Block::bordered().title("Feed"))
                .highlight_style(Style::default().reversed()),
            layout[0],
            &mut self.state,
        );
        if let Some(select) = self.state.selected() {
            if let Ok(posts) = self.posts.read() {
                let post = &posts[posts.len() - 1 - select];
                f.render_widget(FeedViewPostWidget::new(post), layout[1]);
            }
        }
        Ok(())
    }
}
