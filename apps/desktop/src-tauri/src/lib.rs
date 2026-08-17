use capture::{list_displays, DisplayInfo};
use input::InputEvent;
use remotex_core::{AppEvent, AppSettings, AppState};
use serde::Serialize;
use std::sync::Arc;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State,
};
use tokio::sync::Mutex;

pub type SharedState = Arc<Mutex<AppState>>;

#[derive(Serialize)]
struct CommandError {
    message: String,
}

impl From<remotex_core::Error> for CommandError {
    fn from(value: remotex_core::Error) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

#[tauri::command]
async fn snapshot(state: State<'_, SharedState>) -> Result<remotex_core::Snapshot, CommandError> {
    Ok(state.lock().await.snapshot_async().await)
}

#[tauri::command]
async fn refresh_password(app: AppHandle, state: State<'_, SharedState>) -> Result<remotex_core::Snapshot, CommandError> {
    let mut guard = state.lock().await;
    guard.refresh_password()?;
    emit_snapshot(&app, &guard).await;
    Ok(guard.snapshot_async().await)
}

#[tauri::command]
async fn set_temp_password(
    app: AppHandle,
    state: State<'_, SharedState>,
    password: String,
) -> Result<remotex_core::Snapshot, CommandError> {
    let mut guard = state.lock().await;
    guard.set_temp_password(&password)?;
    emit_snapshot(&app, &guard).await;
    Ok(guard.snapshot_async().await)
}

#[tauri::command]
async fn connect(
    app: AppHandle,
    state: State<'_, SharedState>,
    target_id: String,
    password: String,
) -> Result<remotex_core::Snapshot, CommandError> {
    let mut guard = state.lock().await;
    guard.connect(target_id, password).await?;
    emit_snapshot(&app, &guard).await;
    Ok(guard.snapshot_async().await)
}

#[tauri::command]
async fn accept(app: AppHandle, state: State<'_, SharedState>) -> Result<(), CommandError> {
    let mut guard = state.lock().await;
    guard.accept().await;
    emit_snapshot(&app, &guard).await;
    Ok(())
}

#[tauri::command]
async fn decline(app: AppHandle, state: State<'_, SharedState>) -> Result<(), CommandError> {
    let mut guard = state.lock().await;
    guard.decline().await;
    emit_snapshot(&app, &guard).await;
    Ok(())
}

#[tauri::command]
async fn hangup(app: AppHandle, state: State<'_, SharedState>) -> Result<(), CommandError> {
    let mut guard = state.lock().await;
    guard.hangup().await;
    emit_snapshot(&app, &guard).await;
    Ok(())
}

#[tauri::command]
async fn save_settings(
    app: AppHandle,
    state: State<'_, SharedState>,
    settings: AppSettings,
) -> Result<(), CommandError> {
    let mut guard = state.lock().await;
    guard.update_settings(settings)?;
    emit_snapshot(&app, &guard).await;
    Ok(())
}

#[tauri::command]
async fn set_permanent_password(
    app: AppHandle,
    state: State<'_, SharedState>,
    password: String,
) -> Result<(), CommandError> {
    let mut guard = state.lock().await;
    guard.set_permanent_password(&password)?;
    emit_snapshot(&app, &guard).await;
    Ok(())
}

#[tauri::command]
async fn toggle_favorite(
    app: AppHandle,
    state: State<'_, SharedState>,
    id: String,
) -> Result<(), CommandError> {
    let mut guard = state.lock().await;
    guard.toggle_favorite(&id)?;
    emit_snapshot(&app, &guard).await;
    Ok(())
}

#[tauri::command]
fn displays() -> Vec<DisplayInfo> {
    list_displays().unwrap_or_default()
}

#[tauri::command]
async fn latest_frame(state: State<'_, SharedState>) -> Result<Option<remotex_core::RemoteFrame>, CommandError> {
    Ok(state.lock().await.latest_frame())
}

#[tauri::command]
async fn session_input(
    state: State<'_, SharedState>,
    event: InputEvent,
) -> Result<(), CommandError> {
    state.lock().await.send_input(event).await?;
    Ok(())
}

#[tauri::command]
async fn set_session_quality(
    app: AppHandle,
    state: State<'_, SharedState>,
    quality: String,
) -> Result<remotex_core::Snapshot, CommandError> {
    let mut guard = state.lock().await;
    guard.set_session_quality(quality).await?;
    emit_snapshot(&app, &guard).await;
    Ok(guard.snapshot_async().await)
}

#[tauri::command]
fn permissions_status() -> remotex_permissions::PermissionsSnapshot {
    remotex_permissions::PermissionsSnapshot::check()
}

#[tauri::command]
fn open_permission_panel(kind: String) -> Result<(), CommandError> {
    remotex_permissions::PermissionsSnapshot::open_panel(&kind).map_err(CommandError::from)
}

#[tauri::command]
fn request_screen_recording() -> bool {
    remotex_permissions::PermissionsSnapshot::request_screen_recording()
}

#[tauri::command]
fn open_permission_settings() -> Result<(), CommandError> {
    open_permission_panel("screen_recording".into())
}

async fn emit_snapshot(app: &AppHandle, state: &AppState) {
    let _ = app.emit("snapshot", state.snapshot_async().await);
}

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "remotex=info,remotex_core=info".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async {
                let addr = std::net::SocketAddr::from(([0, 0, 0, 0], protocol::DEFAULT_SIGNALING_PORT));
                match signaling::serve(addr).await {
                    Ok(()) => {}
                    Err(err) => tracing::info!("using existing signaling: {err}"),
                }
            });
            std::thread::sleep(std::time::Duration::from_millis(120));
            if signaling::HOSTING.load(std::sync::atomic::Ordering::SeqCst) {
                remotex_core::HOSTING.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            let (state, mut events) = tauri::async_runtime::block_on(async {
                AppState::bootstrap()
                    .await
                    .expect("failed to start RemoteX")
            });
            app.manage(state.clone());

            {
                let snap = tauri::async_runtime::block_on(async { state.lock().await.snapshot_async().await });
                let _ = handle.emit("snapshot", snap);
            }

            let event_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                while let Some(evt) = events.recv().await {
                    match evt {
                        AppEvent::Snapshot(snap) => {
                            let _ = event_handle.emit("snapshot", snap);
                        }
                        AppEvent::Frame(frame) => {
                            let _ = event_handle.emit("remote-frame", frame);
                        }
                        AppEvent::Toast(msg) => {
                            let _ = event_handle.emit("toast", msg);
                        }
                    }
                }
            });

            let open = MenuItem::with_id(app, "open", "Open RemoteX", true, None::<&str>)?;
            let copy_id = MenuItem::with_id(app, "copy_id", "Copy ID", true, None::<&str>)?;
            let copy_pw = MenuItem::with_id(app, "copy_password", "Copy Password", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(
                app,
                &[&open, &sep, &copy_id, &copy_pw, &sep, &settings, &sep, &quit],
            )?;

            let icon = Image::from_bytes(include_bytes!("../icons/tray.png")).ok();
            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("RemoteX")
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "open" | "settings" => show_main(app),
                    "quit" => app.exit(0),
                    "copy_id" => {
                        if let Some(state) = app.try_state::<SharedState>() {
                            tauri::async_runtime::block_on(async {
                                let snap = state.lock().await.snapshot();
                                let _ = app.emit("copy", snap.formatted_id);
                            });
                        }
                    }
                    "copy_password" => {
                        if let Some(state) = app.try_state::<SharedState>() {
                            tauri::async_runtime::block_on(async {
                                let snap = state.lock().await.snapshot();
                                let _ = app.emit("copy", snap.temp_password);
                            });
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main(tray.app_handle());
                    }
                });
            if let Some(icon) = icon {
                tray = tray.icon(icon);
            }
            tray.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            refresh_password,
            set_temp_password,
            connect,
            accept,
            decline,
            hangup,
            save_settings,
            set_permanent_password,
            toggle_favorite,
            displays,
            permissions_status,
            open_permission_panel,
            request_screen_recording,
            open_permission_settings,
            session_input,
            set_session_quality,
            latest_frame
        ])
        .run(tauri::generate_context!())
        .expect("error while running RemoteX");
}
