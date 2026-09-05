// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

//! Regression tests for keeping the LLM loaded between operations.
//!
//! They cover two failures:
//!   * `summarize`/`polish` calling `LlamaBackend::init()` and
//!     `LlamaModel::load_from_file()` themselves, which loads Qwen3-4B into VRAM
//!     and drops it again on every operation (~3–5 s each time).
//!   * `LlamaBackend::init()` being a process-wide singleton guarded by an atomic
//!     flag: it returns `BackendAlreadyInitialized` on a second call while the
//!     first backend is alive, so two overlapping LLM operations fail.
//!
//! The model test is env-gated (the GGUF is ~2.5 GB and not in the repo):
//!   VOODOOSCRIBE_TEST_LLM_MODEL=/path/Qwen3-4B-Q4_K_M.gguf

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use voodooscribe_lib::summarize::llama::{load_model, SummarizeOptions};
use voodooscribe_lib::summarize::{polish, summarize};
use voodooscribe_lib::transcribe::Segment;

const PROMPT: &str = "Summarize the text in one short sentence, in the same language as the text.";
const POLISH_PROMPT: &str = "Add punctuation and capitalization. Do not change the words.";
const TEXT: &str = "so we talked about the schedule and agreed to start on monday \
the client asked for a shorter onboarding and we said that is fine";

fn model_path() -> Option<PathBuf> {
    std::env::var_os("VOODOOSCRIBE_TEST_LLM_MODEL")
        .map(PathBuf::from)
        .filter(|p| p.exists())
}

/// The backend must be obtainable more than once: a fresh `LlamaBackend` per call
/// fails on the second one.
#[test]
fn backend_can_be_acquired_repeatedly() {
    let Some(path) = model_path() else {
        eprintln!("SKIP: set VOODOOSCRIBE_TEST_LLM_MODEL to run");
        return;
    };
    // load_model() goes through the shared backend; a per-call backend would hit
    // BackendAlreadyInitialized here.
    let first = load_model(&path).expect("first load failed");
    let second = load_model(&path).expect("second load failed — backend not shared?");
    drop(first);
    drop(second);
}

/// One loaded model must serve several operations back to back, summarize and
/// polish alike, which is what the AppState cache is for.
#[test]
fn one_model_serves_repeated_operations() {
    let Some(path) = model_path() else {
        eprintln!("SKIP: set VOODOOSCRIBE_TEST_LLM_MODEL to run");
        return;
    };
    let model = load_model(&path).expect("model load failed");
    let cancel = AtomicBool::new(false);
    let opts = SummarizeOptions { max_new_tokens: 96, ..SummarizeOptions::default() };

    for run in 1..=2 {
        let out = summarize(&model, PROMPT, TEXT, &opts, &cancel, |_, _| {})
            .unwrap_or_else(|e| panic!("summarize run {run} failed: {e:?}"));
        assert!(!out.trim().is_empty(), "summarize run {run} produced nothing");
    }

    // A different operation on the same cached model must work too.
    let segments: Vec<Segment> = TEXT
        .split(" and ")
        .enumerate()
        .map(|(i, t)| Segment { t0: i as i64 * 1000, t1: (i as i64 + 1) * 1000, text: t.to_string() })
        .collect();
    let polished = polish(&model, POLISH_PROMPT, &segments, &cancel, |_, _| {})
        .expect("polish on the reused model failed")
        .segments;
    // The pass edits in place: same count, same order, same timecodes, whatever
    // the model answers.
    assert_eq!(polished.len(), segments.len(), "polish changed the number of segments");
    assert!(polished.iter().all(|p| !p.text.trim().is_empty()));
    assert!(
        polished.iter().zip(&segments).all(|(a, b)| a.t0 == b.t0 && a.t1 == b.t1),
        "polish moved the timecodes",
    );
}

/// A pre-set cancel flag must stop the run instead of being ignored.
#[test]
fn cancel_flag_stops_generation() {
    let Some(path) = model_path() else {
        eprintln!("SKIP: set VOODOOSCRIBE_TEST_LLM_MODEL to run");
        return;
    };
    let model = load_model(&path).expect("model load failed");
    let cancel = AtomicBool::new(true);
    let opts = SummarizeOptions::default();
    let err = summarize(&model, PROMPT, TEXT, &opts, &cancel, |_, _| {})
        .expect_err("a pre-set cancel flag must stop the run");
    assert!(
        matches!(err, voodooscribe_lib::error::AppError::Cancelled),
        "expected Cancelled, got {err:?}"
    );
}
