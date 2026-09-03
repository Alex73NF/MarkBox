mod settings;

use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct AppState {
    pub settings: Mutex<settings::Settings>,
}

pub fn settings_path(app: &AppHandle) -> std::path::PathBuf {
    app.path().app_config_dir().unwrap().join("settings.json")
}

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> settings::Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(app: AppHandle, state: tauri::State<AppState>, settings: settings::Settings) -> Result<settings::Settings, String> {
    let normalized = settings::normalize(&settings);
    settings::save_to(&settings_path(&app), &normalized).map_err(|e| e.to_string())?;
    *state.settings.lock().unwrap() = normalized.clone();
    let _ = app.emit_to("mark", "settings-updated", &normalized);
    Ok(normalized)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .setup(|app| {
            let loaded = settings::load_from(&settings_path(app.handle()));
            app.manage(AppState { settings: Mutex::new(loaded) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_settings, save_settings])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
