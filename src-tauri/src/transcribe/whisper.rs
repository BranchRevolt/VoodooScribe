// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use whisper_rs::{
    FullParams, SamplingStrategy, SegmentCallbackData, WhisperContext, WhisperContextParameters,
};

use crate::error::{AppError, AppResult};
use crate::transcribe::hallucination;

// whisper.cpp's built-in Silero VAD model, embedded in the binary (~865 KB).
// whisper.cpp needs a file path rather than bytes, so `ensure_vad_model` writes it
// to disk once. Embedding rather than downloading keeps `tauri dev` and a bundled
// build identical, and it runs on the already-linked ggml engine.
const VAD_MODEL_BYTES: &[u8] = include_bytes!("../../resources/models/ggml-silero-v5.1.2.bin");
const VAD_MODEL_FILENAME: &str = "ggml-silero-v5.1.2.bin";

/// Writes the embedded Silero VAD model into `dir` (if not already there with the
/// right size) and returns its path. Used to feed whisper.cpp's built-in VAD.
pub fn ensure_vad_model(dir: &Path) -> AppResult<PathBuf> {
    let path = dir.join(VAD_MODEL_FILENAME);
    let up_to_date = std::fs::metadata(&path)
        .map(|m| m.len() == VAD_MODEL_BYTES.len() as u64)
        .unwrap_or(false);
    if !up_to_date {
        std::fs::create_dir_all(dir)?;
        std::fs::write(&path, VAD_MODEL_BYTES)?;
    }
    Ok(path)
}

/// Decoder settings that keep whisper from inventing text on silence and noise.
/// Most are whisper.cpp's own defaults, set explicitly so they are visible in one
/// place; `suppress_nst` is the one that differs:
/// - `no_context`   — never carry the previous window's text into the next
///   prompt, so one hallucination can't seed the ones after it.
/// - `suppress_nst` — suppress non-speech tokens (music/noise markers), which
///   whisper.cpp leaves OFF by default.
/// - `suppress_blank`, temperature fallback (0.0, +0.2 per retry) and the
///   entropy/logprob tholds — a window that decodes into gibberish or a
///   degenerate loop is re-decoded at a higher temperature.
///
/// The built-in no-speech gate is no help here: whisper.cpp drops a segment only
/// when `no_speech_prob > no_speech_thold` and `avg_logprobs < logprob_thold`, and
/// it is confident about its subtitle boilerplate, so the second condition never
/// holds. The VAD upstream and `hallucination::clean` downstream remove those.
fn apply_decoding_params(params: &mut FullParams) {
    params.set_no_context(true);
    params.set_suppress_blank(true);
    params.set_suppress_nst(true);
    params.set_temperature(0.0);
    params.set_temperature_inc(0.2);
    params.set_entropy_thold(2.4);
    params.set_logprob_thold(-1.0);
    params.set_no_speech_thold(0.6);
}

/// Abort callback that reads the shared cancel flag directly.
///
/// whisper-rs 0.16.0's `set_abort_callback_safe` is unsound: it stores a
/// `*mut Box<dyn FnMut() -> bool>` in `abort_callback_user_data` but installs
/// `trampoline::<F>` for the concrete closure type `F`. The trampoline casts the
/// fat-pointer box to `*mut F` and calls through it, so the abort answer is
/// whatever a stray heap byte holds; a non-zero one aborts the encoder and makes
/// `full()` fail with "failed to encode" (error -6). Being heap-layout dependent,
/// it typically lets the first file of a session through and fails on the second.
/// (`set_segment_callback_safe*` and `set_progress_callback_safe` instantiate
/// their trampolines with `Box<dyn ...>` and are sound.)
///
/// Passing the `AtomicBool` itself as user data removes the closure indirection:
/// no type confusion, and no box leaked per call, which the safe version does on
/// every invocation.
unsafe extern "C" fn abort_trampoline(user_data: *mut c_void) -> bool {
    if user_data.is_null() {
        return false;
    }
    // SAFETY: `apply_abort_flag` installs a pointer to the AtomicBool owned by the
    // Arc the caller holds across the whole `full()` call.
    unsafe { (*(user_data as *const AtomicBool)).load(Ordering::Relaxed) }
}

/// Wires `cancel` into whisper as the abort condition, so `full()` returns
/// mid-inference instead of running to completion after a cancel.
///
/// SAFETY: `cancel` must outlive the `full()` call `params` is handed to. Every
/// caller here keeps the `Arc` alive for the duration of that call.
fn apply_abort_flag(params: &mut FullParams, cancel: &Arc<AtomicBool>) {
    unsafe {
        params.set_abort_callback(Some(abort_trampoline));
        params.set_abort_callback_user_data(Arc::as_ptr(cancel) as *mut c_void);
    }
}

// Segment is both produced by transcription and accepted by the export command, hence Deserialize.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Segment {
    pub t0: i64, // milliseconds
    pub t1: i64,
    pub text: String,
}

pub struct TranscribeOptions {
    pub language: Option<String>, // None = auto
    pub n_threads: i32,
    /// Some(path) → run whisper.cpp's built-in Silero VAD (the "Strip silence"
    /// toggle); None → transcribe the audio as-is.
    pub vad_model_path: Option<PathBuf>,
}

impl Default for TranscribeOptions {
    fn default() -> Self {
        Self {
            language: None,
            n_threads: num_cpus(),
            vad_model_path: None,
        }
    }
}

fn num_cpus() -> i32 {
    std::thread::available_parallelism()
        .map(|n| n.get().min(8) as i32)
        .unwrap_or(4)
}

/// Loads a whisper model into a context. Slow: a big model streams >1 GB into
/// VRAM, about 9 s. The result is cached and reused across runs.
pub fn load_context(model_path: &Path) -> AppResult<WhisperContext> {
    // whisper-rs 0.16 takes impl AsRef<Path> directly
    WhisperContext::new_with_params(model_path, WhisperContextParameters::default()).map_err(|e| {
        // On a small or busy card a load failure is almost always VRAM, so the
        // message says so with numbers instead of "failed to load model".
        let raw = e.to_string();
        let required = crate::vram::model_file_size_mb(model_path);
        match crate::vram::classify_load_failure(required, &raw) {
            Some((required_mb, free_mb)) => AppError::InsufficientMemory { required_mb, free_mb },
            None => AppError::Transcription(raw),
        }
    })
}

/// Convenience wrapper that loads + transcribes in one go (used by tests/CLI).
pub fn transcribe<F>(
    model_path: &Path,
    samples: &[f32],
    opts: &TranscribeOptions,
    cancel: Arc<AtomicBool>,
    on_progress: F,
) -> AppResult<Vec<Segment>>
where
    F: FnMut(u8) + Send + 'static,
{
    let ctx = load_context(model_path)?;
    transcribe_auto(&ctx, samples, opts, cancel, on_progress)
}

/// Picks how to slice the audio and transcribes it. Shared by the Tauri command
/// and the CLI so both behave identically.
///
/// 1. VAD on: only the stretches that contain speech. Silence never reaches
///    whisper, so it can't hallucinate filler into it, and no window starts inside
///    a pause.
/// 2. VAD off, language "auto": cut on pauses, each chunk detecting its own
///    language (mixed-language files).
/// 3. VAD off, fixed language: one straight pass.
pub fn transcribe_auto<F>(
    ctx: &WhisperContext,
    samples: &[f32],
    opts: &TranscribeOptions,
    cancel: Arc<AtomicBool>,
    on_progress: F,
) -> AppResult<Vec<Segment>>
where
    F: FnMut(u8) + 'static,
{
    match (&opts.vad_model_path, &opts.language) {
        (Some(vad_model), language) => {
            let ranges = super::speech::speech_chunks(vad_model, samples)?;
            transcribe_chunks(
                ctx,
                samples,
                &ranges,
                language.as_deref(),
                opts.n_threads,
                cancel,
                on_progress,
            )
        }
        (None, None) => {
            transcribe_multilang(ctx, samples, opts.n_threads, &None, cancel, on_progress)
        }
        (None, Some(_)) => transcribe_with(ctx, samples, opts, cancel, on_progress),
    }
}

/// Transcribes in independent 30s windows, each detecting its own language. This
/// is what handles mixed-language files (an English intro followed by Russian):
/// a single whisper pass picks one language from the first window and mis-reads
/// the rest. The cost is that a word landing on a 30s cut can be clipped and a
/// noisy window may detect the wrong language. Used only when language is "auto".
pub fn transcribe_multilang<F>(
    ctx: &WhisperContext,
    samples: &[f32],
    n_threads: i32,
    _vad_model_path: &Option<PathBuf>,
    cancel: Arc<AtomicBool>,
    on_progress: F,
) -> AppResult<Vec<Segment>>
where
    F: FnMut(u8) + 'static,
{
    // Cut on pauses rather than a fixed timer, so words aren't sliced at chunk
    // borders.
    let ranges = crate::audio::vad::split_on_silence(samples);
    transcribe_chunks(ctx, samples, &ranges, None, n_threads, cancel, on_progress)
}

/// Transcribes a prepared list of `(start, end)` sample ranges, one `full()` call
/// each, offsetting every segment back onto the original timeline.
///
/// `language` is `None` for per-window auto-detection (mixed-language files) or
/// `Some(code)` to pin every chunk to one language. Ranges come from the VAD
/// (`speech::speech_chunks`, silence already removed) or from the pause finder
/// (`audio::vad::split_on_silence`).
pub fn transcribe_chunks<F>(
    ctx: &WhisperContext,
    samples: &[f32],
    ranges: &[(usize, usize)],
    language: Option<&str>,
    n_threads: i32,
    cancel: Arc<AtomicBool>,
    mut on_progress: F,
) -> AppResult<Vec<Segment>>
where
    F: FnMut(u8) + 'static,
{
    // Progress tracks the audio handed to whisper, so a file whose silence was cut
    // still reaches 100%.
    let total: usize = ranges.iter().map(|&(s, e)| e - s).sum::<usize>().max(1);
    let mut done = 0usize;
    let mut out = Vec::new();

    // One state is reused across all chunks. whisper.cpp clears prior results at
    // the start of every full() call and caches the loaded VAD model on the state,
    // so a fresh state per chunk would reload the Silero model dozens of times on
    // a long file.
    let mut state = ctx
        .create_state()
        .map_err(|e| AppError::Transcription(e.to_string()))?;

    for &(start, end) in ranges {
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }
        let offset_ms = (start / 16) as i64;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(n_threads);
        params.set_print_realtime(false);
        params.set_print_progress(false);
        params.set_print_timestamps(false);
        params.set_print_special(false);
        params.set_token_timestamps(false);
        // None: re-detect per window (mixed-language files). Some: pinned.
        params.set_language(Some(language.unwrap_or("auto")));
        apply_decoding_params(&mut params);
        // Aborts mid-chunk, not only between chunks.
        apply_abort_flag(&mut params, &cancel);

        let full_res = state.full(params, &samples[start..end]);
        // An abort from a cancel makes full() error; report it as a cancellation.
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }
        full_res.map_err(|e| AppError::Transcription(e.to_string()))?;

        let n = state.full_n_segments();
        for i in 0..n {
            let seg = state
                .get_segment(i)
                .ok_or_else(|| AppError::Transcription(format!("segment {i} out of range")))?;
            let text = seg
                .to_str_lossy()
                .map_err(|e| AppError::Transcription(e.to_string()))?
                .trim()
                .to_string();
            if text.is_empty() {
                continue;
            }
            out.push(Segment {
                t0: seg.start_timestamp() * 10 + offset_ms,
                t1: seg.end_timestamp() * 10 + offset_ms,
                text,
            });
        }

        done += end - start;
        on_progress(((done as f64 / total as f64) * 100.0).clamp(0.0, 100.0) as u8);
    }

    Ok(hallucination::clean(out))
}

/// Transcribes using an already-loaded context. Creating per-call state is cheap;
/// the expensive model load is done once in `load_context`.
pub fn transcribe_with<F>(
    ctx: &WhisperContext,
    samples: &[f32],
    opts: &TranscribeOptions,
    cancel: Arc<AtomicBool>,
    mut on_progress: F,
) -> AppResult<Vec<Segment>>
where
    F: FnMut(u8) + 'static,
{
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

    params.set_n_threads(opts.n_threads);
    params.set_print_realtime(false);
    params.set_print_progress(false);
    params.set_print_timestamps(false);
    params.set_print_special(false);
    params.set_token_timestamps(false);

    if let Some(lang) = &opts.language {
        params.set_language(Some(lang.as_str()));
    } else {
        params.set_language(Some("auto"));
    }

    apply_decoding_params(&mut params);

    // whisper's progress_callback only fires at 30s-chunk boundaries (0 and 100 on
    // a short clip), so progress is derived from each new segment's end time
    // against the total duration, which advances as speech is recognized.
    // The abort callback lets whisper leave full() mid-inference (returning true
    // aborts); whisper polls it between compute steps. Without it a fixed-language
    // single pass runs to completion even after a cancel.
    apply_abort_flag(&mut params, &cancel);

    let total_ms = (samples.len() as f64 / 16.0).max(1.0); // 16 samples per ms @ 16 kHz
    let cancel_clone = cancel.clone();
    params.set_segment_callback_safe_lossy(move |data: SegmentCallbackData| {
        if cancel_clone.load(Ordering::Relaxed) {
            return;
        }
        let end_ms = data.end_timestamp as f64 * 10.0; // centiseconds → ms
        let pct = ((end_ms / total_ms) * 100.0).clamp(0.0, 100.0) as u8;
        on_progress(pct);
    });

    let mut state = ctx
        .create_state()
        .map_err(|e| AppError::Transcription(e.to_string()))?;

    let full_res = state.full(params, samples);
    // A cancel triggers the abort callback, which makes full() return an error, so
    // the flag is checked first and the result reported as a cancellation.
    if cancel.load(Ordering::Relaxed) {
        return Err(AppError::Cancelled);
    }
    full_res.map_err(|e| AppError::Transcription(e.to_string()))?;

    // full_n_segments() returns i32 directly in whisper-rs 0.16, not a Result
    let n_segments = state.full_n_segments();

    let mut segments = Vec::with_capacity(n_segments as usize);
    for i in 0..n_segments {
        let seg = state
            .get_segment(i)
            .ok_or_else(|| AppError::Transcription(format!("segment {i} out of range")))?;

        let text = seg
            .to_str_lossy()
            .map_err(|e| AppError::Transcription(e.to_string()))?
            .trim()
            .to_string();

        // whisper timestamps are in centiseconds (10 ms), convert to ms
        segments.push(Segment {
            t0: seg.start_timestamp() * 10,
            t1: seg.end_timestamp() * 10,
            text,
        });
    }

    Ok(hallucination::clean(segments))
}
