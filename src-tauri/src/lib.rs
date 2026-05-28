mod app_state;
mod config;

use app_state::AppSnapshot;
use config::UserSettings;
use serde::Deserialize;
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::{AppHandle, Manager, State};

struct RuntimeState {
    settings: Mutex<UserSettings>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelInstallRequest {
    id: String,
    name: String,
    download_url: String,
    archive_root: Option<String>,
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(&config_dir).map_err(|error| error.to_string())?;
    Ok(config_dir.join("settings.json"))
}

fn read_settings(app: &AppHandle) -> Result<UserSettings, String> {
    let path = settings_path(app)?;
    if !path.exists() {
        return Ok(UserSettings::default());
    }

    let raw = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&raw).map_err(|error| error.to_string())
}

fn write_settings(app: &AppHandle, settings: &UserSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    let raw = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    fs::write(path, raw).map_err(|error| error.to_string())
}

fn extract_zip(zip_path: &Path, destination: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let Some(enclosed_name) = entry.enclosed_name() else {
            continue;
        };
        let out_path = destination.join(enclosed_name);

        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(|error| error.to_string())?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }

        let mut out_file = fs::File::create(&out_path).map_err(|error| error.to_string())?;
        io::copy(&mut entry, &mut out_file).map_err(|error| error.to_string())?;
    }

    Ok(())
}

fn install_zip_model(app: AppHandle, model: ModelInstallRequest) -> Result<String, String> {
    let data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?;
    let models_dir = data_dir.join("models");
    let downloads_dir = data_dir.join("downloads");
    let install_dir = models_dir.join(&model.id);

    if install_dir.exists() {
        return Ok(install_dir.to_string_lossy().to_string());
    }

    fs::create_dir_all(&models_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&downloads_dir).map_err(|error| error.to_string())?;

    let archive_path = downloads_dir.join(format!("{}.zip", model.id));
    let mut response =
        reqwest::blocking::get(&model.download_url).map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("下载 {} 失败：{}", model.name, response.status()));
    }

    let mut archive_file = fs::File::create(&archive_path).map_err(|error| error.to_string())?;
    io::copy(&mut response, &mut archive_file).map_err(|error| error.to_string())?;

    let temp_dir = models_dir.join(format!("{}.tmp", model.id));
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&temp_dir).map_err(|error| error.to_string())?;
    extract_zip(&archive_path, &temp_dir)?;

    let extracted_root = model
        .archive_root
        .as_ref()
        .map(|root| temp_dir.join(root))
        .filter(|path| path.exists())
        .unwrap_or_else(|| temp_dir.clone());

    if install_dir.exists() {
        fs::remove_dir_all(&install_dir).map_err(|error| error.to_string())?;
    }
    fs::rename(&extracted_root, &install_dir).map_err(|error| error.to_string())?;

    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).map_err(|error| error.to_string())?;
    }

    Ok(install_dir.to_string_lossy().to_string())
}

#[tauri::command]
fn get_app_snapshot(state: State<'_, RuntimeState>) -> AppSnapshot {
    let settings = state.settings.lock().expect("settings mutex poisoned");
    AppSnapshot {
        shortcut: settings.shortcut.clone(),
        ..AppSnapshot::default()
    }
}

#[tauri::command]
fn load_settings(app: AppHandle, state: State<'_, RuntimeState>) -> Result<UserSettings, String> {
    let settings = read_settings(&app)?;
    let mut stored = state.settings.lock().expect("settings mutex poisoned");
    *stored = settings.clone();
    Ok(settings)
}

#[tauri::command]
fn save_settings(
    settings: UserSettings,
    app: AppHandle,
    state: State<'_, RuntimeState>,
) -> Result<UserSettings, String> {
    write_settings(&app, &settings)?;
    let mut stored = state.settings.lock().expect("settings mutex poisoned");
    *stored = settings.clone();
    Ok(settings)
}

#[tauri::command]
async fn install_model(app: AppHandle, model: ModelInstallRequest) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || install_zip_model(app, model))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn select_directory() -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        Ok(rfd::FileDialog::new()
            .pick_folder()
            .map(|path| path.to_string_lossy().to_string()))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(RuntimeState {
            settings: Mutex::new(UserSettings::default()),
        })
        .invoke_handler(tauri::generate_handler![
            get_app_snapshot,
            load_settings,
            save_settings,
            install_model,
            select_directory
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
