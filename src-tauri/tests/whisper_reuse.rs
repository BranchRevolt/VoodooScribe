// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

//! Regression test: a cached WhisperContext must survive repeated use.
//!
//! Guards against the second and later transcription in a session failing with
//! "Generic whisper error ... Error code: -6", whisper's "failed to encode",
//! raised when the abort callback asks to abort. whisper-rs 0.16.0's
//! `set_abort_callback_safe` installs a trampoline typed for the concrete closure
//! while handing it a `Box<dyn FnMut() -> bool>`, so the abort decision comes from
//! a stray heap byte and, depending on heap layout, lets the first file through.
//! See `transcribe::whisper::abort_trampoline`.
//!
//! Env-gated so the default `cargo test` stays offline: the model is 1.6 GB and
//! isn't in the repo.
//!   VOODOOSCRIBE_TEST_WHISPER_MODEL=/path/ggml-*.bin
//!   VOODOOSCRIBE_TEST_AUDIO=/path/any.wav

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use voodooscribe_lib::audio::{decode_to_pcm, resample_to_16k};
use voodooscribe_lib::transcribe::{load_context, transcribe_multilang, transcribe_with, TranscribeOptions};

/// Both tests load the full model onto the GPU; running them at once means two
/// 1.6 GB contexts side by side, which segfaults inside the Vulkan backend. Cargo
/// runs tests in one process, so a plain static lock serializes them.
static GPU: Mutex<()> = Mutex::new(());

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key).map(PathBuf::from).filter(|p| p.exists())
}

#[test]
fn cached_context_survives_repeated_runs() {
    let (Some(model), Some(audio)) = (
        env_path("VOODOOSCRIBE_TEST_WHISPER_MODEL"),
        env_path("VOODOOSCRIBE_TEST_AUDIO"),
    ) else {
        eprintln!("SKIP: set VOODOOSCRIBE_TEST_WHISPER_MODEL and VOODOOSCRIBE_TEST_AUDIO to run");
        return;
    };

    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let pcm = decode_to_pcm(&audio, &AtomicBool::new(false)).expect("decode failed");
    let samples = resample_to_16k(pcm.samples, pcm.sample_rate).expect("resample failed");

    // One context, reused, as AppState.whisper_cache serves the queue.
    let ctx = load_context(&model).expect("model load failed");
    let cancel = Arc::new(AtomicBool::new(false));

    // Fixed language: single pass. Three runs, since the bug starts at the second.
    for run in 1..=3 {
        let opts = TranscribeOptions { language: Some("en".into()), ..Default::default() };
        transcribe_with(&ctx, &samples, &opts, cancel.clone(), |_| {})
            .unwrap_or_else(|e| panic!("single-pass run {run} failed: {e:?}"));
    }

    // Auto language: the per-window path, which installs the abort callback once
    // per chunk.
    for run in 1..=2 {
        transcribe_multilang(&ctx, &samples, 4, &None, cancel.clone(), |_| {})
            .unwrap_or_else(|e| panic!("multilang run {run} failed: {e:?}"));
    }
}

#[test]
fn cancel_flag_still_aborts() {
    let (Some(model), Some(audio)) = (
        env_path("VOODOOSCRIBE_TEST_WHISPER_MODEL"),
        env_path("VOODOOSCRIBE_TEST_AUDIO"),
    ) else {
        eprintln!("SKIP: set VOODOOSCRIBE_TEST_WHISPER_MODEL and VOODOOSCRIBE_TEST_AUDIO to run");
        return;
    };

    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let pcm = decode_to_pcm(&audio, &AtomicBool::new(false)).expect("decode failed");
    let samples = resample_to_16k(pcm.samples, pcm.sample_rate).expect("resample failed");
    let ctx = load_context(&model).expect("model load failed");

    // Pre-set flag: the abort callback must actually be read, so inference stops
    // and the error surfaces as a cancellation rather than a failure.
    let cancel = Arc::new(AtomicBool::new(true));
    let opts = TranscribeOptions { language: Some("en".into()), ..Default::default() };
    let err = transcribe_with(&ctx, &samples, &opts, cancel, |_| {})
        .expect_err("a pre-set cancel flag must abort inference");
    assert!(
        matches!(err, voodooscribe_lib::error::AppError::Cancelled),
        "expected Cancelled, got {err:?}"
    );
}
