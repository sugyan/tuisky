use super::super::views::types::Action as ViewsAction;
use super::types::{Action, Data};
use super::ModalComponent;
use bsky_sdk::agent::config::Config;
use bsky_sdk::api::com::atproto::repo::strong_ref;
use bsky_sdk::api::types::string::{AtIdentifier, Cid, Nsid};
use bsky_sdk::BskyAgent;
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear};
use ratatui::Frame;
use regex::Regex;
use std::ops::Deref;
use std::sync::{Arc, LazyLock, Mutex};
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::TextArea;

static RE_AT_URI: LazyLock<Regex> = LazyLock::new(|| {
    // const aturiRegex =
    //   /^at:\/\/(?<authority>[a-zA-Z0-9._:%-]+)(\/(?<collection>[a-zA-Z0-9-.]+)(\/(?<rkey>[a-zA-Z0-9._~:@!$&%')(*+,;=-]+))?)?(#(?<fragment>\/[a-zA-Z0-9._~:@!$&%')(*+,;=\-[\]/\\]*))?$/
    Regex::new(
        r"^at:\/\/(?<authority>[a-zA-Z0-9._:%-]+)(\/(?<collection>[a-zA-Z0-9-.]+)(\/(?<rkey>[a-zA-Z0-9.\-_:~]{1,512})))$"
    ).expect("invalid regex")
});

const PUBLIC_API_ENDPOINT: &str = "https://public.api.bsky.app";

enum Focus {
    None,
    Input,
    Ok,
    Delete,
}

impl Focus {
    fn next(&self, delete: bool) -> Self {
        match self {
            Self::None => Self::Input,
            Self::Input => Self::Ok,
            Self::Ok if delete => Self::Delete,
            Self::Ok => Self::Ok,
            Self::Delete => Self::Delete,
        }
    }
    fn prev(&self, _: bool) -> Self {
        match self {
            Self::None => Self::Input,
            Self::Input => Self::Input,
            Self::Ok => Self::Input,
            Self::Delete => Self::Ok,
        }
    }
}

#[derive(Debug, Clone)]
enum State {
    None,
    Ok(Cid),
    Error(String),
}

pub struct EmbedRecordModalComponent {
    action_tx: UnboundedSender<ViewsAction>,
    input: TextArea<'static>,
    record: Option<String>,
    focus: Focus,
    state: Arc<Mutex<State>>,
}

impl EmbedRecordModalComponent {
    pub fn new(action_tx: UnboundedSender<ViewsAction>, init: Option<String>) -> Self {
        let mut input = if let Some(data) = &init {
            TextArea::new(vec![data.clone()])
        } else {
            TextArea::default()
        };
        input.set_block(Block::bordered().title("Path"));
        input.set_cursor_line_style(Style::default());
        Self {
            action_tx,
            input,
            record: init,
            focus: Focus::Input,
            state: Arc::new(Mutex::new(State::None)),
        }
    }
    fn get_record(&self, uri: &str) -> Option<&str> {
        let Some(captures) = RE_AT_URI.captures(uri) else {
            return Some("invalid at uri");
        };
        let (Some(authority), Some(collection), Some(rkey)) = (
            captures.name("authority"),
            captures.name("collection"),
            captures.name("rkey"),
        ) else {
            return Some("missing authority, collection, or rkey");
        };
        let (Ok(repo), Ok(collection)) = (
            authority.as_str().parse::<AtIdentifier>(),
            collection.as_str().parse::<Nsid>(),
        ) else {
            return Some("invalid authority or collection");
        };
        let rkey = rkey.as_str().to_string();
        let action_tx = self.action_tx.clone();
        let state = self.state.clone();
        tokio::spawn(async move {
            *state.lock().unwrap() = match Self::try_get_record(collection, repo, rkey).await {
                Ok(cid) => State::Ok(cid),
                Err(_) => State::Error("failed to get record".into()),
            };
            if let Err(e) = action_tx.send(ViewsAction::Render) {
                log::error!("failed to send render event: {e}");
            }
        });
        None
    }
    fn update_focus(&mut self, focus: Focus) {
        if let Focus::Input = self.focus {
            self.input.set_cursor_style(Style::default());
            if let Some(block) = self.input.block() {
                self.input.set_block(block.clone().dim());
            }
        }
        self.focus = focus;
        if let Focus::Input = self.focus {
            self.input.set_cursor_style(Style::default().reversed());
            if let Some(block) = self.input.block() {
                self.input.set_block(block.clone().reset());
            }
        }
    }
    async fn try_get_record(collection: Nsid, repo: AtIdentifier, rkey: String) -> Result<Cid> {
        let agent = BskyAgent::builder()
            .config(Config {
                endpoint: PUBLIC_API_ENDPOINT.to_string(),
                ..Default::default()
            })
            .build()
            .await?;
        let output = agent
            .api
            .com
            .atproto
            .repo
            .get_record(
                bsky_sdk::api::com::atproto::repo::get_record::ParametersData {
                    cid: None,
                    collection,
                    repo,
                    rkey,
                }
                .into(),
            )
            .await?;
        Ok(output.data.cid.expect("missing cid"))
    }
}

impl ModalComponent for EmbedRecordModalComponent {
    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        if matches!(self.focus, Focus::Input)
            && !matches!(
                (key.code, key.modifiers),
                (KeyCode::Enter, _) | (KeyCode::Char('m'), KeyModifiers::CONTROL)
            )
        {
            let cursor = self.input.cursor();
            return Ok(if self.input.input(key) {
                *self.state.lock().unwrap() = State::None;
                Some(Action::Render)
            } else if self.input.cursor() != cursor {
                Some(Action::Render)
            } else {
                None
            });
        }
        Ok(None)
    }
    fn update(&mut self, action: ViewsAction) -> Result<Option<Action>> {
        Ok(match action {
            ViewsAction::NextItem => {
                self.update_focus(self.focus.next(self.record.is_some()));
                Some(Action::Render)
            }
            ViewsAction::PrevItem => {
                self.update_focus(self.focus.prev(self.record.is_some()));
                Some(Action::Render)
            }
            ViewsAction::Enter => match self.focus {
                Focus::Ok => {
                    let uri = self.input.lines().join("");
                    let mut state = self.state.lock().unwrap();
                    if let State::Ok(cid) = state.deref() {
                        Some(Action::Ok(Data::Record(
                            strong_ref::MainData {
                                cid: cid.clone(),
                                uri,
                            }
                            .into(),
                        )))
                    } else {
                        self.focus = Focus::None;
                        if let Some(err) = self.get_record(&uri) {
                            *state = State::Error(err.into());
                        }
                        Some(Action::Render)
                    }
                }
                Focus::Delete => Some(Action::Delete(None)),
                _ => self.update(ViewsAction::NextItem)?,
            },
            ViewsAction::Back => Some(Action::Cancel),
            _ => None,
        })
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let area = area.inner(Margin {
            horizontal: 2,
            vertical: 1,
        });
        let [area] = Layout::vertical([Constraint::Max(8)]).areas(area);

        let block = Block::bordered().title("Embed record");
        let inner = block.inner(area);
        f.render_widget(Clear, area);
        f.render_widget(block, area);

        let mut constraints = vec![
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ];
        if self.record.is_some() {
            constraints.push(Constraint::Length(1));
        }
        let layout = Layout::vertical(constraints).split(inner);

        let state = self.state.lock().unwrap().clone();
        if let Some(block) = self.input.block() {
            let block = block.clone();
            self.input.set_block(match &state {
                State::None => block.border_style(Color::Reset),
                State::Ok(_) => block.border_style(Color::Green),
                State::Error(_) => block.border_style(Color::Red),
            });
        }
        f.render_widget(&self.input, layout[0]);
        f.render_widget(
            match &state {
                State::None => Line::from(""),
                State::Ok(cid) => Line::from(format!("CID: {}", cid.as_ref())).bold(),
                State::Error(err) => Line::from(err.clone()).red(),
            },
            layout[1],
        );
        f.render_widget(
            Line::from(match &state {
                State::Ok(_) => "OK",
                _ => "Get Record",
            })
            .centered()
            .blue()
            .patch_style(if let Focus::Ok = self.focus {
                Style::default().reversed()
            } else {
                Style::default()
            }),
            layout[2],
        );
        if let Some(area) = layout.get(3) {
            f.render_widget(
                Line::from("Delete").centered().red().patch_style(
                    if let Focus::Delete = self.focus {
                        Style::default().reversed()
                    } else {
                        Style::default()
                    },
                ),
                *area,
            )
        }

        Ok(())
    }
}
