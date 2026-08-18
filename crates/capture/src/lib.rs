use image::imageops::FilterType;
use image::RgbaImage;
use protocol::Quality;
use serde::Serialize;
use thiserror::Error;
use xcap::Monitor;

#[derive(Debug, Clone, Serialize)]
pub struct DisplayInfo {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
}

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Xcap(#[from] xcap::XCapError),
    #[error(transparent)]
    Image(#[from] image::ImageError),
}

pub trait ScreenCapture {
    fn displays(&self) -> Result<Vec<DisplayInfo>, CaptureError>;
    fn capture_primary_jpeg(&self, max_width: u32, quality: u8) -> Result<FrameJpeg, CaptureError>;
}

#[derive(Debug, Clone)]
pub struct FrameJpeg {
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct NativeCapture;

impl ScreenCapture for NativeCapture {
    fn displays(&self) -> Result<Vec<DisplayInfo>, CaptureError> {
        Ok(list_displays()?)
    }

    fn capture_primary_jpeg(&self, max_width: u32, quality: u8) -> Result<FrameJpeg, CaptureError> {
        capture_primary_jpeg(max_width, quality)
    }
}

pub fn list_displays() -> Result<Vec<DisplayInfo>, CaptureError> {
    Monitor::all()?
        .into_iter()
        .enumerate()
        .map(|(idx, monitor)| {
            Ok(DisplayInfo {
                id: idx as u32,
                name: monitor.name().unwrap_or_else(|_| format!("Display {}", idx + 1)),
                width: monitor.width()?,
                height: monitor.height()?,
                is_primary: monitor.is_primary().unwrap_or(idx == 0),
            })
        })
        .collect()
}

pub fn capture_primary_jpeg(max_width: u32, quality: u8) -> Result<FrameJpeg, CaptureError> {
    capture_primary_jpeg_changed(max_width, quality, None).map(|frame| {
        frame.expect("fingerprint disabled always encodes")
    })
}

/// When `last_fp` is set, returns `Ok(None)` if the screen has not changed.
pub fn capture_primary_jpeg_changed(
    max_width: u32,
    quality: u8,
    last_fp: Option<&mut u64>,
) -> Result<Option<FrameJpeg>, CaptureError> {
    match capture_xcap(max_width, quality, last_fp) {
        Ok(frame) => Ok(frame),
        Err(err) => {
            #[cfg(target_os = "macos")]
            if let Ok(frame) = capture_screencapture_macos(max_width, quality) {
                return Ok(Some(frame));
            }
            Err(err)
        }
    }
}

fn capture_xcap(
    max_width: u32,
    quality: u8,
    last_fp: Option<&mut u64>,
) -> Result<Option<FrameJpeg>, CaptureError> {
    let monitors = Monitor::all()?;
    let monitor = monitors
        .iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .or(monitors.first())
        .ok_or_else(|| CaptureError::Message("No display found".into()))?;

    let image = monitor.capture_image()?;
    if let Some(fp) = last_fp {
        let next = fingerprint(&image);
        if *fp == next {
            return Ok(None);
        }
        *fp = next;
    }
    Ok(Some(encode_scaled(image, max_width, quality)?))
}

fn fingerprint(image: &RgbaImage) -> u64 {
    let w = image.width();
    let h = image.height();
    let raw = image.as_raw();
    let mut hash = (w as u64).wrapping_shl(32) ^ h as u64;
    if w == 0 || h == 0 || raw.len() < 4 {
        return hash;
    }
    let stride = (w as usize).saturating_mul(4);
    for row in 0..8 {
        let y = (h as usize - 1) * row / 7;
        let row_off = y.saturating_mul(stride);
        for col in 0..8 {
            let x = (w as usize - 1) * col / 7;
            let i = row_off + x.saturating_mul(4);
            if i + 3 < raw.len() {
                hash = hash
                    .wrapping_mul(1099511628211)
                    .wrapping_add(u32::from_le_bytes([raw[i], raw[i + 1], raw[i + 2], raw[i + 3]]) as u64);
            }
        }
    }
    hash
}

fn encode_scaled(mut image: RgbaImage, max_width: u32, quality: u8) -> Result<FrameJpeg, CaptureError> {
    let (mut width, mut height) = (image.width(), image.height());

    if max_width > 0 && width > max_width {
        let scaled_h = ((height as f32) * (max_width as f32) / (width as f32)).round() as u32;
        let filter = if quality >= 86 {
            FilterType::CatmullRom
        } else {
            FilterType::Triangle
        };
        image = image::imageops::resize(&image, max_width, scaled_h.max(1), filter);
        width = image.width();
        height = image.height();
    }

    let bytes = encode_jpeg(&image, quality.clamp(40, 95))?;
    Ok(FrameJpeg {
        width,
        height,
        bytes,
    })
}

#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(target_os = "macos")]
static LAST_SCREENCAPTURE_MS: AtomicU64 = AtomicU64::new(0);

#[cfg(target_os = "macos")]
fn capture_screencapture_macos(max_width: u32, quality: u8) -> Result<FrameJpeg, CaptureError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let last = LAST_SCREENCAPTURE_MS.load(Ordering::Relaxed);
    if now.saturating_sub(last) < 2500 {
        return Err(CaptureError::Message("screencapture cooldown".into()));
    }
    LAST_SCREENCAPTURE_MS.store(now, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("remotex-frame-{}.jpg", std::process::id()));
    let status = std::process::Command::new("/usr/sbin/screencapture")
        .args(["-x", "-C", "-t", "jpg", path.to_str().unwrap_or("")])
        .status()
        .map_err(|err| CaptureError::Message(err.to_string()))?;
    if !status.success() {
        return Err(CaptureError::Message("screencapture failed".into()));
    }
    let bytes = std::fs::read(&path).map_err(|err| CaptureError::Message(err.to_string()))?;
    let _ = std::fs::remove_file(&path);
    let image = image::load_from_memory(&bytes)
        .map_err(|err| CaptureError::Message(err.to_string()))?
        .to_rgba8();
    encode_scaled(image, max_width, quality)
}

pub fn quality_max_width(quality: &str) -> u32 {
    quality_params(quality).0
}

/// (max_width, jpeg_quality, interval_ms)
pub fn quality_params(quality: &str) -> (u32, u8, u64) {
    match quality {
        "smooth" => (1280, 58, 33),
        "high" | "original" => (1920, 86, 48),
        _ => (1600, 72, 40),
    }
}

pub fn capture_max_width(resolution: &str, viewport_w: u32) -> u32 {
    match resolution {
        "720p" => 1280,
        "1080p" => 1920,
        "original" => 3840,
        _ => viewport_w.clamp(960, 2560),
    }
}

pub fn capture_interval_ms(fps: u32, quality_wait_ms: u64) -> u64 {
    if fps == 0 {
        return quality_wait_ms;
    }
    (1000 / fps.clamp(15, 120) as u64).clamp(8, 66)
}

fn encode_jpeg(image: &RgbaImage, quality: u8) -> Result<Vec<u8>, CaptureError> {
    let mut out = Vec::with_capacity((image.width() * image.height()) as usize / 6);
    let mut encoder = jpeg_encoder::Encoder::new(&mut out, quality);
    encoder.set_sampling_factor(if quality >= 84 {
        jpeg_encoder::SamplingFactor::F_1_1
    } else {
        jpeg_encoder::SamplingFactor::F_2_2
    });
    encoder
        .encode(
            image.as_raw(),
            image.width() as u16,
            image.height() as u16,
            jpeg_encoder::ColorType::Rgba,
        )
        .map_err(|err| CaptureError::Message(err.to_string()))?;
    Ok(out)
}

pub fn quality_label(quality: &Quality) -> &'static str {
    match quality {
        Quality::Smooth => "Smooth",
        Quality::Balanced => "Balanced",
        Quality::High => "High Quality",
        Quality::Original => "Original",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_size_helpers() {
        assert_eq!(capture_max_width("720p", 2000), 1280);
        assert_eq!(capture_max_width("1080p", 800), 1920);
        assert_eq!(capture_max_width("auto", 1120), 1120);
        assert_eq!(capture_interval_ms(60, 40), 16);
    }

    #[test]
    fn list_displays_does_not_panic() {
        let _ = list_displays();
    }
}
