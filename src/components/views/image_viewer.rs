use super::ViewComponent;
use super::types::{Action, Data, Transition, View};
use crate::image::{ImageCache, ImageData, ImageRenderer, ThumbnailStore};
use color_eyre::Result;
use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Stylize};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Clear, Padding, Paragraph, Wrap};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoadingState {
    Loading,
    Loaded(ImageData),
    Error(String),
}

pub struct ImageViewerComponent {
    image_url: String,
    alt_text: Option<String>,
    loading_state: LoadingState,
    renderer: Arc<ImageRenderer>,
    image_cache: ImageCache,
    thumbnail_cache: ThumbnailStore,
    action_tx: UnboundedSender<Action>,
    quit: Option<oneshot::Sender<()>>,
}

impl ImageViewerComponent {
    pub fn new(
        action_tx: UnboundedSender<Action>,
        image_url: String,
        alt_text: Option<String>,
        renderer: Arc<ImageRenderer>,
        image_cache: ImageCache,
        thumbnail_cache: ThumbnailStore,
    ) -> Self {
        Self {
            image_url,
            alt_text,
            loading_state: LoadingState::Loading,
            renderer,
            image_cache,
            thumbnail_cache,
            action_tx,
            quit: None,
        }
    }

    fn start_loading(&mut self) {
        let (quit_tx, _quit_rx) = oneshot::channel();

        let tx = self.action_tx.clone();
        let url = self.image_url.clone();
        let cache = Arc::clone(&self.image_cache);
        let thumbnail_cache = Arc::clone(&self.thumbnail_cache);
        let renderer = Arc::clone(&self.renderer);

        self.quit = Some(quit_tx);

        tokio::spawn(async move {
            // Fetch image with caching
            let bytes = match crate::image::get_or_fetch_image(&cache, &url).await {
                Ok(b) => b,
                Err(e) => {
                    let _ = tx.send(Action::Update(Box::new(Data::ImageError(format!("Failed to fetch image: {}", e)))));
                    return;
                }
            };

            // Create thumbnail for fast rendering
            match renderer.create_thumbnail(&bytes) {
                Ok(thumbnail) => {
                    // Store thumbnail in cache
                    {
                        let mut thumb_guard = thumbnail_cache.lock().await;
                        thumb_guard.insert(url.clone(), thumbnail);
                    }
                    let _ = tx.send(Action::Update(Box::new(Data::ImageLoaded(ImageData::new(bytes, url)))));
                }
                Err(e) => {
                    let _ = tx.send(Action::Update(Box::new(Data::ImageError(format!("Failed to create thumbnail: {}", e)))));
                }
            }
        });
    }
}

impl ViewComponent for ImageViewerComponent {
    fn view(&self) -> View {
        View::ImageGallery(vec![(self.image_url.clone(), self.alt_text.clone())])
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        match key.code {
            ratatui::crossterm::event::KeyCode::Esc | ratatui::crossterm::event::KeyCode::Backspace | ratatui::crossterm::event::KeyCode::Char('q') => {
                Ok(Some(Action::Back))
            }
            // Press 'o' to open image in external viewer
            ratatui::crossterm::event::KeyCode::Char('o') => {
                let _ = open::that(self.image_url.clone());
                Ok(None)
            }
            _ => Ok(None),
        }
    }
    
    fn activate(&mut self) -> Result<()> {
        self.start_loading();
        Ok(())
    }
    
    fn deactivate(&mut self) -> Result<()> {
        if let Some(tx) = self.quit.take() {
            let _ = tx.send(());
        }
        Ok(())
    }
    
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Back => {
                return Ok(Some(Action::Transition(Transition::Pop)));
            }
            Action::Update(data) => {
                match data.as_ref() {
                    Data::ImageLoaded(image_data) => {
                        self.loading_state = LoadingState::Loaded(image_data.clone());
                        return Ok(Some(Action::Render));
                    }
                    Data::ImageError(msg) => {
                        self.loading_state = LoadingState::Error(msg.clone());
                        return Ok(Some(Action::Render));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(None)
    }
    
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        // Clear the background
        f.render_widget(Clear, area);

        // Create a centered block for the image viewer
        let block = Block::bordered()
            .title(format!(" Image Viewer - {} ", self.renderer.protocol_name()).bold())
            .title_alignment(Alignment::Center)
            .border_style(Color::Cyan)
            .padding(Padding::uniform(1));

        let inner_area = block.inner(area);
        f.render_widget(block, area);

        match &self.loading_state {
            LoadingState::Loading => {
                let text = Text::from(vec![
                    Line::from(""),
                    Line::from("⏳ Loading image...").yellow().centered(),
                    Line::from(""),
                    Line::from(self.image_url.as_str()).dark_gray().centered(),
                ]);
                let paragraph = Paragraph::new(text)
                    .alignment(Alignment::Center)
                    .wrap(Wrap { trim: true });
                f.render_widget(paragraph, inner_area);
            }
            LoadingState::Error(msg) => {
                let text = Text::from(vec![
                    Line::from(""),
                    Line::from("❌ Error loading image").red().bold().centered(),
                    Line::from(""),
                    Line::from(msg.as_str()).red().centered(),
                    Line::from(""),
                    Line::from("Press Esc or q to go back").dark_gray().centered(),
                ]);
                let paragraph = Paragraph::new(text)
                    .alignment(Alignment::Center)
                    .wrap(Wrap { trim: true });
                f.render_widget(paragraph, inner_area);
            }
            LoadingState::Loaded(image_data) => {
                // Get thumbnail from cache and render
                let thumbnail_cache = Arc::clone(&self.thumbnail_cache);
                let url = self.image_url.clone();
                let renderer = Arc::clone(&self.renderer);
                
                // Try to get cached thumbnail
                let mut cached_thumbnail = None;
                {
                    let thumb_guard = futures_executor::block_on(thumbnail_cache.lock());
                    if let Some(thumb) = thumb_guard.get(&url) {
                        cached_thumbnail = Some(thumb.thumbnail.clone());
                    }
                }

                if let Some(thumb) = cached_thumbnail {
                    // Create protocol from thumbnail and render
                    let mut protocol = renderer.create_protocol_from_thumbnail(&thumb);
                    let image_widget = ratatui_image::StatefulImage::<ratatui_image::protocol::StatefulProtocol>::default();
                    f.render_stateful_widget(
                        image_widget,
                        inner_area,
                        &mut protocol,
                    );
                } else {
                    // Fallback: render from bytes (slow)
                    match self.renderer.create_image_state(&image_data.bytes) {
                        Ok(mut protocol) => {
                            let image_widget = ratatui_image::StatefulImage::<ratatui_image::protocol::StatefulProtocol>::default();
                            f.render_stateful_widget(
                                image_widget,
                                inner_area,
                                &mut protocol,
                            );
                        }
                        Err(e) => {
                            let text = Text::from(vec![
                                Line::from(""),
                                Line::from("❌ Failed to render image").red().centered(),
                                Line::from(format!("Error: {}", e)).red().centered(),
                            ]);
                            f.render_widget(Paragraph::new(text).alignment(Alignment::Center), inner_area);
                        }
                    }
                }

                // Show image info at bottom
                let mut info_lines = vec![
                    Line::from(format!("URL: {}", self.image_url)).dark_gray(),
                ];
                if let Some((w, h)) = image_data.dimensions() {
                    info_lines.push(Line::from(format!("Dimensions: {}x{}", w, h)).dark_gray());
                }
                if let Some(alt) = &self.alt_text {
                    info_lines.push(Line::from(format!("Alt: {}", alt)).dark_gray());
                }
                info_lines.push(Line::from("Press 'o' to open in external viewer | Esc/q: Back").dark_gray());
                let info = Paragraph::new(Text::from(info_lines));
                f.render_widget(info, Rect {
                    x: inner_area.x,
                    y: inner_area.y + inner_area.height - 3,
                    width: inner_area.width,
                    height: 3,
                });
            }
        }

        Ok(())
    }
}

/// A simpler image viewer that displays multiple images in a gallery
pub struct ImageGalleryComponent {
    image_urls: Vec<(String, Option<String>)>,
    current_index: usize,
    renderer: Arc<ImageRenderer>,
    image_cache: ImageCache,
    thumbnail_cache: ThumbnailStore,
    action_tx: UnboundedSender<Action>,
    quit: Option<oneshot::Sender<()>>,
    loading_states: Vec<LoadingState>,
}

impl ImageGalleryComponent {
    pub fn new(
        action_tx: UnboundedSender<Action>,
        image_urls: Vec<(String, Option<String>)>,
        renderer: Arc<ImageRenderer>,
        image_cache: ImageCache,
        thumbnail_cache: ThumbnailStore,
    ) -> Self {
        let len = image_urls.len();
        Self {
            image_urls,
            current_index: 0,
            renderer,
            image_cache,
            thumbnail_cache,
            action_tx,
            quit: None,
            loading_states: vec![LoadingState::Loading; len],
        }
    }

    fn current_image(&self) -> Option<&(String, Option<String>)> {
        self.image_urls.get(self.current_index)
    }

    fn preload_adjacent_images(&self) {
        // No preloading - keep it simple
    }

    fn start_loading_current(&mut self) {
        if let Some((url, _)) = self.current_image().cloned() {
            let (quit_tx, _quit_rx) = oneshot::channel();

            let tx = self.action_tx.clone();
            let index = self.current_index;
            let cache = Arc::clone(&self.image_cache);
            let thumbnail_cache = Arc::clone(&self.thumbnail_cache);
            let renderer = Arc::clone(&self.renderer);

            self.quit = Some(quit_tx);

            tokio::spawn(async move {
                // Fetch image with caching
                let bytes = match crate::image::get_or_fetch_image(&cache, &url).await {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = tx.send(Action::Update(Box::new(Data::GalleryImageError(index, format!("Failed to fetch: {}", e)))));
                        return;
                    }
                };

                // Create thumbnail for fast rendering
                match renderer.create_thumbnail(&bytes) {
                    Ok(thumbnail) => {
                        // Store thumbnail in cache
                        {
                            let mut thumb_guard = thumbnail_cache.lock().await;
                            thumb_guard.insert(url.clone(), thumbnail);
                        }
                        let _ = tx.send(Action::Update(Box::new(Data::GalleryImageLoaded(index, ImageData::new(bytes, url)))));
                    }
                    Err(e) => {
                        let _ = tx.send(Action::Update(Box::new(Data::GalleryImageError(index, format!("Failed to create thumbnail: {}", e)))));
                    }
                }
            });
        }
    }
}

impl ViewComponent for ImageGalleryComponent {
    fn view(&self) -> View {
        View::ImageGallery(self.image_urls.clone())
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        match key.code {
            ratatui::crossterm::event::KeyCode::Esc | ratatui::crossterm::event::KeyCode::Backspace | ratatui::crossterm::event::KeyCode::Char('q') => {
                Ok(Some(Action::Back))
            }
            // Press 'o' to open current image in external viewer
            ratatui::crossterm::event::KeyCode::Char('o') => {
                if let Some((url, _)) = self.current_image() {
                    let _ = open::that(url.clone());
                }
                Ok(None)
            }
            ratatui::crossterm::event::KeyCode::Left | ratatui::crossterm::event::KeyCode::Char('h') => {
                if self.current_index > 0 {
                    self.current_index -= 1;
                    self.start_loading_current();
                    Ok(Some(Action::Render))
                } else {
                    Ok(None)
                }
            }
            ratatui::crossterm::event::KeyCode::Right | ratatui::crossterm::event::KeyCode::Char('l') => {
                if self.current_index < self.image_urls.len() - 1 {
                    self.current_index += 1;
                    self.start_loading_current();
                    Ok(Some(Action::Render))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }
    
    fn activate(&mut self) -> Result<()> {
        self.start_loading_current();
        Ok(())
    }
    
    fn deactivate(&mut self) -> Result<()> {
        if let Some(tx) = self.quit.take() {
            let _ = tx.send(());
        }
        Ok(())
    }
    
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Back => {
                return Ok(Some(Action::Transition(Transition::Pop)));
            }
            Action::Update(data) => {
                match data.as_ref() {
                    Data::GalleryImageLoaded(index, image_data) => {
                        if let Some(state) = self.loading_states.get_mut(*index) {
                            *state = LoadingState::Loaded(image_data.clone());
                        }
                        return Ok(Some(Action::Render));
                    }
                    Data::GalleryImageError(index, msg) => {
                        if let Some(state) = self.loading_states.get_mut(*index) {
                            *state = LoadingState::Error(msg.clone());
                        }
                        return Ok(Some(Action::Render));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(None)
    }
    
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        f.render_widget(Clear, area);

        let total = self.image_urls.len();
        let current = self.current_index + 1;

        let block = Block::bordered()
            .title(format!(" Image Gallery ({}/{}) ", current, total).bold())
            .title_alignment(Alignment::Center)
            .border_style(Color::Cyan)
            .padding(Padding::uniform(1));

        let inner_area = block.inner(area);
        f.render_widget(block, area);

        if let Some((url, alt)) = self.current_image() {
            let state = self.loading_states.get(self.current_index).cloned()
                .unwrap_or(LoadingState::Error("Unknown state".to_string()));

            match state {
                LoadingState::Loading => {
                    let text = Text::from(vec![
                        Line::from(""),
                        Line::from(format!("⏳ Loading image {}/{}...", current, total)).yellow().centered(),
                        Line::from(""),
                        Line::from(url.as_str()).dark_gray().centered(),
                    ]);
                    f.render_widget(Paragraph::new(text).alignment(Alignment::Center), inner_area);
                }
                LoadingState::Error(msg) => {
                    let text = Text::from(vec![
                        Line::from(""),
                        Line::from("❌ Error").red().bold().centered(),
                        Line::from(msg.as_str()).red().centered(),
                        Line::from(""),
                        Line::from("←/→ or h/l: Navigate | Esc/q: Back").dark_gray().centered(),
                    ]);
                    f.render_widget(Paragraph::new(text).alignment(Alignment::Center), inner_area);
                }
                LoadingState::Loaded(image_data) => {
                    // Get thumbnail from cache and render
                    let thumbnail_cache = Arc::clone(&self.thumbnail_cache);
                    let url = url.clone();
                    let renderer = Arc::clone(&self.renderer);
                    
                    // Try to get cached thumbnail
                    let mut cached_thumbnail = None;
                    {
                        let thumb_guard = futures_executor::block_on(thumbnail_cache.lock());
                        if let Some(thumb) = thumb_guard.get(&url) {
                            cached_thumbnail = Some(thumb.thumbnail.clone());
                        }
                    }

                    if let Some(thumb) = cached_thumbnail {
                        // Create protocol from thumbnail and render
                        let mut protocol = renderer.create_protocol_from_thumbnail(&thumb);
                        let image_widget = ratatui_image::StatefulImage::<ratatui_image::protocol::StatefulProtocol>::default();
                        f.render_stateful_widget(
                            image_widget,
                            inner_area,
                            &mut protocol,
                        );
                    } else {
                        // Fallback: render from bytes (slow)
                        match self.renderer.create_image_state(&image_data.bytes) {
                            Ok(mut protocol) => {
                                let image_widget = ratatui_image::StatefulImage::<ratatui_image::protocol::StatefulProtocol>::default();
                                f.render_stateful_widget(
                                    image_widget,
                                    inner_area,
                                    &mut protocol,
                                );
                            }
                            Err(e) => {
                                let text = Text::from(vec![
                                    Line::from(""),
                                    Line::from("❌ Failed to render image").red().centered(),
                                    Line::from(format!("Error: {}", e)).red().centered(),
                                ]);
                                f.render_widget(Paragraph::new(text).alignment(Alignment::Center), inner_area);
                            }
                        }
                    }

                    // Show image info at bottom
                    let mut info_lines = vec![
                        Line::from(format!("Image {}/{}", current, total)).dark_gray(),
                    ];
                    if let Some((w, h)) = image_data.dimensions() {
                        info_lines.push(Line::from(format!("Dimensions: {}x{}", w, h)).dark_gray());
                    }
                    if let Some(alt) = alt {
                        info_lines.push(Line::from(format!("Alt: {}", alt)).dark_gray());
                    }
                    info_lines.push(Line::from("Press 'o' to open | ←/→ or h/l: Navigate | Esc/q: Back").dark_gray());
                    let info = Paragraph::new(Text::from(info_lines));
                    f.render_widget(info, Rect {
                        x: inner_area.x,
                        y: inner_area.y + inner_area.height - 3,
                        width: inner_area.width,
                        height: 3,
                    });
                }
            }
        }

        Ok(())
    }
}
