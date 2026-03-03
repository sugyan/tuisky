use super::ViewComponent;
use super::types::{Action, Data, Transition, View};
use crate::image::{ImageCache, ImageData, ImageRenderer};
use color_eyre::Result;
use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
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
    image_url: String,  // 缩略图 URL
    alt_text: Option<String>,
    loading_state: LoadingState,
    renderer: Arc<ImageRenderer>,
    image_cache: ImageCache,
    original_url: String,  // 原图 URL，用于 'o' 键打开
    action_tx: UnboundedSender<Action>,
    quit: Option<oneshot::Sender<()>>,
    /// 当前期望显示的 URL，用于防抖
    expected_url: Option<String>,
}

impl ImageViewerComponent {
    pub fn new(
        action_tx: UnboundedSender<Action>,
        image_url: String,
        alt_text: Option<String>,
        renderer: Arc<ImageRenderer>,
        image_cache: ImageCache,
    ) -> Self {
        // 保存原图 URL（将 /feed_thumbnail 替换回 /feed_fullsize）
        let original_url = image_url
            .replace("/feed_thumbnail", "/feed_fullsize")
            .replace("/thumbnail", "/fullsize");
        
        Self {
            image_url,
            alt_text,
            loading_state: LoadingState::Loading,
            renderer,
            image_cache,
            original_url,
            expected_url: None,
            action_tx,
            quit: None,
        }
    }

    fn start_loading(&mut self) {
        // 更新期望的 URL
        self.expected_url = Some(self.image_url.clone());
        
        let (quit_tx, _quit_rx) = oneshot::channel();

        let tx = self.action_tx.clone();
        let url = self.image_url.clone();
        let cache = Arc::clone(&self.image_cache);

        log::info!("Starting image load: {}", url);

        self.quit = Some(quit_tx);

        tokio::spawn(async move {
            // Fetch image with caching
            let bytes = match crate::image::get_or_fetch_image(&cache, &url).await {
                Ok(b) => {
                    log::info!("Image fetched successfully: {} bytes", b.len());
                    b
                },
                Err(e) => {
                    log::error!("Failed to fetch image: {}", e);
                    let _ = tx.send(Action::Update(Box::new(Data::ImageError(format!("Failed to fetch image: {}", e)))));
                    return;
                }
            };

            // 直接发送加载完成（带 URL，用于防抖检查）
            log::info!("Image load complete: {}", url);
            let _ = tx.send(Action::Update(Box::new(Data::ImageLoadedWithUrl(ImageData::new(bytes, url.clone()), url))));
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
            // Press 'o' to open image in external viewer (use original fullsize URL)
            ratatui::crossterm::event::KeyCode::Char('o') => {
                let _ = open::that(self.original_url.clone());
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
                        log::info!("ImageLoaded: url={}", image_data.url);
                        self.loading_state = LoadingState::Loaded(image_data.clone());
                        return Ok(Some(Action::Render));
                    }
                    Data::ImageLoadedWithUrl(image_data, url) => {
                        // 防抖检查：只处理期望的 URL
                        log::info!("ImageLoadedWithUrl: url={}, expected={:?}", url, self.expected_url);
                        if self.expected_url.as_ref() == Some(url) {
                            log::info!("URL matches expected, displaying image");
                            self.loading_state = LoadingState::Loaded(image_data.clone());
                            return Ok(Some(Action::Render));
                        } else {
                            log::warn!("URL mismatch, discarding stale image: {} != {:?}", url, self.expected_url);
                            return Ok(None);
                        }
                    }
                    Data::ImageError(msg) => {
                        log::error!("ImageError: msg={}", msg);
                        self.loading_state = LoadingState::Error(msg.clone());
                        return Ok(Some(Action::Render));
                    }
                    _ => {
                        log::debug!("Unhandled data: {:?}", data);
                    }
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
                // Create protocol from image bytes and render
                // Clear the area first to avoid image artifacts in zellij
                f.render_widget(Clear, inner_area);
                
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
    image_urls: Vec<(String, Option<String>)>,  // 缩略图 URL
    current_index: usize,
    renderer: Arc<ImageRenderer>,
    image_cache: ImageCache,
    action_tx: UnboundedSender<Action>,
    quit: Option<oneshot::Sender<()>>,
    loading_states: Vec<LoadingState>,
    /// 原图 URLs，用于 'o' 键打开
    original_urls: Vec<String>,
    /// 当前期望显示的 URL，用于防抖
    expected_url: Option<String>,
}

impl ImageGalleryComponent {
    pub fn new(
        action_tx: UnboundedSender<Action>,
        image_urls: Vec<(String, Option<String>)>,
        renderer: Arc<ImageRenderer>,
        image_cache: ImageCache,
    ) -> Self {
        let len = image_urls.len();
        // 保存原图 URLs
        let original_urls = image_urls.iter().map(|(url, _)| {
            url.replace("/feed_thumbnail", "/feed_fullsize")
                .replace("/thumbnail", "/fullsize")
        }).collect();

        Self {
            image_urls,
            current_index: 0,
            renderer,
            image_cache,
            action_tx,
            quit: None,
            loading_states: vec![LoadingState::Loading; len],
            original_urls,
            expected_url: None,
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
            // 更新期望的 URL
            self.expected_url = Some(url.clone());
            log::info!("[GALLERY] start_loading_current: index={}, url={}", self.current_index, url);
            
            // 先设置为 Loading 状态，避免显示旧图片
            if let Some(state) = self.loading_states.get_mut(self.current_index) {
                *state = LoadingState::Loading;
                log::info!("[GALLERY] State set to Loading for index={}", self.current_index);
            }
            
            let (quit_tx, _quit_rx) = oneshot::channel();

            let tx = self.action_tx.clone();
            let index = self.current_index;
            let cache = Arc::clone(&self.image_cache);
            let expected_url = url.clone();

            log::info!("[GALLERY] Spawning fetch task: index={}, url={}", index, expected_url);

            self.quit = Some(quit_tx);

            tokio::spawn(async move {
                log::info!("[GALLERY] Async task started: index={}, url={}", index, expected_url);
                
                // Fetch image with caching
                let bytes = match crate::image::get_or_fetch_image(&cache, &expected_url).await {
                    Ok(b) => {
                        log::info!("[GALLERY] Image fetched: index={}, url={}, {} bytes", index, expected_url, b.len());
                        b
                    },
                    Err(e) => {
                        log::error!("[GALLERY] Failed to fetch: index={}, url={}, error={}", index, expected_url, e);
                        let _ = tx.send(Action::Update(Box::new(Data::GalleryImageError(index, format!("Failed to fetch: {}", e)))));
                        return;
                    }
                };

                // 直接发送加载完成（带 URL，用于防抖检查）
                log::info!("[GALLERY] Sending GalleryImageLoadedWithUrl: index={}, url={}", index, expected_url);
                let _ = tx.send(Action::Update(Box::new(Data::GalleryImageLoadedWithUrl(index, ImageData::new(bytes, expected_url.clone()), expected_url))));
                log::info!("[GALLERY] Message sent successfully");
            });
        } else {
            log::warn!("[GALLERY] start_loading_current: No image at current index");
        }
    }
}

impl ViewComponent for ImageGalleryComponent {
    fn view(&self) -> View {
        View::ImageGallery(self.image_urls.clone())
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        log::info!("[GALLERY] handle_key_events: key={:?}, current_index={}", key.code, self.current_index);
        
        match key.code {
            ratatui::crossterm::event::KeyCode::Esc | ratatui::crossterm::event::KeyCode::Backspace | ratatui::crossterm::event::KeyCode::Char('q') => {
                log::info!("[GALLERY] Back/Quit key pressed");
                Ok(Some(Action::Back))
            }
            // Press 'o' to open current image in external viewer (use original fullsize URL)
            ratatui::crossterm::event::KeyCode::Char('o') => {
                if let Some(url) = self.original_urls.get(self.current_index) {
                    log::info!("[GALLERY] Opening in external viewer: {}", url);
                    let _ = open::that(url.clone());
                }
                Ok(None)
            }
            ratatui::crossterm::event::KeyCode::Left | ratatui::crossterm::event::KeyCode::Char('h') => {
                if self.current_index > 0 {
                    self.current_index -= 1;
                    log::info!("[GALLERY] Moving to previous image: new_index={}", self.current_index);
                    self.start_loading_current();
                    Ok(Some(Action::Render))
                } else {
                    log::debug!("[GALLERY] Already at first image");
                    Ok(None)
                }
            }
            ratatui::crossterm::event::KeyCode::Right | ratatui::crossterm::event::KeyCode::Char('l') => {
                if self.current_index < self.image_urls.len() - 1 {
                    self.current_index += 1;
                    log::info!("[GALLERY] Moving to next image: new_index={}", self.current_index);
                    self.start_loading_current();
                    Ok(Some(Action::Render))
                } else {
                    log::debug!("[GALLERY] Already at last image");
                    Ok(None)
                }
            }
            other => {
                log::debug!("[GALLERY] Unhandled key: {:?}", other);
                Ok(None)
            }
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
        log::debug!("[GALLERY] update: action={:?}", action);
        
        match action {
            Action::Back => {
                log::info!("[GALLERY] Back action, returning Transition::Pop");
                return Ok(Some(Action::Transition(Transition::Pop)));
            }
            Action::Update(data) => {
                match data.as_ref() {
                    Data::GalleryImageLoaded(index, image_data) => {
                        log::info!("[GALLERY] GalleryImageLoaded: index={}, url={}", index, image_data.url);
                        if let Some(state) = self.loading_states.get_mut(*index) {
                            *state = LoadingState::Loaded(image_data.clone());
                            log::info!("[GALLERY] State updated to Loaded, returning Render");
                        }
                        return Ok(Some(Action::Render));
                    }
                    Data::GalleryImageLoadedWithUrl(index, image_data, url) => {
                        // 防抖检查：只处理期望的 URL
                        log::info!("[GALLERY] GalleryImageLoadedWithUrl: index={}, url={}, expected={:?}", index, url, self.expected_url);
                        if self.expected_url.as_ref() == Some(url) {
                            log::info!("[GALLERY] URL matches expected, displaying image");
                            if let Some(state) = self.loading_states.get_mut(*index) {
                                *state = LoadingState::Loaded(image_data.clone());
                                log::info!("[GALLERY] State updated to Loaded, returning Render");
                            }
                            return Ok(Some(Action::Render));
                        } else {
                            log::warn!("[GALLERY] URL mismatch, discarding stale image: {} != {:?}", url, self.expected_url);
                            return Ok(None);
                        }
                    }
                    Data::GalleryImageError(index, msg) => {
                        log::error!("[GALLERY] GalleryImageError: index={}, msg={}", index, msg);
                        if let Some(state) = self.loading_states.get_mut(*index) {
                            *state = LoadingState::Error(msg.clone());
                        }
                        return Ok(Some(Action::Render));
                    }
                    other => {
                        log::debug!("[GALLERY] Unhandled data: {:?}", other);
                    }
                }
            }
            other => {
                log::debug!("[GALLERY] Unhandled action: {:?}", other);
            }
        }
        log::debug!("[GALLERY] update: returning Ok(None)");
        Ok(None)
    }
    
    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        log::debug!("[GALLERY] draw: area={:?}, current_index={}", area, self.current_index);
        
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

        // Split inner area: top for loading/info, bottom for image
        let areas = Layout::vertical([
            Constraint::Length(3),  // Loading/info area (3 lines)
            Constraint::Min(10),    // Image area (remaining space)
        ]).split(inner_area);
        let info_area = areas[0];
        let image_area = areas[1];

        if let Some((url, alt)) = self.current_image() {
            let state = self.loading_states.get(self.current_index).cloned()
                .unwrap_or(LoadingState::Error("Unknown state".to_string()));
            
            log::debug!("[GALLERY] draw: current_url={}, state={:?}", url, state);

            match state {
                LoadingState::Loading => {
                    let text = Text::from(vec![
                        Line::from(""),
                        Line::from(format!("⏳ Loading image {}/{}...", current, total)).yellow().centered(),
                        Line::from(""),
                    ]);
                    f.render_widget(Paragraph::new(text).alignment(Alignment::Center), info_area);
                }
                LoadingState::Error(msg) => {
                    let text = Text::from(vec![
                        Line::from(""),
                        Line::from("❌ Error").red().bold().centered(),
                        Line::from(msg.as_str()).red().centered(),
                    ]);
                    f.render_widget(Paragraph::new(text).alignment(Alignment::Center), info_area);
                    
                    // Show navigation hint in image area
                    let nav_text = Text::from("←/→ or h/l: Navigate | Esc/q: Back").dark_gray().centered();
                    f.render_widget(Paragraph::new(nav_text), image_area);
                }
                LoadingState::Loaded(image_data) => {
                    // Create protocol from image bytes and render in image_area
                    // Clear the area first to avoid image artifacts in zellij
                    log::info!("[GALLERY] Rendering Loaded image: url={}, bytes={}", image_data.url, image_data.bytes.len());
                    f.render_widget(Clear, image_area);
                    
                    match self.renderer.create_image_state(&image_data.bytes) {
                        Ok(mut protocol) => {
                            log::info!("[GALLERY] Protocol created, rendering image");
                            let image_widget = ratatui_image::StatefulImage::<ratatui_image::protocol::StatefulProtocol>::default();
                            f.render_stateful_widget(
                                image_widget,
                                image_area,
                                &mut protocol,
                            );
                            log::info!("[GALLERY] Image rendered successfully");
                        }
                        Err(e) => {
                            log::error!("[GALLERY] Failed to create protocol: {}", e);
                            let text = Text::from(vec![
                                Line::from(""),
                                Line::from("❌ Failed to render image").red().centered(),
                                Line::from(format!("Error: {}", e)).red().centered(),
                            ]);
                            f.render_widget(Paragraph::new(text).alignment(Alignment::Center), image_area);
                        }
                    }
                    
                    // Show info in info_area
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
                    f.render_widget(info, info_area);
                }
            }
        }

        Ok(())
    }
}
