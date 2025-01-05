use {
    super::{
        types::{Action, Transition, View},
        utils::profile_name_as_str,
        ViewComponent,
    },
    crate::{
        backend::{
            types::{FeedSourceInfo, PinnedFeed},
            {Watch, Watcher},
        },
        components::views::types::Data,
    },
    color_eyre::Result,
    ratatui::{
        style::{Style, Stylize},
        text::{Line, Span, Text},
        widgets::{Block, List, ListState, Padding},
        {layout::Rect, Frame},
    },
    std::sync::Arc,
    tokio::sync::{mpsc::UnboundedSender, oneshot},
};

pub struct RootComponent {
    items: Vec<PinnedFeed>,
    state: ListState,
    action_tx: UnboundedSender<Action>,
    watcher: Box<dyn Watch<Output = Vec<PinnedFeed>>>,
    quit: Option<oneshot::Sender<()>>,
}

impl RootComponent {
    pub fn new(action_tx: UnboundedSender<Action>, watcher: Arc<Watcher>) -> Self {
        Self {
            items: Vec::new(),
            state: ListState::default(),
            action_tx,
            watcher: Box::new(watcher.pinned_feeds()),
            quit: None,
        }
    }
}

impl ViewComponent for RootComponent {
    fn view(&self) -> View {
        View::Root
    }
    fn activate(&mut self) -> Result<()> {
        let (tx, mut rx) = (self.action_tx.clone(), self.watcher.subscribe());
        let (quit_tx, mut quit_rx) = oneshot::channel();
        self.quit = Some(quit_tx);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    changed = rx.changed() => {
                        match changed {
                            Ok(()) => {
                                if let Err(e) = tx.send(Action::Update(Box::new(Data::SavedFeeds(
                                    rx.borrow_and_update().clone(),
                                )))) {
                                    log::error!("failed to send update action: {e}");
                                }
                            }
                            Err(e) => {
                                log::warn!("changed channel error: {e}");
                                break;
                            }
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
            Action::NextItem if !self.items.is_empty() => {
                self.state.select(Some(
                    self.state
                        .selected()
                        .map(|s| (s + 1).min(self.items.len()))
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
            Action::Enter if !self.items.is_empty() => {
                if let Some(index) = self.state.selected() {
                    if index == self.items.len() {
                        self.deactivate()?;
                        return Ok(Some(Action::Logout));
                    }
                    if let Some(feed) = self.items.get(index) {
                        return Ok(Some(Action::Transition(Transition::Push(Box::new(
                            View::Feed(Box::new(feed.info.clone())),
                        )))));
                    }
                }
            }
            Action::Refresh => {
                self.watcher.refresh();
            }
            Action::Update(data) => {
                let Data::SavedFeeds(feeds) = data.as_ref() else {
                    return Ok(None);
                };
                self.items.clone_from(feeds);
                if self.state.selected().is_none() && !self.items.is_empty() {
                    self.state.select(Some(0));
                }
                return Ok(Some(Action::Render));
            }
            _ => {}
        }
        Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let mut items = self
            .items
            .iter()
            .map(|feed| match &feed.info {
                FeedSourceInfo::Feed(generator_view) => Text::from(vec![
                    Line::from(vec![
                        Span::from("[feed]").blue(),
                        Span::from(" "),
                        Span::from(generator_view.display_name.clone()).bold(),
                        Span::from(" "),
                        Span::from(format!(
                            "by {}",
                            profile_name_as_str(&generator_view.creator)
                        ))
                        .gray(),
                    ]),
                    Line::from(format!(
                        "  {}",
                        generator_view.description.as_deref().unwrap_or_default()
                    ))
                    .dim(),
                ]),
                FeedSourceInfo::List(list_view) => Text::from(vec![
                    Line::from(vec![
                        Span::from("[list]").yellow(),
                        Span::from(" "),
                        Span::from(list_view.name.as_str()).bold(),
                        Span::from(" "),
                        Span::from(format!("by {}", profile_name_as_str(&list_view.creator)))
                            .gray(),
                    ]),
                    Line::from(format!(
                        "  {}",
                        list_view.description.as_deref().unwrap_or_default()
                    ))
                    .dim(),
                ]),
                FeedSourceInfo::Timeline(_) => Text::from(vec![
                    Line::from(vec![
                        Span::from("[timeline]").green(),
                        Span::from(" "),
                        Span::from("Following").bold(),
                    ]),
                    Line::from("  Your following feed").dim(),
                ]),
            })
            .collect::<Vec<_>>();
        if !items.is_empty() {
            items.push(Text::from("Sign out").red());
        }
        f.render_stateful_widget(
            List::new(items)
                .block(Block::default().padding(Padding::uniform(1)))
                .highlight_style(Style::default().reset().reversed()),
            area,
            &mut self.state,
        );
        Ok(())
    }
}
