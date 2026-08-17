use protocol::Quality;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DisplayInfo {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
}

pub trait ScreenCapture {
    fn displays(&self) -> Vec<DisplayInfo>;
}

#[derive(Debug, Default)]
pub struct NativeCapture;

impl ScreenCapture for NativeCapture {
    fn displays(&self) -> Vec<DisplayInfo> {
        list_displays()
    }
}

pub fn list_displays() -> Vec<DisplayInfo> {
    vec![DisplayInfo {
        id: 0,
        name: "Display 1".into(),
        width: 1920,
        height: 1080,
        is_primary: true,
    }]
}

pub fn quality_label(quality: &Quality) -> &'static str {
    match quality {
        Quality::Smooth => "Smooth",
        Quality::Balanced => "Balanced",
        Quality::High => "High Quality",
        Quality::Original => "Original",
    }
}
