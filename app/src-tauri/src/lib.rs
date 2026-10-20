mod ai;
mod db;

use std::sync::Mutex;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager};
use tauri::{PhysicalPosition, PhysicalSize};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_notification::NotificationExt;

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub client: reqwest::Client,
    pub shortcut: Mutex<String>,
    pub draft_requests: Mutex<std::collections::HashMap<String, tokio::sync::watch::Sender<bool>>>,
}

fn default_shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "Command+Shift+Space"
    } else {
        "Control+Shift+Space"
    }
}

#[tauri::command]
fn show_panel(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("Task panel unavailable")?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_mascot_visible(app: tauri::AppHandle, visible: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("mascot")
        .ok_or("Mascot window unavailable")?;
    if visible {
        window.show()
    } else {
        window.hide()
    }
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_mascot_size(app: tauri::AppHandle, size: String) -> Result<(), String> {
    let side = match size.as_str() {
        "small" => 170,
        "medium" => 220,
        "large" => 290,
        _ => return Err("Invalid mascot size".into()),
    };
    let window = app
        .get_webview_window("mascot")
        .ok_or("Mascot window unavailable")?;
    window
        .set_size(PhysicalSize::new(side, side + 20))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn set_shortcut(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    shortcut: String,
) -> Result<(), String> {
    if shortcut.len() > 80 || shortcut.trim().is_empty() {
        return Err("Enter a valid shortcut".into());
    }
    let mut current = state.shortcut.lock().map_err(|_| "Shortcut unavailable")?;
    if *current == shortcut {
        return Ok(());
    }
    app.global_shortcut()
        .register(shortcut.as_str())
        .map_err(|e| format!("Shortcut unavailable: {e}"))?;
    if !current.is_empty() {
        let _ = app.global_shortcut().unregister(current.as_str());
    }
    state.db.lock().map_err(|_| "Database unavailable")?.execute("INSERT INTO settings(key,value) VALUES ('shortcut',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [&shortcut]).map_err(|e| e.to_string())?;
    *current = shortcut;
    let _ = app.emit("settings-changed", ());
    Ok(())
}

fn launch_reminders(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(30));
            let Some(state) = app.try_state::<AppState>() else {
                continue;
            };
            let due = {
                let Ok(conn) = state.db.lock() else { continue };
                if db::setting(&conn, "quiet").ok().flatten().as_deref() == Some("true") {
                    continue;
                }
                db::due_reminders(&conn).unwrap_or_default()
            };
            if due.is_empty() {
                continue;
            }
            let _ = app.emit("tasks-changed", ());
            let _ = app.emit("mascot-state", "reminder");
            let body = if due.len() == 1 {
                due[0].1.clone()
            } else {
                format!("{} tasks need your attention", due.len())
            };
            let delivered = app
                .notification()
                .builder()
                .title("Moonie reminder")
                .body(&body)
                .show();
            if delivered.is_ok()
                && let Ok(conn) = state.db.lock()
            {
                let _ = db::mark_reminders_delivered(&conn, &due);
            }
        }
    });
}

fn restore_mascot(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("mascot") else {
        return;
    };
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let Ok(conn) = state.db.lock() else { return };
    if let Ok(Some(size)) = db::setting(&conn, "mascotSize") {
        let _ = set_mascot_size(app.clone(), size);
    }
    let position = db::setting(&conn, "mascotX")
        .ok()
        .flatten()
        .and_then(|x| x.parse::<i32>().ok())
        .zip(
            db::setting(&conn, "mascotY")
                .ok()
                .flatten()
                .and_then(|y| y.parse::<i32>().ok()),
        );
    let Ok(monitors) = window.available_monitors() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let saved = position.unwrap_or_else(|| {
        let monitor = window
            .current_monitor()
            .ok()
            .flatten()
            .or_else(|| monitors.first().cloned());
        monitor
            .map(|m| {
                (
                    m.position().x + m.size().width as i32 - size.width as i32 - 34,
                    m.position().y + m.size().height as i32 - size.height as i32 - 70,
                )
            })
            .unwrap_or((100, 100))
    });
    let monitor = monitors
        .iter()
        .find(|m| {
            saved.0 >= m.position().x
                && saved.0 < m.position().x + m.size().width as i32
                && saved.1 >= m.position().y
                && saved.1 < m.position().y + m.size().height as i32
        })
        .or_else(|| monitors.first());
    if let Some(m) = monitor {
        let x = saved.0.clamp(
            m.position().x,
            m.position().x + (m.size().width as i32 - size.width as i32).max(0),
        );
        let y = saved.1.clamp(
            m.position().y,
            m.position().y + (m.size().height as i32 - size.height as i32).max(0),
        );
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(|app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed { let _ = show_panel(app.clone()); }
        }).build())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let path = app.path().app_data_dir()?.join("moonie.sqlite3");
            std::fs::create_dir_all(path.parent().unwrap())?;
            let conn = rusqlite::Connection::open(path)?;
            db::migrate(&conn)?;
            let requested = db::setting(&conn, "shortcut")?.unwrap_or_else(|| default_shortcut().into());
            let active = if app.global_shortcut().register(requested.as_str()).is_ok() { requested }
                else if app.global_shortcut().register(default_shortcut()).is_ok() { default_shortcut().into() }
                else { String::new() };
            app.manage(AppState {
                db: Mutex::new(conn),
                client: reqwest::Client::builder().timeout(std::time::Duration::from_secs(60)).build()?,
                shortcut: Mutex::new(active),
                draft_requests: Mutex::new(std::collections::HashMap::new()),
            });
            let open = MenuItemBuilder::with_id("open", "Open Moonie").build(app)?;
            let toggle = MenuItemBuilder::with_id("toggle", "Show or hide mascot").build(app)?;
            let quiet = MenuItemBuilder::with_id("quiet", "Toggle quiet mode").build(app)?;
            let exit = MenuItemBuilder::with_id("quit", "Quit Moonie").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&open, &toggle, &quiet, &exit]).build()?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => { let _ = show_panel(app.clone()); }
                    "toggle" => {
                        if let Some(w) = app.get_webview_window("mascot") {
                            let _ = if w.is_visible().unwrap_or(false) { w.hide() } else { w.show() };
                        }
                    }
                    "quiet" => {
                        if let Some(state) = app.try_state::<AppState>()
                            && let Ok(conn) = state.db.lock() {
                                let current = db::setting(&conn, "quiet").ok().flatten().as_deref() == Some("true");
                                let _ = conn.execute("INSERT INTO settings(key,value) VALUES ('quiet',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [if current { "false" } else { "true" }]);
                                let _ = app.emit("settings-changed", ());
                                let _ = app.emit("mascot-state", if current { "idle" } else { "resting" });
                            }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            restore_mascot(app.handle());
            launch_reminders(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "mascot"
                && let tauri::WindowEvent::Moved(position) = event
                    && let Some(state) = window.app_handle().try_state::<AppState>()
                        && let Ok(conn) = state.db.lock() {
                            let _ = conn.execute("INSERT INTO settings(key,value) VALUES ('mascotX',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [position.x.to_string()]);
                            let _ = conn.execute("INSERT INTO settings(key,value) VALUES ('mascotY',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [position.y.to_string()]);
                        }
            if window.label() == "main"
                && let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
        })
        .invoke_handler(tauri::generate_handler![
            show_panel, set_mascot_visible, set_mascot_size, set_shortcut, quit,
            db::list_tasks, db::save_manual_task, db::approve_drafts, db::update_task,
            db::delete_task, db::next_task, db::get_settings, db::set_setting,
            db::get_focus, db::focus_action,
            ai::set_api_key, ai::has_api_key, ai::get_models, ai::create_draft, ai::cancel_draft,
        ])
        .build(tauri::generate_context!())
        .expect("error building Moonie");
    app.run(|_, _| {});
}
