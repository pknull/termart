use crate::colors::scheme_color;
use crate::terminal::Terminal;
use crossterm::style::Color;
use image::{DynamicImage, RgbaImage};
use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread;

/// Maximum size for downloaded cover art (10MB)
const MAX_COVER_SIZE: u64 = 10 * 1024 * 1024;

/// Alpha threshold below which a pixel is considered transparent
const ALPHA_THRESHOLD: u8 = 10;

/// Maximum number of cached cover art images
const MAX_CACHE_SIZE: usize = 50;

/// Cover art cache and loader
pub struct CoverArtLoader {
    cache: HashMap<String, Option<DynamicImage>>,
    cache_order: VecDeque<String>,
    pending: Option<String>,
    cancel_flag: Arc<AtomicBool>,
    receiver: Receiver<(String, Option<DynamicImage>)>,
    sender: Sender<(String, Option<DynamicImage>)>,
}

pub struct CoverRenderCache {
    url: String,
    width: u16,
    height: u16,
    rgba: RgbaImage,
}

impl CoverArtLoader {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();

        Self {
            cache: HashMap::new(),
            cache_order: VecDeque::new(),
            pending: None,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            receiver: rx,
            sender: tx,
        }
    }

    /// Request cover art for a URL (non-blocking)
    pub fn request(&mut self, url: &str) {
        if self.cache.contains_key(url) {
            return;
        }

        if self.pending.as_deref() == Some(url) {
            return;
        }

        self.cancel_flag.store(true, Ordering::Relaxed);
        self.cancel_flag = Arc::new(AtomicBool::new(false));

        let url_owned = url.to_string();
        self.pending = Some(url_owned.clone());
        let tx = self.sender.clone();
        let cancel = self.cancel_flag.clone();

        thread::spawn(move || {
            let result = load_image(&url_owned, &cancel);
            if !cancel.load(Ordering::Relaxed) {
                let _ = tx.send((url_owned, result));
            }
        });
    }

    /// Get cover art if available
    pub fn get(&mut self, url: &str) -> Option<&DynamicImage> {
        // Check for completed loads
        loop {
            match self.receiver.try_recv() {
                Ok((loaded_url, img_opt)) => {
                    if self.pending.as_ref() == Some(&loaded_url) {
                        self.pending = None;
                    }
                    self.cache.insert(loaded_url.clone(), img_opt);
                    self.cache_order.push_back(loaded_url);

                    while self.cache_order.len() > MAX_CACHE_SIZE {
                        if let Some(old_key) = self.cache_order.pop_front() {
                            self.cache.remove(&old_key);
                        }
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }

        self.cache.get(url)?.as_ref()
    }
}

/// Calculate aspect-ratio-preserving cover dimensions for half-block rendering.
/// Returns (art_w, art_h_cells, x_offset, y_offset).
/// art_w × (art_h_cells * 2) is the pixel size to resize the image to.
pub fn calc_cover_dimensions(term_w: u16, term_h: u16) -> (u16, u16, u16, u16) {
    if term_w == 0 || term_h == 0 {
        return (0, 0, 0, 0);
    }

    // Terminal chars are ~2:1 (twice as tall as wide).
    // With half-blocks each cell covers 1 pixel wide × 2 pixels tall.
    // For a visually square result: art_w = 2 * art_h_cells.
    // Image pixel dimensions: art_w × (art_h_cells * 2) = 2h × 2h = square.
    let (art_w, art_h_cells) = if term_h * 2 <= term_w {
        (term_h * 2, term_h)
    } else {
        (term_w, term_w / 2)
    };
    let x_offset = (term_w.saturating_sub(art_w)) / 2;
    let y_offset = 0;

    (art_w, art_h_cells, x_offset, y_offset)
}

pub fn resized_rgba<'a>(
    cache: &'a mut Option<CoverRenderCache>,
    url: &str,
    pixel_w: u16,
    pixel_h: u16,
    image: &DynamicImage,
) -> &'a RgbaImage {
    let needs_update = match cache.as_ref() {
        Some(existing) => existing.url != url || existing.width != pixel_w || existing.height != pixel_h,
        None => true,
    };

    if needs_update {
        let resized = image
            .resize_exact(pixel_w as u32, pixel_h as u32, image::imageops::FilterType::Triangle)
            .to_rgba8();
        *cache = Some(CoverRenderCache {
            url: url.to_string(),
            width: pixel_w,
            height: pixel_h,
            rgba: resized,
        });
    }

    &cache.as_ref().expect("cache must be initialized").rgba
}

/// Render cover art using half-block technique with full RGB colors.
/// `art_w` and `art_h_cells` are in terminal cell units.
/// The image must be `art_w × (art_h_cells * 2)` pixels.
pub fn render_cover_halfblock(
    term: &mut Terminal,
    rgba: &RgbaImage,
    x_offset: u16,
    y_offset: u16,
    art_w: u16,
    art_h_cells: u16,
) {
    if art_w == 0 || art_h_cells == 0 {
        return;
    }

    for cy in 0..art_h_cells as u32 {
        let top_row = cy * 2;
        let bot_row = cy * 2 + 1;

        for cx in 0..art_w as u32 {
            let top_px = rgba.get_pixel(cx, top_row).0;
            let (tr, tg, tb, ta) = (top_px[0], top_px[1], top_px[2], top_px[3]);

            let has_bot = bot_row < rgba.height();
            let (br, bg_c, bb, ba) = if has_bot {
                let bot_px = rgba.get_pixel(cx, bot_row).0;
                (bot_px[0], bot_px[1], bot_px[2], bot_px[3])
            } else {
                (0, 0, 0, 0)
            };

            let tx = x_offset as i32 + cx as i32;
            let ty = y_offset as i32 + cy as i32;

            if ta < ALPHA_THRESHOLD && (!has_bot || ba < ALPHA_THRESHOLD) {
                term.set(tx, ty, ' ', None, false);
            } else if ta < ALPHA_THRESHOLD {
                // Only bottom pixel visible: use lower half block with fg = bottom color
                term.set(tx, ty, '▄', Some(Color::Rgb { r: br, g: bg_c, b: bb }), false);
            } else if !has_bot || ba < ALPHA_THRESHOLD {
                // Only top pixel visible
                term.set(tx, ty, '▀', Some(Color::Rgb { r: tr, g: tg, b: tb }), false);
            } else {
                term.set_with_bg(
                    tx, ty, '▀',
                    Some(Color::Rgb { r: tr, g: tg, b: tb }),
                    Some(Color::Rgb { r: br, g: bg_c, b: bb }),
                    false,
                );
            }
        }
    }
}

/// Render cover art using half-block technique with luminance-mapped scheme colors.
/// Converts each pixel to luminance, then maps through the scheme's color gradient.
pub fn render_cover_halfblock_palette(
    term: &mut Terminal,
    rgba: &RgbaImage,
    x_offset: u16,
    y_offset: u16,
    art_w: u16,
    art_h_cells: u16,
    scheme: u8,
) {
    if art_w == 0 || art_h_cells == 0 {
        return;
    }

    for cy in 0..art_h_cells as u32 {
        let top_row = cy * 2;
        let bot_row = cy * 2 + 1;

        for cx in 0..art_w as u32 {
            let top_px = rgba.get_pixel(cx, top_row).0;
            let has_bot = bot_row < rgba.height();
            let bot_px = if has_bot { rgba.get_pixel(cx, bot_row).0 } else { [0, 0, 0, 0] };

            let tx = x_offset as i32 + cx as i32;
            let ty = y_offset as i32 + cy as i32;

            if top_px[3] < ALPHA_THRESHOLD && (!has_bot || bot_px[3] < ALPHA_THRESHOLD) {
                term.set(tx, ty, ' ', None, false);
                continue;
            }

            let top_color = luminance_to_scheme(scheme, top_px[0], top_px[1], top_px[2]);
            let bot_color = if has_bot && bot_px[3] >= ALPHA_THRESHOLD {
                Some(luminance_to_scheme(scheme, bot_px[0], bot_px[1], bot_px[2]))
            } else {
                None
            };

            if top_px[3] < ALPHA_THRESHOLD {
                if let Some(bc) = bot_color {
                    term.set(tx, ty, '▄', Some(bc), false);
                }
            } else if let Some(bc) = bot_color {
                term.set_with_bg(tx, ty, '▀', Some(top_color), Some(bc), false);
            } else {
                term.set(tx, ty, '▀', Some(top_color), false);
            }
        }
    }
}

/// Map a pixel to its scheme color via luminance banding.
/// Uses the actual Color enum values so the terminal renders them
/// identically to audio bars, text, and other scheme-colored elements.
fn luminance_to_scheme(scheme: u8, r: u8, g: u8, b: u8) -> Color {
    let lum = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32)
        .round() as u8;
    let intensity = match lum {
        0..=30 => return Color::Black,
        31..=95 => 0,
        96..=160 => 1,
        161..=210 => 2,
        _ => 3,
    };
    scheme_color(scheme, intensity, true).0
}

fn load_image(url: &str, cancel: &AtomicBool) -> Option<DynamicImage> {
    if cancel.load(Ordering::Relaxed) {
        return None;
    }

    if url.starts_with("file://") {
        let path = url.strip_prefix("file://")?;
        let path = urlencoding::decode(path).ok()?;
        let bytes = std::fs::read(Path::new(path.as_ref())).ok()?;
        image::load_from_memory(&bytes).ok()
    } else if url.starts_with("http://") || url.starts_with("https://") {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }

        let response = ureq::get(url)
            .timeout(std::time::Duration::from_secs(10))
            .call()
            .ok()?;

        if let Some(len) = response
            .header("Content-Length")
            .and_then(|s| s.parse::<u64>().ok())
        {
            if len > MAX_COVER_SIZE {
                return None;
            }
        }

        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(MAX_COVER_SIZE)
            .read_to_end(&mut bytes)
            .ok()?;

        if cancel.load(Ordering::Relaxed) {
            return None;
        }

        image::load_from_memory(&bytes).ok()
    } else {
        let bytes = std::fs::read(Path::new(url)).ok()?;
        image::load_from_memory(&bytes).ok()
    }
}

