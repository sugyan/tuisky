use bsky_sdk::api::app::bsky::actor::defs::ProfileViewBasic;
use ratatui::{style::Stylize, text::Span};

pub fn profile_name(author: &ProfileViewBasic) -> Vec<Span> {
    if let Some(display_name) = author.display_name.as_ref().filter(|s| !s.is_empty()) {
        vec![
            Span::from(display_name.to_string()).bold(),
            Span::from(" "),
            format!("@{}", author.handle.as_str()).gray(),
        ]
    } else {
        vec![format!("@{}", author.handle.as_str()).bold()]
    }
}
