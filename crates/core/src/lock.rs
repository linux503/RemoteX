use std::process::Command;

pub fn lock_workstation() {
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("osascript")
            .args([
                "-e",
                r#"tell application "System Events" to keystroke "q" using {control down, command down}"#,
            ])
            .spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("rundll32.exe")
            .arg("user32.dll,LockWorkStation")
            .spawn();
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = Command::new("loginctl").arg("lock-session").spawn();
    }
}
