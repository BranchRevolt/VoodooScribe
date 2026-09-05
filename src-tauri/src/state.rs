// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::AppHandle;

use crate::models::{downloader, registry, ModelKind, WhisperSize};
use crate::transcribe::WhisperContext;
use llama_cpp_2::model::LlamaModel;

/// Cached whisper model: the path it was loaded from plus the loaded context.
/// Loading a big model into VRAM takes ~9 s, so the context is reused across
/// transcriptions and only a model change triggers a reload.
pub type WhisperCache = Arc<Mutex<Option<(PathBuf, Arc<WhisperContext>)>>>;

/// Cached LLM: the path it was loaded from plus the loaded model. Loading
/// Qwen3-4B costs ~3–5 s, which would otherwise be paid on every
/// summarize/polish.
pub type LlmCache = Arc<Mutex<Option<(PathBuf, Arc<LlamaModel>)>>>;

/// Held for the duration of an LLM operation; clears the busy flag on drop, so an
/// early return or a panic can't leave the app stuck as busy.
pub struct LlmBusyGuard(Arc<AtomicBool>);

/// Which of the two GPU-resident models a call is about.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Model {
    Whisper,
    Llm,
}

impl Drop for LlmBusyGuard {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::Release);
    }
}

pub struct AppState {
    pub whisper_model_path: Arc<Mutex<Option<PathBuf>>>,
    pub llm_model_path: Arc<Mutex<Option<PathBuf>>>,
    /// Loaded whisper context cache (see WhisperCache).
    pub whisper_cache: WhisperCache,
    /// Loaded LLM cache (see LlmCache).
    pub llm_cache: LlmCache,
    /// Set while a summarize/polish runs. The llama backend and the cached model
    /// are process-wide, so concurrent LLM operations would fight over them.
    llm_busy: Arc<AtomicBool>,
    /// Cancels the running transcription.
    pub cancel_flag: Arc<AtomicBool>,
    /// Cancels the running summarize / polish (LLM) operation.
    pub summarize_cancel: Arc<AtomicBool>,
    /// Cancels the running model download.
    pub download_cancel: Arc<AtomicBool>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            whisper_model_path: Arc::new(Mutex::new(None)),
            llm_model_path: Arc::new(Mutex::new(None)),
            whisper_cache: Arc::new(Mutex::new(None)),
            llm_cache: Arc::new(Mutex::new(None)),
            llm_busy: Arc::new(AtomicBool::new(false)),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            summarize_cancel: Arc::new(AtomicBool::new(false)),
            download_cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Claims the LLM for one operation, or returns None if one is already
    /// running. Callers surface that as `AppError::LlmBusy` rather than queueing.
    pub fn try_claim_llm(&self) -> Option<LlmBusyGuard> {
        self.llm_busy
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            )
            .ok()
            .map(|_| LlmBusyGuard(self.llm_busy.clone()))
    }

    /// VRAM the currently-selected whisper model needs, looked up by filename.
    /// Falls back to the largest shipped model when the file isn't recognised.
    fn whisper_vram_mb(&self) -> u32 {
        self.whisper_model_path
            .lock()
            .as_ref()
            .and_then(|p| p.file_name().and_then(|f| f.to_str()).map(str::to_owned))
            .and_then(|name| {
                registry::all_whisper_models()
                    .into_iter()
                    .find(|m| m.filename == name)
            })
            .map(|m| m.vram_required_mb)
            .unwrap_or(1_600)
    }

    /// VRAM the currently-selected LLM needs. An unrecognised (user-supplied) GGUF
    /// falls back to the default entry's figure.
    fn llm_vram_mb(&self) -> u32 {
        self.llm_model_path
            .lock()
            .as_ref()
            .and_then(|p| p.file_name().and_then(|f| f.to_str()))
            .and_then(registry::find_by_filename)
            .map(|m| m.vram_required_mb)
            .unwrap_or_else(|| registry::default_llm_model().vram_required_mb)
    }

    /// Drops the other model from VRAM when both won't fit at once. On a roomy
    /// card both stay resident; on a small one they take turns.
    fn evict_unless_both_fit(&self, keep: Model) {
        let llm_mb = self.llm_vram_mb();
        if crate::vram::both_models_fit(self.whisper_vram_mb(), llm_mb) {
            return;
        }
        match keep {
            Model::Llm => *self.whisper_cache.lock() = None,
            Model::Whisper => *self.llm_cache.lock() = None,
        }
    }

    /// Call before loading the LLM: frees whisper if they don't both fit.
    pub fn make_room_for_llm(&self) {
        self.evict_unless_both_fit(Model::Llm);
    }

    /// Call before loading whisper: frees the LLM if they don't both fit.
    pub fn make_room_for_whisper(&self) {
        self.evict_unless_both_fit(Model::Whisper);
    }

    /// Called once at startup: deletes leftover .tmp files, then auto-loads the
    /// finished models.
    pub fn auto_discover(&self, app: &AppHandle) {
        let Ok(models_dir) = downloader::models_dir(app) else {
            return;
        };

        // Partial downloads from previous sessions would confuse the downloader.
        if let Ok(entries) = std::fs::read_dir(&models_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("tmp") {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }

        // Prefer Large v3 Turbo, fall back to whatever is present.
        let mut models = registry::all_whisper_models();
        models.sort_by_key(|m| {
            if matches!(&m.kind, ModelKind::Whisper(WhisperSize::LargeV3Turbo)) {
                0i32
            } else {
                1
            }
        });

        for m in models {
            let path = models_dir.join(&m.filename);
            if path.exists() {
                *self.whisper_model_path.lock() = Some(path);
                break;
            }
        }

        // The biggest installed LLM the card can hold; if none fits, the smallest
        // installed one, leaving the OOM error to report the problem.
        let vram = crate::vram::total_vram_mb();
        let installed: Vec<_> = registry::all_llm_models()
            .into_iter()
            .filter(|m| models_dir.join(&m.filename).exists())
            .collect();
        let chosen = installed
            .iter()
            .filter(|m| vram == 0 || m.vram_required_mb <= vram)
            .max_by_key(|m| m.vram_required_mb)
            .or_else(|| installed.first());
        if let Some(m) = chosen {
            *self.llm_model_path.lock() = Some(models_dir.join(&m.filename));
        }
    }
}
