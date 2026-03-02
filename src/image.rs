use ratatui::layout::Rect;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::StatefulImage;
use std::io::Cursor;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 缩略图最大宽度（像素），用于终端显示优化
pub const THUMBNAIL_MAX_WIDTH: u32 = 800;
/// 缩略图最大高度（像素）
pub const THUMBNAIL_MAX_HEIGHT: u32 = 600;

/// Image data for caching (bytes can be cloned)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageData {
    pub bytes: Vec<u8>,
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl ImageData {
    pub fn new(bytes: Vec<u8>, url: String) -> Self {
        let dimensions = get_image_dimensions(&bytes);
        Self {
            bytes,
            url,
            width: dimensions.map(|(w, _)| w),
            height: dimensions.map(|(_, h)| h),
        }
    }

    pub fn dimensions(&self) -> Option<(u32, u32)> {
        self.width.zip(self.height)
    }
}

/// 缓存的缩略图（解码后的图片）
pub struct ThumbnailCache {
    pub thumbnail: image::DynamicImage,
    pub width: u32,
    pub height: u32,
}

/// Image renderer that supports multiple terminal graphics protocols
pub struct ImageRenderer {
    picker: Picker,
    protocol_type: ProtocolType,
}

impl ImageRenderer {
    /// Create a new image renderer with auto-detected protocol
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Try to detect the best protocol for the current terminal
        let picker = Picker::from_query_stdio()?;
        let protocol_type = picker.protocol_type();

        log::info!("Detected image protocol: {:?}", protocol_type);

        Ok(Self {
            picker,
            protocol_type,
        })
    }

    /// Create a new image renderer with a specific protocol
    pub fn with_protocol(protocol: ImageProtocol) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        match protocol {
            ImageProtocol::Auto => {
                let picker = Picker::from_query_stdio()?;
                let protocol_type = picker.protocol_type();
                log::info!("Auto-detected image protocol: {:?}", protocol_type);
                Ok(Self {
                    picker,
                    protocol_type,
                })
            }
            ImageProtocol::Kitty => {
                // Force kitty protocol
                let mut picker = Picker::halfblocks();
                picker.set_protocol_type(ProtocolType::Kitty);
                log::info!("Using image protocol: kitty (forced)");
                Ok(Self {
                    picker,
                    protocol_type: ProtocolType::Kitty,
                })
            }
            ImageProtocol::Iterm2 => {
                // Force iterm2 protocol
                let mut picker = Picker::halfblocks();
                picker.set_protocol_type(ProtocolType::Iterm2);
                log::info!("Using image protocol: iterm2 (forced)");
                Ok(Self {
                    picker,
                    protocol_type: ProtocolType::Iterm2,
                })
            }
            ImageProtocol::Sixel => {
                // Force sixel protocol
                let mut picker = Picker::halfblocks();
                picker.set_protocol_type(ProtocolType::Sixel);
                log::info!("Using image protocol: sixel (forced)");
                Ok(Self {
                    picker,
                    protocol_type: ProtocolType::Sixel,
                })
            }
            ImageProtocol::Halfblocks => {
                let picker = Picker::halfblocks();
                log::info!("Using image protocol: halfblocks");
                Ok(Self {
                    picker,
                    protocol_type: ProtocolType::Halfblocks,
                })
            }
        }
    }

    /// Create image state from bytes for rendering
    pub fn create_image_state(&self, image_bytes: &[u8]) -> Result<StatefulProtocol, Box<dyn std::error::Error + Send + Sync>> {
        let img = image::ImageReader::new(Cursor::new(image_bytes))
            .with_guessed_format()
            .map_err(|e| format!("Failed to read image: {e}"))?
            .decode()
            .map_err(|e| format!("Failed to decode image: {e}"))?;

        // Use new_resize_protocol which scales to fit the render area
        let protocol = self.picker.new_resize_protocol(img);

        Ok(protocol)
    }

    /// Create thumbnail from image bytes (scaled down for faster rendering)
    pub fn create_thumbnail(&self, image_bytes: &[u8]) -> Result<ThumbnailCache, String> {
        let mut img = image::ImageReader::new(Cursor::new(image_bytes))
            .with_guessed_format()
            .map_err(|e| format!("Failed to read image: {e}"))?
            .decode()
            .map_err(|e| format!("Failed to decode image: {e}"))?;

        let (orig_width, orig_height) = (img.width(), img.height());

        // Scale down if image is larger than thumbnail max size
        if orig_width > THUMBNAIL_MAX_WIDTH || orig_height > THUMBNAIL_MAX_HEIGHT {
            let scale_w = THUMBNAIL_MAX_WIDTH as f32 / orig_width as f32;
            let scale_h = THUMBNAIL_MAX_HEIGHT as f32 / orig_height as f32;
            let scale = scale_w.min(scale_h);
            
            let new_width = (orig_width as f32 * scale) as u32;
            let new_height = (orig_height as f32 * scale) as u32;
            
            img = img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3);
            
            log::debug!("Created thumbnail: {}x{} -> {}x{}", orig_width, orig_height, new_width, new_height);
        } else {
            log::debug!("Using original size: {}x{}", orig_width, orig_height);
        }

        Ok(ThumbnailCache {
            thumbnail: img,
            width: orig_width,
            height: orig_height,
        })
    }

    /// Create protocol from thumbnail for fast rendering
    pub fn create_protocol_from_thumbnail(&self, thumbnail: &image::DynamicImage) -> StatefulProtocol {
        self.picker.new_resize_protocol(thumbnail.clone())
    }

    /// Get the detected protocol type
    pub fn protocol_type(&self) -> ProtocolType {
        self.protocol_type
    }

    /// Get protocol name as string
    pub fn protocol_name(&self) -> &'static str {
        match self.protocol_type {
            ProtocolType::Kitty => "Kitty",
            ProtocolType::Iterm2 => "iTerm2",
            ProtocolType::Sixel => "Sixel",
            ProtocolType::Halfblocks => "Halfblocks",
        }
    }

    /// Check if the current protocol supports true graphics (not just halfblocks)
    pub fn supports_graphics(&self) -> bool {
        !matches!(self.protocol_type, ProtocolType::Halfblocks)
    }
}

impl Default for ImageRenderer {
    fn default() -> Self {
        // Fallback to halfblocks if detection fails
        let picker = Picker::halfblocks();
        Self {
            picker,
            protocol_type: ProtocolType::Halfblocks,
        }
    }
}

/// Supported image protocols
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageProtocol {
    #[default]
    Auto,
    Kitty,
    Iterm2,
    Sixel,
    Halfblocks,
}

impl ImageProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImageProtocol::Auto => "auto",
            ImageProtocol::Kitty => "kitty",
            ImageProtocol::Iterm2 => "iterm2",
            ImageProtocol::Sixel => "sixel",
            ImageProtocol::Halfblocks => "halfblocks",
        }
    }
}

impl std::str::FromStr for ImageProtocol {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(ImageProtocol::Auto),
            "kitty" => Ok(ImageProtocol::Kitty),
            "iterm2" | "iterm" => Ok(ImageProtocol::Iterm2),
            "sixel" => Ok(ImageProtocol::Sixel),
            "halfblocks" | "half" | "ascii" => Ok(ImageProtocol::Halfblocks),
            _ => Err(format!("Unknown image protocol: {}", s)),
        }
    }
}

/// Render an image from bytes to the terminal
pub fn render_image(
    frame: &mut ratatui::Frame,
    area: Rect,
    protocol: &mut StatefulProtocol,
) -> Result<(), Box<dyn std::error::Error>> {
    let image = StatefulImage::<StatefulProtocol>::default();
    frame.render_stateful_widget(image, area, protocol);
    Ok(())
}

/// Calculate the best image size to fit in the given area while maintaining aspect ratio
pub fn calculate_image_size(
    img_width: u32,
    img_height: u32,
    max_width: u32,
    max_height: u32,
) -> (u32, u32) {
    if img_width <= max_width && img_height <= max_height {
        return (img_width, img_height);
    }

    let width_ratio = max_width as f32 / img_width as f32;
    let height_ratio = max_height as f32 / img_height as f32;
    let scale = width_ratio.min(height_ratio);

    (
        (img_width as f32 * scale) as u32,
        (img_height as f32 * scale) as u32,
    )
}

/// Fetch image data from URL
pub async fn fetch_image(url: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("Failed to fetch image: {e}"))?;

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read image bytes: {e}"))?;

    Ok(bytes.to_vec())
}

/// Check if image bytes are valid
pub fn is_valid_image(bytes: &[u8]) -> bool {
    image::guess_format(bytes).is_ok()
}

/// Get image dimensions without decoding the full image
pub fn get_image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if let Ok(reader) = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format() {
        if let Ok((width, height)) = reader.into_dimensions() {
            return Some((width, height));
        }
    }
    None
}

/// 线程安全的图像缓存
pub type ImageCache = Arc<Mutex<std::collections::HashMap<String, Vec<u8>>>>;

/// 线程安全的缩略图缓存
pub type ThumbnailStore = Arc<Mutex<std::collections::HashMap<String, ThumbnailCache>>>;

/// Create a new image cache
pub fn create_image_cache() -> ImageCache {
    Arc::new(Mutex::new(std::collections::HashMap::new()))
}

/// Create a new thumbnail cache
pub fn create_thumbnail_cache() -> ThumbnailStore {
    Arc::new(Mutex::new(std::collections::HashMap::new()))
}

/// Get image from cache or fetch it
pub async fn get_or_fetch_image(
    cache: &ImageCache,
    url: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    {
        let cache_guard = cache.lock().await;
        if let Some(data) = cache_guard.get(url) {
            log::debug!("Image cache hit for: {}", url);
            return Ok(data.clone());
        }
    }

    log::debug!("Fetching image from: {}", url);
    let data = fetch_image(url).await?;

    {
        let mut cache_guard = cache.lock().await;
        cache_guard.insert(url.to_string(), data.clone());
    }

    Ok(data)
}
