use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputEvent {
    MouseMove { x: f32, y: f32 },
    MouseDown { button: u8, x: f32, y: f32 },
    MouseUp { button: u8, x: f32, y: f32 },
    Wheel { dx: f32, dy: f32 },
    KeyDown { key: String, modifiers: u8 },
    KeyUp { key: String, modifiers: u8 },
}

pub trait InputInjector {
    fn inject(&self, event: &InputEvent);
}

#[derive(Debug, Default)]
pub struct NativeInjector;

impl InputInjector for NativeInjector {
    fn inject(&self, _event: &InputEvent) {
        // Platform backends land in 1.0: CGEvent (macOS) / SendInput (Windows).
    }
}

pub fn map_key_for_os(key: &str, target_os: &str) -> String {
    match (key, target_os) {
        ("Meta", "windows") => "Control".into(),
        ("Control", "macos") => "Meta".into(),
        ("Alt", "macos") => "Alt".into(),
        other => other.0.to_string(),
    }
}
