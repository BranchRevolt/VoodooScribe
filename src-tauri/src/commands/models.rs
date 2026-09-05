// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::AppError;
use crate::models::{downloader, registry, ModelInfo, ModelKind};
use crate::state::AppState;
use crate::vram;

#[derive(serde::Serialize)]
pub struct ModelsStatus {
    pub whisper_loaded: bool,
    pub whisper_path: Option<String>,
    pub llm_loaded: bool,
    pub llm_path: Option<String>,
    pub installed_whisper: Vec<String>,
    pub installed_llm: Vec<String>,
    pub available_vram_mb: u32,
    pub recommended_whisper: String,
}

/// Emitted when a download finishes (success, cancel, or error).
#[derive(Clone, serde::Serialize)]
pub struct DownloadDoneEvent {
    pub filename: String,
    pub cancelled: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn cmd_models_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ModelsStatus, AppError> {
    let models_dir = downloader::models_dir(&app)?;
    {
        let mut wpath = state.whisper_model_path.lock();
        if wpath.is_none() {
            for m in registry::all_whisper_models() {
                let p = models_dir.join(&m.filename);
                if p.exists() {
                    *wpath = Some(p);
                    break;
                }
            }
        }
    }
    {
        let mut lpath = state.llm_model_path.lock();
        if lpath.is_none() {
            for m in registry::all_llm_models() {
                let p = models_dir.join(&m.filename);
                if p.exists() {
                    *lpath = Some(p);
                    break;
                }
            }
        }
    }

    let installed_whisper: Vec<String> = registry::all_whisper_models()
        .into_iter()
        .filter(|m| models_dir.join(&m.filename).exists())
        .map(|m| m.filename)
        .collect();

    let installed_llm: Vec<String> = registry::all_llm_models()
        .into_iter()
        .filter(|m| models_dir.join(&m.filename).exists())
        .map(|m| m.filename)
        .collect();

    let available_vram_mb = vram::total_vram_mb();
    let recommended_whisper = vram::recommend_for_vram(available_vram_mb).to_string();

    // Each mutex is locked exactly once into a local. Locking the same parking_lot
    // mutex twice inside one struct literal deadlocks: the temporary guards live
    // until the end of the expression, so the second .lock() waits on a guard that
    // has not been dropped.
    let whisper_path = state
        .whisper_model_path
        .lock()
        .as_ref()
        .map(|p| p.display().to_string());
    let llm_path = state
        .llm_model_path
        .lock()
        .as_ref()
        .map(|p| p.display().to_string());

    Ok(ModelsStatus {
        whisper_loaded: whisper_path.is_some(),
        whisper_path,
        llm_loaded: llm_path.is_some(),
        llm_path,
        installed_whisper,
        installed_llm,
        available_vram_mb,
        recommended_whisper,
    })
}

#[tauri::command]
pub fn cmd_list_whisper_models() -> Vec<ModelInfo> {
    registry::all_whisper_models()
}

#[tauri::command]
pub fn cmd_list_llm_models() -> Vec<ModelInfo> {
    registry::all_llm_models()
}

/// State of a single model file on disk. `installed` = the final file exists;
/// `partial_bytes` = size of a leftover .tmp (0 = none).
#[derive(serde::Serialize)]
pub struct DownloadFileStatus {
    pub installed: bool,
    pub partial_bytes: u64,
}

#[tauri::command]
pub async fn cmd_download_status(
    app: AppHandle,
    filename: String,
) -> Result<DownloadFileStatus, AppError> {
    let dir = downloader::models_dir(&app)?;
    let dest = dir.join(&filename);
    let tmp = dest.with_extension("tmp");

    let installed = dest.exists();
    let partial_bytes = if tmp.exists() {
        tokio::fs::metadata(&tmp).await.map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    Ok(DownloadFileStatus { installed, partial_bytes })
}

/// Starts a download in the background and returns immediately.
/// Completion (success / cancel / error) is signalled via the
/// `model://download-done` event so the JS invoke never hangs.
#[tauri::command]
pub async fn cmd_download_model(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: ModelKind,
) -> Result<(), AppError> {
    let models_dir = downloader::models_dir(&app)?;

    let info = registry::find(&kind).ok_or_else(|| AppError::Other("Unknown model kind".into()))?;

    // Reset the cancel flag before spawning.
    state.download_cancel.store(false, Ordering::Relaxed);
    let cancel = state.download_cancel.clone();

    // Clone what the task needs; AppHandle is Clone + Send + 'static.
    let url      = info.url.clone();
    let filename = info.filename.clone();
    let sha256   = info.sha256.clone();

    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();

        let result = downloader::download_model(
            app.clone(),
            &url,
            &models_dir,
            &filename,
            sha256.as_deref(),
            cancel,
        )
        .await;

        let event = match result {
            Ok(dest) => {
                match kind {
                    ModelKind::Whisper(_) => *state.whisper_model_path.lock() = Some(dest),
                    ModelKind::Llama(_) => *state.llm_model_path.lock() = Some(dest),
                }
                DownloadDoneEvent { filename: filename.clone(), cancelled: false, error: None }
            }
            Err(AppError::Cancelled) => {
                DownloadDoneEvent { filename: filename.clone(), cancelled: true, error: None }
            }
            Err(e) => {
                DownloadDoneEvent { filename: filename.clone(), cancelled: false, error: Some(e.to_string()) }
            }
        };

        tracing::info!("[dl] {filename}: emitting download-done cancelled={} error={:?}", event.cancelled, event.error);
        let _ = app.emit("model://download-done", event);
    });

    Ok(())
}

#[tauri::command]
pub fn cmd_cancel_download(state: State<'_, AppState>) -> Result<(), AppError> {
    state.download_cancel.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub async fn cmd_delete_model(
    app: AppHandle,
    state: State<'_, AppState>,
    filename: String,
) -> Result<(), AppError> {
    let path = downloader::models_dir(&app)?.join(&filename);
    if path.exists() {
        tokio::fs::remove_file(&path).await?;
    }
    let mut wpath = state.whisper_model_path.lock();
    if wpath.as_ref().and_then(|p| p.file_name()) == Some(std::ffi::OsStr::new(&filename)) {
        *wpath = None;
    }
    drop(wpath);
    let mut lpath = state.llm_model_path.lock();
    if lpath.as_ref().and_then(|p| p.file_name()) == Some(std::ffi::OsStr::new(&filename)) {
        *lpath = None;
    }
    Ok(())
}

#[tauri::command]
pub async fn cmd_cancel_and_delete(
    app: AppHandle,
    state: State<'_, AppState>,
    filename: String,
) -> Result<(), AppError> {
    state.download_cancel.store(true, Ordering::Relaxed);
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    let tmp = downloader::models_dir(&app)?.join(&filename).with_extension("tmp");
    if tmp.exists() {
        tokio::fs::remove_file(&tmp).await.ok();
    }
    Ok(())
}

/// Returns the directory models are currently downloaded to / loaded from.
#[tauri::command]
pub async fn cmd_get_models_dir(app: AppHandle) -> Result<String, AppError> {
    Ok(downloader::models_dir(&app)?.display().to_string())
}

/// Changes the models directory (creates it if missing) and re-scans it.
/// Existing files in the old location are left untouched; the loaded model paths
/// are reset and re-discovered from the new directory.
#[tauri::command]
pub async fn cmd_set_models_dir(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<String, AppError> {
    downloader::set_models_dir(&app, &path)?;
    *state.whisper_model_path.lock() = None;
    *state.llm_model_path.lock() = None;
    state.auto_discover(&app);
    Ok(downloader::models_dir(&app)?.display().to_string())
}

/// Selects which installed model (whisper or LLM) the next run uses. Resolves the
/// file from the registry and the models dir, and errors if it is not installed.
#[tauri::command]
pub async fn cmd_select_model(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: ModelKind,
) -> Result<(), AppError> {
    let models_dir = downloader::models_dir(&app)?;
    let filename = registry::find(&kind)
        .map(|m| m.filename)
        .ok_or_else(|| AppError::Other("unknown model".into()))?;
    let path = models_dir.join(&filename);
    if !path.exists() {
        return Err(AppError::FileNotFound(filename));
    }
    match kind {
        ModelKind::Whisper(_) => *state.whisper_model_path.lock() = Some(path),
        ModelKind::Llama(_) => *state.llm_model_path.lock() = Some(path),
    }
    Ok(())
}

#[tauri::command]
pub async fn cmd_set_model_path(
    state: State<'_, AppState>,
    kind: ModelKind,
    path: String,
) -> Result<(), AppError> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(AppError::FileNotFound(path));
    }
    match kind {
        ModelKind::Whisper(_) => *state.whisper_model_path.lock() = Some(p),
        ModelKind::Llama(_) => *state.llm_model_path.lock() = Some(p),
    }
    Ok(())
}
