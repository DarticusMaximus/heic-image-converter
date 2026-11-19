mod convert;
mod decode;
mod estimate;
mod formats;
mod heic;
mod scan;

#[cfg(test)]
mod backend_integration;
#[cfg(test)]
mod codec_artifacts;
#[cfg(test)]
mod fixture_corpus;
#[cfg(test)]
mod source_review;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
struct ConversionState {
    running: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
}

#[tauri::command]
async fn scan_paths(paths: Vec<String>) -> Result<scan::ScanResult, String> {
    tauri::async_runtime::spawn_blocking(move || scan::scan(&paths))
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn load_thumbnail(path: String, size: Option<u32>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || scan::thumbnail(&path, size.unwrap_or(128)))
        .await
        .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn estimate_size(
    items: Vec<estimate::EstimateItem>,
    settings: formats::ConvertSettings,
) -> Result<estimate::EstimateResult, String> {
    let settings = settings.sanitized();
    tauri::async_runtime::spawn_blocking(move || estimate::estimate(&items, &settings))
        .await
        .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn convert_files(
    app: AppHandle,
    items: Vec<convert::ConvertItem>,
    settings: formats::ConvertSettings,
) -> Result<convert::ConversionOutcome, String> {
    if items.is_empty() {
        return Err("nothing to convert".into());
    }
    let settings = settings.sanitized();

    if let Some(dir) = &settings.output_dir {
        std::fs::create_dir_all(dir)
            .map_err(|err| format!("could not create output folder: {err}"))?;
    }

    let state = app.state::<ConversionState>();
    if state
        .running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("a conversion is already running".into());
    }
    state.cancel.store(false, Ordering::SeqCst);
    let cancel = state.cancel.clone();
    let running = state.running.clone();
    let worker_app = app.clone();

    let joined = tauri::async_runtime::spawn_blocking(move || {
        let outcome = convert::convert_all(&items, &settings, cancel, |event| {
            let _ = worker_app.emit("convert-progress", event);
        });
        running.store(false, Ordering::SeqCst);
        outcome
    })
    .await
    .map_err(|err| {
        // The pool task itself failed; make sure the app is not stuck "busy".
        app.state::<ConversionState>()
            .running
            .store(false, Ordering::SeqCst);
        err.to_string()
    })?;

    Ok(joined)
}

#[tauri::command]
fn cancel_conversion(app: AppHandle) {
    let state = app.state::<ConversionState>();
    if state.running.load(Ordering::SeqCst) {
        state.cancel.store(true, Ordering::SeqCst);
    }
}

/// Open a folder, or reveal a file in the system file manager.
#[tauri::command]
fn reveal_path(app: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let target = Path::new(&path);
    let result = if target.is_dir() {
        app.opener().open_path(&path, None::<&str>)
    } else {
        app.opener().reveal_item_in_dir(target)
    };
    result.map_err(|err| err.to_string())
}

/// Paths handed to the binary on the command line, so "Open with" from a file
/// manager (and `heic-image-converter photo.heic`) pre-fills the queue.
#[tauri::command]
fn startup_paths() -> Vec<String> {
    std::env::args()
        .skip(1)
        .filter(|arg| !arg.starts_with('-') && Path::new(arg).exists())
        .collect()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(ConversionState::default())
        .invoke_handler(tauri::generate_handler![
            scan_paths,
            load_thumbnail,
            estimate_size,
            convert_files,
            cancel_conversion,
            reveal_path,
            startup_paths
        ])
        .run(tauri::generate_context!())
        .expect("error while running the application");
}
