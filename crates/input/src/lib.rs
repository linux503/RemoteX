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
    fn inject(&self, event: &InputEvent, screen: (u32, u32));
}

#[derive(Debug, Default)]
pub struct NativeInjector;

impl InputInjector for NativeInjector {
    fn inject(&self, event: &InputEvent, screen: (u32, u32)) {
        platform::inject(event, screen);
    }
}

pub fn inject(event: &InputEvent, screen: (u32, u32)) {
    platform::inject(event, screen);
}

#[cfg(target_os = "macos")]
mod platform {
    use super::InputEvent;
    use std::ffi::c_void;
    use std::ptr;

    type CGEventRef = *mut c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventCreateMouseEvent(
            source: *mut c_void,
            mouse_type: u32,
            point: CGPoint,
            mouse_button: u32,
        ) -> CGEventRef;
        fn CGEventCreateScrollWheelEvent(
            source: *mut c_void,
            units: u32,
            wheel_count: u32,
            wheel1: i32,
            wheel2: i32,
            wheel3: i32,
        ) -> CGEventRef;
        fn CGEventCreateKeyboardEvent(source: *mut c_void, keycode: u16, keydown: bool) -> CGEventRef;
        fn CGEventSetFlags(event: CGEventRef, flags: u64);
        fn CGEventPost(tap: i32, event: CGEventRef) -> ();
        fn CFRelease(cf: *const c_void);
    }

    #[repr(C)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    const K_CG_HID_EVENT_TAP: i32 = 0;
    const K_CG_EVENT_MOUSE_MOVED: u32 = 5;
    const K_CG_EVENT_LEFT_MOUSE_DRAGGED: u32 = 6;
    const K_CG_EVENT_RIGHT_MOUSE_DRAGGED: u32 = 7;
    const K_CG_EVENT_OTHER_MOUSE_DRAGGED: u32 = 27;
    const K_CG_EVENT_LEFT_MOUSE_DOWN: u32 = 1;
    const K_CG_EVENT_LEFT_MOUSE_UP: u32 = 2;
    const K_CG_EVENT_RIGHT_MOUSE_DOWN: u32 = 3;
    const K_CG_EVENT_RIGHT_MOUSE_UP: u32 = 4;
    const K_CG_EVENT_OTHER_MOUSE_DOWN: u32 = 25;
    const K_CG_EVENT_OTHER_MOUSE_UP: u32 = 26;
    const K_CG_SCROLL_EVENT_UNIT_LINE: u32 = 1;

    fn point(screen: (u32, u32), x: f32, y: f32) -> CGPoint {
        CGPoint {
            x: (x.clamp(0.0, 1.0) * screen.0 as f32) as f64,
            y: (y.clamp(0.0, 1.0) * screen.1 as f32) as f64,
        }
    }

    fn mouse_drag_type(button: u8) -> u32 {
        match button {
            2 => K_CG_EVENT_RIGHT_MOUSE_DRAGGED,
            1 => K_CG_EVENT_OTHER_MOUSE_DRAGGED,
            _ => K_CG_EVENT_LEFT_MOUSE_DRAGGED,
        }
    }

    fn post(event: CGEventRef) {
        if event.is_null() {
            return;
        }
        unsafe {
            CGEventPost(K_CG_HID_EVENT_TAP, event);
            CFRelease(event);
        }
    }

    fn mouse_type(down: bool, button: u8) -> u32 {
        match button {
            2 => {
                if down {
                    K_CG_EVENT_RIGHT_MOUSE_DOWN
                } else {
                    K_CG_EVENT_RIGHT_MOUSE_UP
                }
            }
            1 => {
                if down {
                    K_CG_EVENT_OTHER_MOUSE_DOWN
                } else {
                    K_CG_EVENT_OTHER_MOUSE_UP
                }
            }
            _ => {
                if down {
                    K_CG_EVENT_LEFT_MOUSE_DOWN
                } else {
                    K_CG_EVENT_LEFT_MOUSE_UP
                }
            }
        }
    }

    fn keycode(key: &str) -> Option<u16> {
        Some(match key {
            "KeyA" => 0x00,
            "KeyB" => 0x0B,
            "KeyC" => 0x08,
            "KeyD" => 0x02,
            "KeyE" => 0x0E,
            "KeyF" => 0x03,
            "KeyG" => 0x05,
            "KeyH" => 0x04,
            "KeyI" => 0x22,
            "KeyJ" => 0x26,
            "KeyK" => 0x28,
            "KeyL" => 0x25,
            "KeyM" => 0x2E,
            "KeyN" => 0x2D,
            "KeyO" => 0x1F,
            "KeyP" => 0x23,
            "KeyQ" => 0x0C,
            "KeyR" => 0x0F,
            "KeyS" => 0x01,
            "KeyT" => 0x11,
            "KeyU" => 0x20,
            "KeyV" => 0x09,
            "KeyW" => 0x0D,
            "KeyX" => 0x07,
            "KeyY" => 0x10,
            "KeyZ" => 0x06,
            "Digit0" => 0x1D,
            "Digit1" => 0x12,
            "Digit2" => 0x13,
            "Digit3" => 0x14,
            "Digit4" => 0x15,
            "Digit5" => 0x17,
            "Digit6" => 0x16,
            "Digit7" => 0x1A,
            "Digit8" => 0x1C,
            "Digit9" => 0x19,
            "Enter" | "NumpadEnter" => 0x24,
            "Escape" => 0x35,
            "Backspace" => 0x33,
            "Tab" => 0x30,
            "Space" => 0x31,
            "ArrowLeft" => 0x7B,
            "ArrowRight" => 0x7C,
            "ArrowDown" => 0x7D,
            "ArrowUp" => 0x7E,
            "Delete" => 0x75,
            "Home" => 0x73,
            "End" => 0x77,
            "PageUp" => 0x74,
            "PageDown" => 0x79,
            "Minus" => 0x1B,
            "Equal" => 0x18,
            "BracketLeft" => 0x21,
            "BracketRight" => 0x1E,
            "Backslash" => 0x2A,
            "Semicolon" => 0x29,
            "Quote" => 0x27,
            "Comma" => 0x2B,
            "Period" => 0x2F,
            "Slash" => 0x2C,
            "MetaLeft" | "MetaRight" => 0x37,
            "ShiftLeft" | "ShiftRight" => 0x38,
            "ControlLeft" | "ControlRight" => 0x3B,
            "AltLeft" | "AltRight" => 0x3A,
            _ => return None,
        })
    }

    fn modifier_flags(bits: u8) -> u64 {
        let mut flags = 0u64;
        if bits & 1 != 0 {
            flags |= 1 << 18; // shift
        }
        if bits & 2 != 0 {
            flags |= 1 << 20; // control
        }
        if bits & 4 != 0 {
            flags |= 1 << 19; // alt
        }
        if bits & 8 != 0 {
            flags |= 1 << 21; // command
        }
        flags
    }

    pub fn inject(event: &InputEvent, screen: (u32, u32)) {
        use std::sync::atomic::{AtomicU8, Ordering};
        static HELD: AtomicU8 = AtomicU8::new(255);
        if screen.0 == 0 || screen.1 == 0 {
            return;
        }
        match event {
            InputEvent::MouseMove { x, y } => {
                let held = HELD.load(Ordering::Relaxed);
                let mouse_type = if held == 255 {
                    K_CG_EVENT_MOUSE_MOVED
                } else {
                    mouse_drag_type(held)
                };
                let evt = unsafe {
                    CGEventCreateMouseEvent(
                        ptr::null_mut(),
                        mouse_type,
                        point(screen, *x, *y),
                        if held == 2 { 1 } else { 0 },
                    )
                };
                post(evt);
            }
            InputEvent::MouseDown { button, x, y } | InputEvent::MouseUp { button, x, y } => {
                let down = matches!(event, InputEvent::MouseDown { .. });
                HELD.store(if down { *button } else { 255 }, Ordering::Relaxed);
                let evt = unsafe {
                    CGEventCreateMouseEvent(
                        ptr::null_mut(),
                        mouse_type(down, *button),
                        point(screen, *x, *y),
                        if *button == 2 { 1 } else { 0 },
                    )
                };
                post(evt);
            }
            InputEvent::Wheel { dy, .. } => {
                let delta = (*dy * 3.0).round() as i32;
                if delta == 0 {
                    return;
                }
                let evt = unsafe {
                    CGEventCreateScrollWheelEvent(
                        ptr::null_mut(),
                        K_CG_SCROLL_EVENT_UNIT_LINE,
                        1,
                        delta,
                        0,
                        0,
                    )
                };
                post(evt);
            }
            InputEvent::KeyDown { key, modifiers } | InputEvent::KeyUp { key, modifiers } => {
                let Some(code) = keycode(key) else { return };
                let down = matches!(event, InputEvent::KeyDown { .. });
                let evt = unsafe { CGEventCreateKeyboardEvent(ptr::null_mut(), code, down) };
                if !evt.is_null() && *modifiers != 0 {
                    unsafe { CGEventSetFlags(evt, modifier_flags(*modifiers)) };
                }
                post(evt);
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::InputEvent;
    use std::mem::size_of;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP,
        MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN,
        MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
        MOUSEEVENTF_WHEEL, MOUSEINPUT, VIRTUAL_KEY,
    };

    fn abs_point(x: f32, y: f32) -> (i32, i32) {
        (
            (x.clamp(0.0, 1.0) * 65535.0).round() as i32,
            (y.clamp(0.0, 1.0) * 65535.0).round() as i32,
        )
    }

    fn send(inputs: &[INPUT]) {
        unsafe {
            SendInput(inputs.len() as u32, inputs.as_ptr(), size_of::<INPUT>() as i32);
        }
    }

    fn vk(key: &str) -> Option<VIRTUAL_KEY> {
        Some(match key {
            "KeyA" => 0x41,
            "KeyB" => 0x42,
            "KeyC" => 0x43,
            "KeyD" => 0x44,
            "KeyE" => 0x45,
            "KeyF" => 0x46,
            "KeyG" => 0x47,
            "KeyH" => 0x48,
            "KeyI" => 0x49,
            "KeyJ" => 0x4A,
            "KeyK" => 0x4B,
            "KeyL" => 0x4C,
            "KeyM" => 0x4D,
            "KeyN" => 0x4E,
            "KeyO" => 0x4F,
            "KeyP" => 0x50,
            "KeyQ" => 0x51,
            "KeyR" => 0x52,
            "KeyS" => 0x53,
            "KeyT" => 0x54,
            "KeyU" => 0x55,
            "KeyV" => 0x56,
            "KeyW" => 0x57,
            "KeyX" => 0x58,
            "KeyY" => 0x59,
            "KeyZ" => 0x5A,
            "Digit0" => 0x30,
            "Digit1" => 0x31,
            "Digit2" => 0x32,
            "Digit3" => 0x33,
            "Digit4" => 0x34,
            "Digit5" => 0x35,
            "Digit6" => 0x36,
            "Digit7" => 0x37,
            "Digit8" => 0x38,
            "Digit9" => 0x39,
            "Enter" | "NumpadEnter" => 0x0D,
            "Escape" => 0x1B,
            "Backspace" => 0x08,
            "Tab" => 0x09,
            "Space" => 0x20,
            "ArrowLeft" => 0x25,
            "ArrowRight" => 0x27,
            "ArrowDown" => 0x28,
            "ArrowUp" => 0x26,
            "Delete" => 0x2E,
            "Home" => 0x24,
            "End" => 0x23,
            "PageUp" => 0x21,
            "PageDown" => 0x22,
            _ => return None,
        })
    }

    pub fn inject(event: &InputEvent, screen: (u32, u32)) {
        if screen.0 == 0 || screen.1 == 0 {
            return;
        }
        match event {
            InputEvent::MouseMove { x, y } => {
                let (px, py) = abs_point(*x, *y);
                let input = INPUT {
                    r#type: INPUT_MOUSE,
                    Anonymous: INPUT_0 {
                        mi: MOUSEINPUT {
                            dx: px,
                            dy: py,
                            mouseData: 0,
                            dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                };
                send(&[input]);
            }
            InputEvent::MouseDown { button, x, y } | InputEvent::MouseUp { button, x, y } => {
                let (px, py) = abs_point(*x, *y);
                let down = matches!(event, InputEvent::MouseDown { .. });
                let flags = match (*button, down) {
                    (2, true) => MOUSEEVENTF_RIGHTDOWN,
                    (2, false) => MOUSEEVENTF_RIGHTUP,
                    (1, true) => MOUSEEVENTF_MIDDLEDOWN,
                    (1, false) => MOUSEEVENTF_MIDDLEUP,
                    (_, true) => MOUSEEVENTF_LEFTDOWN,
                    (_, false) => MOUSEEVENTF_LEFTUP,
                };
                let input = INPUT {
                    r#type: INPUT_MOUSE,
                    Anonymous: INPUT_0 {
                        mi: MOUSEINPUT {
                            dx: px,
                            dy: py,
                            mouseData: 0,
                            dwFlags: flags | MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                };
                send(&[input]);
            }
            InputEvent::Wheel { dy, .. } => {
                let delta = (*dy * 120.0) as i32;
                if delta == 0 {
                    return;
                }
                let input = INPUT {
                    r#type: INPUT_MOUSE,
                    Anonymous: INPUT_0 {
                        mi: MOUSEINPUT {
                            dx: 0,
                            dy: 0,
                            mouseData: delta as u32,
                            dwFlags: MOUSEEVENTF_WHEEL,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                };
                send(&[input]);
            }
            InputEvent::KeyDown { key, .. } | InputEvent::KeyUp { key, .. } => {
                let Some(vk) = vk(key) else { return };
                let down = matches!(event, InputEvent::KeyDown { .. });
                let input = INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: vk,
                            wScan: 0,
                            dwFlags: if down { 0 } else { KEYEVENTF_KEYUP },
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                };
                send(&[input]);
            }
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    use super::InputEvent;

    pub fn inject(_event: &InputEvent, _screen: (u32, u32)) {}
}

pub fn map_key_for_os(key: &str, target_os: &str) -> String {
    match (key, target_os) {
        ("Meta", "windows") => "Control".into(),
        ("Control", "macos") => "Meta".into(),
        ("Alt", "macos") => "Alt".into(),
        other => other.0.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_event_roundtrip() {
        let event = InputEvent::MouseMove { x: 0.5, y: 0.5 };
        let json = serde_json::to_string(&event).unwrap();
        let back: InputEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, InputEvent::MouseMove { .. }));
    }
}
