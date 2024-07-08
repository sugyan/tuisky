use bsky_sdk::api::app::bsky::actor::defs::{ProfileView, ProfileViewBasic};
use ratatui::{style::Stylize, text::Span};

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

pub fn profile_name(author: &dyn Profile) -> Vec<Span> {
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
