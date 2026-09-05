// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::audio;
use crate::error::AppError;
use crate::state::AppState;
use crate::transcribe::{self, Segment, TranscribeOptions};

#[derive(Clone, serde::Serialize)]
struct TranscribeProgress {
    percent: u8,
    /// Pipeline stage the UI shows as a label:
    /// "decoding"     — decode + resample (+ optional VAD) of the whole file (scales with length),
    /// "loading"      — the model is being read into VRAM (once, then cached),
    /// "transcribing" — whisper inference.
    phase: &'static str,
    segment: Option<Segment>,
    /// Free-form heartbeat for phases with no percentage of their own: currently
    /// how much audio has been decoded so far ("3:12" / "3:12 / 41:07").
    detail: Option<String>,
}

#[tauri::command]
pub async fn cmd_transcribe(
    app: AppHandle,
    state: State<'_, AppState>,
    file_path: String,
    language: Option<String>,
    use_vad: bool,
    n_threads: Option<i32>,
) -> Result<Vec<Segment>, AppError> {
    // The file may have been moved or deleted since it was queued; fail here with
    // "file not found" rather than with a decode error later.
    if !std::path::Path::new(&file_path).exists() {
        return Err(AppError::FileNotFound(file_path));
    }

    let whisper_path = state
        .whisper_model_path
        .lock()
        .clone()
        .ok_or(AppError::WhisperModelNotLoaded)?;

    let cancel = state.cancel_flag.clone();
    cancel.store(false, Ordering::Relaxed);

    // On a card that can't hold whisper and the LLM at once, drop the LLM before
    // loading whisper. A running summarize holds its own Arc, so this cannot pull
    // the model out from under an operation in flight.
    state.make_room_for_whisper();

    // "Strip silence" drives whisper.cpp's built-in Silero VAD, which needs the
    // embedded VAD model materialized to a path it can load.
    let vad_model_path = if use_vad {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| AppError::Other(e.to_string()))?;
        Some(transcribe::ensure_vad_model(&dir)?)
    } else {
        None
    };

    // Decoding and resampling scale with file length and emit no progress of
    // their own, so announce the "decoding" stage up front.
    let _ = app.emit(
        "transcribe://progress",
        TranscribeProgress { percent: 0, phase: "decoding", segment: None, detail: None },
    );

    // 16 kHz mono f32 samples: symphonia in-process, falling back to the bundled
    // ffmpeg sidecar for codecs/containers it can't handle (Opus, AC-3, DTS, AMR,
    // AVI, …).
    let samples = load_samples_16k(&app, &file_path, cancel.clone()).await?;

    // Cancelled during decode/resample: stop before loading the model.
    if cancel.load(Ordering::Relaxed) {
        return Err(AppError::Cancelled);
    }

    let app_clone = app.clone();
    let cancel_clone = cancel.clone();
    let cache = state.whisper_cache.clone();

    tauri::async_runtime::spawn_blocking(move || {
        // Cancelled in the gap before inference: don't load the model.
        if cancel_clone.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }

        let threads = n_threads.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get().min(8) as i32)
                .unwrap_or(4)
        });

        let opts = TranscribeOptions {
            language,
            n_threads: threads,
            vad_model_path,
        };

        // Cached context for this model, or a load, which for big models streams
        // >1 GB into VRAM. The "loading" phase puts the UI on an indeterminate bar
        // for its duration.
        let ctx = {
            let mut guard = cache.lock();
            match &*guard {
                Some((p, c)) if *p == whisper_path => c.clone(),
                _ => {
                    let _ = app_clone.emit(
                        "transcribe://progress",
                        TranscribeProgress { percent: 0, phase: "loading", segment: None, detail: None },
                    );
                    let c = std::sync::Arc::new(transcribe::load_context(&whisper_path)?);
                    *guard = Some((whisper_path.clone(), c.clone()));
                    c
                }
            }
        };

        let app_for_cb = app_clone.clone();
        // Emit only when the percent changes, to avoid flooding the WebKitGTK IPC
        // channel.
        let mut last_pct: i32 = -1;
        let on_progress = move |percent: u8| {
            if percent as i32 != last_pct {
                last_pct = percent as i32;
                let _ = app_for_cb.emit(
                    "transcribe://progress",
                    TranscribeProgress { percent, phase: "transcribing", segment: None, detail: None },
                );
            }
        };

        // The slicing strategy (VAD chunks / pause chunks / single pass) lives in
        // transcribe_auto, shared with the CLI.
        let segments = transcribe::transcribe_auto(&ctx, &samples, &opts, cancel_clone, on_progress)?;

        // All segments go to the UI once inference completes.
        for seg in &segments {
            let _ = app_clone.emit(
                "transcribe://progress",
                TranscribeProgress {
                    percent: 100,
                    phase: "transcribing",
                    segment: Some(seg.clone()),
                    detail: None,
                },
            );
        }

        Ok(segments)
    })
    .await
    .map_err(|e| AppError::Other(e.to_string()))?
}

/// Formats decoded-so-far as a `m:ss` heartbeat and emits it on the decoding
/// phase. `rate` 0 means the source rate isn't known yet, in which case only the
/// raw sample count is available and no time is shown.
fn emit_decode_heartbeat(app: &AppHandle, frames: u64, total: Option<u64>, rate: u32) {
    let fmt = |f: u64, r: u32| {
        let secs = f / r.max(1) as u64;
        format!("{}:{:02}", secs / 60, secs % 60)
    };
    let (percent, detail) = match (rate, total) {
        (0, _) => (0u8, None),
        (r, Some(t)) if t > 0 => (
            ((frames as f64 / t as f64) * 100.0).clamp(0.0, 100.0) as u8,
            Some(format!("{} / {}", fmt(frames, r), fmt(t, r))),
        ),
        (r, _) => (0, Some(fmt(frames, r))),
    };
    let _ = app.emit(
        "transcribe://progress",
        TranscribeProgress { percent, phase: "decoding", segment: None, detail },
    );
}

/// Decodes a file to 16 kHz mono f32. Tries symphonia first (fast, in-process);
/// on any decode failure falls back to the ffmpeg sidecar (universal codec support).
async fn load_samples_16k(
    app: &AppHandle,
    file_path: &str,
    cancel: Arc<AtomicBool>,
) -> Result<Vec<f32>, AppError> {
    let fp = file_path.to_string();
    let app_dec = app.clone();
    let native = tauri::async_runtime::spawn_blocking(move || {
        let pcm = audio::decode_to_pcm_with_progress(
            &PathBuf::from(&fp),
            &cancel,
            // Frames are source-rate mono samples; the rate comes from the
            // decoder's spec, so a time can be shown once it is known.
            &mut |frames, total, rate| emit_decode_heartbeat(&app_dec, frames, total, rate),
        )?;
        // Resampling the whole buffer is lengthy too, so check for a cancel here.
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }
        audio::resample_to_16k(pcm.samples, pcm.sample_rate)
    })
    .await
    .map_err(|e| AppError::Other(e.to_string()))?;

    let samples = match native {
        Ok(samples) => samples,
        // A cancel is propagated as such, not treated as a decode failure that
        // falls through to ffmpeg.
        Err(AppError::Cancelled) => return Err(AppError::Cancelled),
        // symphonia couldn't handle it; ffmpeg gets a turn (Opus, AC-3, AVI, …).
        Err(_) => {
            let app_ff = app.clone();
            audio::ffmpeg::decode_16k_mono(app, file_path, &mut |samples| {
                emit_decode_heartbeat(&app_ff, samples, None, 16_000)
            })
            .await?
        }
    };

    Ok(samples)
}

#[tauri::command]
pub fn cmd_cancel_transcribe(state: State<'_, AppState>) -> Result<(), AppError> {
    state.cancel_flag.store(true, Ordering::Relaxed);
    Ok(())
}
