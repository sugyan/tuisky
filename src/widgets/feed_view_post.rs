use atrium_api::app::bsky::feed::defs::{
    FeedViewPost, FeedViewPostReasonRefs, PostViewEmbedRefs, ReplyRefParentRefs,
};
use atrium_api::records::{KnownRecord, Record};
use atrium_api::types::Union;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Padding, Paragraph, Widget, Wrap};

pub struct FeedViewPostWidget<'a> {
    post: &'a FeedViewPost,
}

impl<'a> FeedViewPostWidget<'a> {
    pub fn new(post: &'a FeedViewPost) -> Self {
        Self { post }
    }
}

impl<'a> Widget for FeedViewPostWidget<'a> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let block = Block::bordered().title("Post detail");
        let inner = block.inner(area);
        block.render(area, buffer);
        // basic info
        let mut lines = vec![
            Line::from(vec![
                Span::from("CID: ").style(Style::default().dim()),
                Span::from(self.post.post.cid.as_ref().to_string()),
            ]),
            Line::from(vec![
                Span::from("URI: ").style(Style::default().dim()),
                Span::from(self.post.post.uri.clone()),
            ]),
            Line::from(vec![
                Span::from("Indexed At: ").style(Style::default().dim()),
                Span::from(self.post.post.indexed_at.as_ref().to_rfc3339()),
            ]),
            Line::from("Author: ").style(Style::default().dim()),
            Line::from(vec![
                Span::from("  DID: ").style(Style::default().dim()),
                Span::from(self.post.post.author.did.as_str()),
            ]),
            Line::from(vec![
                Span::from("  Handle: ").style(Style::default().dim()),
                Span::from(self.post.post.author.handle.as_str()),
            ]),
        ];
        if let Some(display_name) = &self.post.post.author.display_name {
            lines.push(Line::from(vec![
                Span::from("  Display Name: ").style(Style::default().dim()),
                Span::from(display_name.clone()),
            ]));
        }
        if let Some(Union::Refs(embed)) = &self.post.post.embed {
            lines.push(Line::from("Embed: ").style(Style::default().dim()));
            match embed {
                PostViewEmbedRefs::AppBskyEmbedImagesView(view) => {
                    lines.push(Line::from("  Image: ").style(Style::default().dim()));
                    for image in &view.images {
                        lines.push(Line::from(vec![
                            Span::from("  - [").style(Style::default().dim()),
                            Span::from(image.alt.clone()),
                            Span::from("](").style(Style::default().dim()),
                            Span::from(image.fullsize.clone()),
                            Span::from(")").style(Style::default().dim()),
                        ]));
                    }
                }
                PostViewEmbedRefs::AppBskyEmbedExternalView(view) => {
                    lines.push(Line::from("  External: ").style(Style::default().dim()));
                    lines.extend([
                        Line::from(vec![
                            Span::from("    URL: ").style(Style::default().dim()),
                            Span::from(view.external.uri.clone()),
                        ]),
                        Line::from(vec![
                            Span::from("    Title: ").style(Style::default().dim()),
                            Span::from(view.external.title.clone()),
                        ]),
                    ]);
                    if let Some(thumb) = &view.external.thumb {
                        lines.push(Line::from(vec![
                            Span::from("    Thumb: ").style(Style::default().dim()),
                            Span::from(thumb.clone()),
                        ]));
                    }
                    lines.push(Line::from(vec![
                        Span::from("    Description: ").style(Style::default().dim()),
                        Span::from(view.external.description.clone()),
                    ]));
                }
                PostViewEmbedRefs::AppBskyEmbedRecordView(_) => {
                    // TODO
                }
                PostViewEmbedRefs::AppBskyEmbedRecordWithMediaView(_) => {
                    // TODO
                }
            }
        }
        if let Some(Union::Refs(FeedViewPostReasonRefs::ReasonRepost(reason))) = &self.post.reason {
            lines.extend([
                Line::from("Reposted: ").style(Style::default().dim()),
                Line::from(vec![
                    Span::from("  By: ").style(Style::default().dim()),
                    Span::from(if let Some(display_name) = &reason.by.display_name {
                        format!("{display_name} (@{})", reason.by.handle.as_str())
                    } else {
                        format!("@{}", reason.by.handle.as_str())
                    }),
                ]),
                Line::from(vec![
                    Span::from("  Indexed At: ").style(Style::default().dim()),
                    Span::from(reason.indexed_at.as_ref().to_rfc3339()),
                ]),
            ]);
        }
        if let Some(reply) = &self.post.reply {
            lines.push(Line::from("Reply To: ").style(Style::default().dim()));
            if let Union::Refs(parent) = &reply.parent {
                match parent {
                    ReplyRefParentRefs::PostView(post_view) => {
                        lines.extend([
                            Line::from(vec![
                                Span::from("  Indexed At: ").style(Style::default().dim()),
                                Span::from(post_view.indexed_at.as_ref().to_rfc3339()),
                            ]),
                            Line::from(vec![
                                Span::from("  Author: ").style(Style::default().dim()),
                                Span::from(
                                    if let Some(display_name) = &post_view.author.display_name {
                                        format!(
                                            "{display_name} (@{})",
                                            post_view.author.handle.as_str()
                                        )
                                    } else {
                                        format!("@{}", post_view.author.handle.as_str())
                                    },
                                ),
                            ]),
                        ]);
                        if let Record::Known(KnownRecord::AppBskyFeedPost(record)) =
                            &post_view.record
                        {
                            lines.push(Line::from(vec![
                                Span::from("  Text: ").style(Style::default().dim()),
                                Span::from(record.text.clone()),
                            ]));
                        }
                    }
                    ReplyRefParentRefs::NotFoundPost(_) => {
                        // TODO
                    }
                    ReplyRefParentRefs::BlockedPost(_) => {
                        // TODO
                    }
                }
            }
        }
        lines.push(Line::from(vec![
            Span::from("Text: ").style(Style::default().dim())
        ]));
        let layout = Layout::vertical([
            Constraint::Length(lines.len() as u16),
            Constraint::Percentage(100),
        ])
        .split(inner);
        Text::from(lines).render(layout[0], buffer);
        // post text
        if let Record::Known(KnownRecord::AppBskyFeedPost(record)) = &self.post.post.record {
            Paragraph::new(record.text.clone())
                .block(Block::default().padding(Padding::left(2)))
                .wrap(Wrap::default())
                .render(layout[1], buffer);
        }
    }
}
