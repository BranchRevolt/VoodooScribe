// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, State};

use std::path::Path;
use std::sync::Arc;

use llama_cpp_2::model::LlamaModel;

use crate::error::{AppError, AppResult};
use crate::state::{AppState, LlmCache};
use crate::transcribe::Segment;
use crate::summarize;

/// Progress for the summarize / polish LLM op, emitted as `summarize://progress`.
/// Only one of these runs at a time, so both commands share the event.
#[derive(Clone, serde::Serialize)]
struct SummarizeProgress {
    percent: u8,
}

/// Builds a throttled progress callback that emits `summarize://progress` to the UI,
/// converting (done, total) steps into a percent and skipping unchanged values.
fn progress_emitter(app: AppHandle) -> impl FnMut(u32, u32) {
    let mut last: i32 = -1;
    move |done: u32, total: u32| {
        let pct = if total > 0 { (done * 100 / total).min(100) as i32 } else { 0 };
        if pct != last {
            last = pct;
            let _ = app.emit("summarize://progress", SummarizeProgress { percent: pct as u8 });
        }
    }
}

// Prompts are embedded at compile time: resource_dir() does not work under
// `tauri dev`, where bundle resources aren't copied to target/debug. Editing a
// prompt therefore needs a rebuild.
const SUMMARIZE_PROMPT: &str = include_str!("../../resources/prompts/summarize.md");
const STRUCTURED_PROMPT: &str = include_str!("../../resources/prompts/structured.md");
const POLISH_PROMPT: &str = include_str!("../../resources/prompts/polish.md");
const POLISH_EDITED_PROMPT: &str = include_str!("../../resources/prompts/polish_edited.md");

/// Returns the cached LLM, loading it if the cache is empty or holds a different
/// model. Loading is the slow part (~3–5 s), so this runs inside spawn_blocking.
fn cached_llm(cache: &LlmCache, path: &Path) -> AppResult<Arc<LlamaModel>> {
    // Locked once and held across the load: a second operation can't reach here
    // anyway (see AppState::try_claim_llm), and re-locking would deadlock.
    let mut guard = cache.lock();
    if let Some((cached_path, model)) = guard.as_ref() {
        if cached_path == path {
            return Ok(model.clone());
        }
    }
    let model = Arc::new(summarize::llama::load_model(path)?);
    *guard = Some((path.to_path_buf(), model.clone()));
    Ok(model)
}

#[tauri::command]
pub async fn cmd_summarize(
    app: AppHandle,
    state: State<'_, AppState>,
    transcript: String,
    // "brief" = short plain retelling (default); "structured" = detailed report
    // with an overall topic, sub-topic sections, theses and lists. Unknown → brief.
    mode: Option<String>,
) -> Result<String, AppError> {
    let llm_path = state
        .llm_model_path
        .lock()
        .clone()
        .ok_or(AppError::LlmModelNotLoaded)?;

    let cancel = state.summarize_cancel.clone();
    cancel.store(false, Ordering::Relaxed);

    // The structured report is much longer than a brief summary and needs more
    // room to generate; brief mode keeps the tuned defaults.
    let structured = mode.as_deref() == Some("structured");
    let prompt = if structured { STRUCTURED_PROMPT } else { SUMMARIZE_PROMPT };
    let opts = if structured {
        summarize::llama::SummarizeOptions {
            max_new_tokens: 2048,
            ..summarize::llama::SummarizeOptions::default()
        }
    } else {
        summarize::llama::SummarizeOptions::default()
    };

    // One LLM operation at a time; the guard clears the flag on drop.
    let claim = state.try_claim_llm().ok_or(AppError::LlmBusy)?;
    state.make_room_for_llm();
    let cache = state.llm_cache.clone();

    let progress = progress_emitter(app);
    tauri::async_runtime::spawn_blocking(move || {
        let _claim = claim;
        let model = cached_llm(&cache, &llm_path)?;
        summarize::summarize(&model, prompt, &transcript, &opts, &cancel, progress)
    })
    .await
    .map_err(|e| AppError::Other(e.to_string()))?
}

/// Cleans up the transcript in place: the answer has the same segments, in the
/// same order, with the same timecodes, and only the text is edited.
///
/// `mode` picks how far the editing goes. "verbatim" (the default) touches only
/// punctuation, capitalization and sentence boundaries, leaving the words as they
/// were said. "edited" additionally fixes grammatical agreement, case endings and
/// word order, which matters in inflected languages but makes the result an edited
/// text rather than a literal record. Unknown → verbatim.
#[tauri::command]
pub async fn cmd_polish_transcript(
    app: AppHandle,
    state: State<'_, AppState>,
    segments: Vec<Segment>,
    mode: Option<String>,
) -> Result<Vec<Segment>, AppError> {
    let prompt = if mode.as_deref() == Some("edited") {
        POLISH_EDITED_PROMPT
    } else {
        POLISH_PROMPT
    };
    let llm_path = state
        .llm_model_path
        .lock()
        .clone()
        .ok_or(AppError::LlmModelNotLoaded)?;

    let cancel = state.summarize_cancel.clone();
    cancel.store(false, Ordering::Relaxed);

    let claim = state.try_claim_llm().ok_or(AppError::LlmBusy)?;
    state.make_room_for_llm();
    let cache = state.llm_cache.clone();

    let progress = progress_emitter(app.clone());
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _claim = claim;
        let model = cached_llm(&cache, &llm_path)?;
        summarize::polish(&model, prompt, &segments, &cancel, progress)
    })
    .await
    .map_err(|e| AppError::Other(e.to_string()))??;

    // Report lines that came back unedited, so the pass doesn't look like it did
    // nothing.
    if result.rejected_lines > 0 {
        let _ = app.emit(
            "summarize://degraded",
            DegradedEvent { lines: result.rejected_lines },
        );
    }
    Ok(result.segments)
}

/// Emitted when some lines came back unusable and kept their original text.
#[derive(Clone, serde::Serialize)]
struct DegradedEvent {
    lines: usize,
}

/// Cancels a running summarize / polish operation. The operation returns
/// `Cancelled`, which the frontend treats as a user action rather than an error.
#[tauri::command]
pub fn cmd_cancel_summarize(state: State<'_, AppState>) -> Result<(), AppError> {
    state.summarize_cancel.store(true, Ordering::Relaxed);
    Ok(())
}
