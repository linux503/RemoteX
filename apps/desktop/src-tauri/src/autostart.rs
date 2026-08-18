use std::fs;

pub fn apply(enabled: bool) {
    if let Err(err) = apply_inner(enabled) {
        tracing::warn!("autostart: {err}");
    }
}

fn apply_inner(enabled: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").map_err(|_| "no HOME")?;
        let plist = std::path::PathBuf::from(home).join("Library/LaunchAgents/com.remotex.app.plist");
        if enabled {
            if let Some(parent) = plist.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let body = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.remotex.app</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
</dict>
</plist>
"#,
                exe.display()
            );
            fs::write(&plist, body).map_err(|e| e.to_string())?;
            let path = plist.to_string_lossy().to_string();
            let _ = std::process::Command::new("launchctl")
                .args(["unload", &path])
                .status();
            let _ = std::process::Command::new("launchctl")
                .args(["load", &path])
                .status();
        } else if plist.exists() {
            let path = plist.to_string_lossy().to_string();
            let _ = std::process::Command::new("launchctl")
                .args(["unload", &path])
                .status();
            let _ = fs::remove_file(plist);
        }
    }
    #[cfg(target_os = "windows")]
    {
        let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        if enabled {
            let value = format!("\"{}\"", exe.display());
            let status = std::process::Command::new("reg")
                .args(["add", key, "/v", "RemoteX", "/t", "REG_SZ", "/d", &value, "/f"])
                .status()
                .map_err(|e| e.to_string())?;
            if !status.success() {
                return Err("reg add failed".into());
            }
        } else {
            let _ = std::process::Command::new("reg")
                .args(["delete", key, "/v", "RemoteX", "/f"])
                .status();
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (enabled, exe);
    }
    Ok(())
}
