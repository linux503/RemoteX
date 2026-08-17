use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermState {
    Granted,
    Denied,
    Unknown,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PermissionsSnapshot {
    pub screen_recording: PermState,
    pub accessibility: PermState,
    pub input_monitoring: PermState,
    pub platform: String,
    pub all_granted: bool,
}

impl PermissionsSnapshot {
    pub fn check() -> Self {
        #[cfg(target_os = "macos")]
        {
            return macos::check();
        }
        #[cfg(target_os = "windows")]
        {
            return Self {
                screen_recording: PermState::Unknown,
                accessibility: PermState::Unknown,
                input_monitoring: PermState::Unknown,
                platform: "windows".into(),
                all_granted: true,
            };
        }
        #[allow(unreachable_code)]
        Self {
            screen_recording: PermState::Unknown,
            accessibility: PermState::Unknown,
            input_monitoring: PermState::Unknown,
            platform: "other".into(),
            all_granted: false,
        }
    }

    pub fn open_panel(kind: &str) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            return macos::open_panel(kind);
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = kind;
            Ok(())
        }
    }

    pub fn request_screen_recording() -> bool {
        #[cfg(target_os = "macos")]
        {
            return macos::request_screen_recording();
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{PermState, PermissionsSnapshot};
    use std::process::Command;

    mod ffi {
        use std::os::raw::c_int;

        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            pub fn CGPreflightScreenCaptureAccess() -> bool;
            pub fn CGRequestScreenCaptureAccess() -> bool;
        }

        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            pub fn AXIsProcessTrusted() -> bool;
        }

        #[link(name = "IOKit", kind = "framework")]
        extern "C" {
            pub fn IOHIDCheckAccess(request: c_int) -> c_int;
            pub fn IOHIDRequestAccess(request: c_int) -> bool;
        }

        pub const K_IOHID_REQUEST_TYPE_LISTEN_EVENT: c_int = 1;
        pub const K_IOHID_ACCESS_TYPE_GRANTED: c_int = 0;
        pub const K_IOHID_ACCESS_TYPE_DENIED: c_int = 1;
        pub const K_IOHID_ACCESS_TYPE_UNKNOWN: c_int = 2;
    }

    pub fn check() -> PermissionsSnapshot {
        let screen_recording = if unsafe { ffi::CGPreflightScreenCaptureAccess() } {
            PermState::Granted
        } else {
            PermState::Denied
        };

        let accessibility = if unsafe { ffi::AXIsProcessTrusted() } {
            PermState::Granted
        } else {
            PermState::Denied
        };

        let input_monitoring = match unsafe {
            ffi::IOHIDCheckAccess(ffi::K_IOHID_REQUEST_TYPE_LISTEN_EVENT)
        } {
            ffi::K_IOHID_ACCESS_TYPE_GRANTED => PermState::Granted,
            ffi::K_IOHID_ACCESS_TYPE_DENIED => PermState::Denied,
            _ => PermState::Unknown,
        };

        let all_granted = screen_recording == PermState::Granted
            && accessibility == PermState::Granted
            && input_monitoring == PermState::Granted;

        PermissionsSnapshot {
            screen_recording,
            accessibility,
            input_monitoring,
            platform: "macos".into(),
            all_granted,
        }
    }

    pub fn request_screen_recording() -> bool {
        unsafe { ffi::CGRequestScreenCaptureAccess() }
    }

    pub fn open_panel(kind: &str) -> Result<(), String> {
        let urls: &[&str] = match kind {
            "screen_recording" => &[
                "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_ScreenCapture",
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
            ],
            "accessibility" => &[
                "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_Accessibility",
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
            ],
            "input_monitoring" => &[
                "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_ListenEvent",
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
            ],
            _ => return Err(format!("unknown permission kind: {kind}")),
        };
        for url in urls {
            if Command::new("open").arg(url).spawn().is_ok() {
                return Ok(());
            }
        }
        Err("failed to open System Settings".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_serializes() {
        let snap = PermissionsSnapshot {
            screen_recording: PermState::Denied,
            accessibility: PermState::Granted,
            input_monitoring: PermState::Unknown,
            platform: "macos".into(),
            all_granted: false,
        };
        let json = serde_json::to_string(&snap).unwrap();
        assert!(json.contains("screen_recording"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_check_runs() {
        let snap = PermissionsSnapshot::check();
        assert_eq!(snap.platform, "macos");
        assert!(matches!(
            snap.screen_recording,
            PermState::Granted | PermState::Denied | PermState::Unknown
        ));
    }
}
