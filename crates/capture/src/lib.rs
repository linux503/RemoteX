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
    let monitors = Monitor::all()?;
    let monitor = monitors
        .iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .or(monitors.first())
        .ok_or_else(|| CaptureError::Message("No display found".into()))?;

    let mut image = monitor.capture_image()?;
    let (mut width, mut height) = (image.width(), image.height());

    if max_width > 0 && width > max_width {
        let scaled_h = ((height as f32) * (max_width as f32) / (width as f32)).round() as u32;
        image = image::imageops::resize(&image, max_width, scaled_h.max(1), FilterType::Triangle);
        width = image.width();
        height = image.height();
    }

    let bytes = encode_jpeg(&image, quality.clamp(35, 92))?;
    Ok(FrameJpeg {
        width,
        height,
        bytes,
    })
}

fn encode_jpeg(image: &RgbaImage, quality: u8) -> Result<Vec<u8>, CaptureError> {
    let mut rgb = Vec::with_capacity((image.width() * image.height() * 3) as usize);
    for px in image.pixels() {
        rgb.extend_from_slice(&[px[0], px[1], px[2]]);
    }
    let mut out = Vec::new();
    let encoder = jpeg_encoder::Encoder::new(&mut out, quality);
    encoder
        .encode(
            &rgb,
            image.width() as u16,
            image.height() as u16,
            jpeg_encoder::ColorType::Rgb,
        )
        .map_err(|err| CaptureError::Message(err.to_string()))?;
    Ok(out)
}

pub fn quality_max_width(quality: &str) -> u32 {
    match quality {
        "smooth" => 1280,
        "high" => 2560,
        "original" => 0,
        _ => 1600,
    }
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
    fn list_displays_does_not_panic() {
        let _ = list_displays();
    }
}
