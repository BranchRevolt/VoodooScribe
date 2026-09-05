// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

//! End-to-end ASR smoke test (decode → resample → whisper).
//!
//! Gated on env vars so the default `cargo test` stays fast and offline: neither
//! the 74 MB model nor a speech fixture is in the repo. To run it:
//!
//!   VOODOOSCRIBE_TEST_WHISPER_MODEL=/path/to/ggml-tiny.bin \
//!   VOODOOSCRIBE_TEST_AUDIO=/path/to/speech.wav \
//!   VOODOOSCRIBE_TEST_EXPECT=hello \   # optional substring to assert (lowercased)
//!   VOODOOSCRIBE_TEST_LANG=en \        # optional fixed language (else auto)
//!   cargo test --test asr -- --nocapture
//!
//! Without the model/audio vars it prints a SKIP note and passes.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use voodooscribe_lib::audio::{decode_to_pcm, resample_to_16k};
use voodooscribe_lib::transcribe::{transcribe, TranscribeOptions};

#[test]
fn transcribes_known_speech() {
    let (Some(model), Some(audio)) = (
        std::env::var_os("VOODOOSCRIBE_TEST_WHISPER_MODEL"),
        std::env::var_os("VOODOOSCRIBE_TEST_AUDIO"),
    ) else {
        eprintln!(
            "SKIP asr: set VOODOOSCRIBE_TEST_WHISPER_MODEL (ggml-tiny.bin) and \
             VOODOOSCRIBE_TEST_AUDIO (a speech file) to run this test"
        );
        return;
    };

    let model = PathBuf::from(model);
    let audio = PathBuf::from(audio);
    assert!(model.exists(), "model not found: {}", model.display());
    assert!(audio.exists(), "audio not found: {}", audio.display());

    // Reuse the real decode + resample pipeline.
    let pcm = decode_to_pcm(&audio, &AtomicBool::new(false)).expect("decode failed");
    let samples = resample_to_16k(pcm.samples, pcm.sample_rate).expect("resample failed");
    assert!(!samples.is_empty(), "no samples after resample");

    // A fixed language takes the single-pass path, auto the multilang chunker.
    let opts = TranscribeOptions {
        language: std::env::var("VOODOOSCRIBE_TEST_LANG").ok(),
        ..Default::default()
    };

    let segments = transcribe(&model, &samples, &opts, Arc::new(AtomicBool::new(false)), |_| {})
        .expect("transcription failed");

    let text = segments
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    eprintln!("ASR output: {text:?}");

    assert!(!segments.is_empty(), "no segments produced");
    assert!(!text.trim().is_empty(), "transcription is empty");

    if let Ok(expect) = std::env::var("VOODOOSCRIBE_TEST_EXPECT") {
        let expect = expect.to_lowercase();
        assert!(text.contains(&expect), "expected {expect:?} in transcription {text:?}");
    }
}
