use bsky_sdk::api::app::bsky::actor::defs::{ProfileView, ProfileViewBasic};
use bsky_sdk::api::app::bsky::feed::defs::PostView;
use ratatui::style::{Style, Stylize};
use ratatui::text::Span;

pub trait Profile {
    fn display_name(&self) -> Option<&str>;
    fn handle(&self) -> &str;
}

impl Profile for ProfileView {
    fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref().filter(|s| !s.is_empty())
    }
    fn handle(&self) -> &str {
        self.handle.as_str()
    }
}

impl Profile for ProfileViewBasic {
    fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref().filter(|s| !s.is_empty())
    }
    fn handle(&self) -> &str {
        self.handle.as_str()
    }
}

pub fn profile_name_as_str(author: &dyn Profile) -> &str {
    author.display_name().unwrap_or(author.handle())
}

pub fn profile_name(author: &dyn Profile) -> Vec<Span<'_>> {
    if let Some(display_name) = author.display_name() {
        vec![
            Span::from(display_name.to_string()).bold(),
            Span::from(" "),
            format!("@{}", author.handle()).gray(),
        ]
    } else {
        vec![format!("@{}", author.handle()).bold()]
    }
}

pub fn counts(post_view: &PostView, pad: usize) -> Vec<Span<'_>> {
    let (mut reposted, mut liked) = (false, false);
    if let Some(viewer) = &post_view.viewer {
        reposted = viewer.repost.is_some();
        liked = viewer.like.is_some();
    }
    let (replies, reposts, quotes, likes) = (
        post_view.reply_count.unwrap_or_default(),
        post_view.repost_count.unwrap_or_default(),
        post_view.quote_count.unwrap_or_default(),
        post_view.like_count.unwrap_or_default(),
    );
    let style = |b| {
        if b {
            Style::default()
        } else {
            Style::default().dim()
        }
    };
    vec![
        Span::from(format!("{replies:pad$} replies")).style(style(replies > 0)),
        Span::from(", ").dim(),
        Span::from(format!("{reposts:pad$}")).style(if reposted {
            Style::default().green()
        } else {
            style(reposts > 0)
        }),
        Span::from(" reposts").style(style(reposts > 0)),
        Span::from(", ").dim(),
        Span::from(format!("{quotes:pad$}")).style(style(reposts > 0)),
        Span::from(" quotes").style(style(reposts > 0)),
        Span::from(", ").dim(),
        Span::from(format!("{likes:pad$}")).style(if liked {
            Style::default().red()
        } else {
            style(likes > 0)
        }),
        Span::from(" likes").style(style(likes > 0)),
    ]
}
