// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

//! Silero VAD: reduces the audio to the stretches that contain speech.
//!
//! whisper.cpp applies its built-in VAD only inside `whisper_full()`
//! (`whisper.cpp:7750`), while whisper-rs transcribes through
//! `WhisperState::full()` → `whisper_full_with_state()`, which never reads
//! `params.vad`. `FullParams::enable_vad(true)` is therefore inert with this
//! crate.
//!
//! The VAD is instead run directly through whisper-rs's `WhisperVadContext`, and
//! whisper receives only the speech ranges, with timestamps offset back onto the
//! original timeline. Silence then never reaches the model (no "Дякую за
//! перегляд!" hallucinations) and no 30s window starts inside a long pause, which
//! made whisper collapse the window into one bogus segment and skip the speech
//! after it.

use std::path::Path;

use whisper_rs::{WhisperVadContext, WhisperVadContextParams, WhisperVadParams};

use crate::error::{AppError, AppResult};

/// 16 kHz mono, so one second is 16 000 samples.
const RATE: usize = 16_000;
/// Stay under whisper's 30s analysis window.
const MAX_CHUNK: usize = 28 * RATE;
/// Neighbouring segments separated by less than this are merged: speech shorter
/// than a whisper window is cheaper to transcribe in one `full()` call.
const MERGE_GAP: usize = RATE; // 1 s
/// Padding kept around each segment: VAD boundaries clip plosives and trailing
/// vowels, and whisper reads better with a short run-up.
const PAD: usize = RATE / 5; // 200 ms

/// Runs the VAD over the whole buffer and returns `(start, end)` sample ranges
/// that contain speech, merged and capped to whisper's window size.
///
/// An empty result means the VAD found no speech at all, which the caller reports
/// as such instead of transcribing the file anyway.
pub fn speech_chunks(vad_model: &Path, samples: &[f32]) -> AppResult<Vec<(usize, usize)>> {
    let model_path = vad_model
        .to_str()
        .ok_or_else(|| AppError::Transcription("VAD model path is not valid UTF-8".into()))?;

    let mut ctx = WhisperVadContext::new(model_path, WhisperVadContextParams::default())
        .map_err(|e| AppError::Transcription(format!("VAD init failed: {e}")))?;

    let mut params = WhisperVadParams::default();
    // The defaults split on every breath (100 ms of silence), which would send
    // whisper a stream of fragments and cost it context. Short pauses are merged
    // across instead, with the padding covering the edges.
    params.set_min_silence_duration(300);
    params.set_speech_pad(200);

    let segments = ctx
        .segments_from_samples(params, samples)
        .map_err(|e| AppError::Transcription(format!("VAD failed: {e}")))?;

    // whisper-rs reports VAD timestamps in centiseconds.
    let to_sample = |cs: f32| ((cs.max(0.0) as f64 / 100.0) * RATE as f64) as usize;

    let mut raw: Vec<(usize, usize)> = Vec::new();
    for seg in segments {
        let start = to_sample(seg.start).saturating_sub(PAD);
        let end = (to_sample(seg.end) + PAD).min(samples.len());
        if end > start {
            raw.push((start, end));
        }
    }
    raw.sort_by_key(|&(s, _)| s);

    Ok(merge_and_cap(raw))
}

/// Merges overlapping/near-adjacent ranges, then splits anything longer than a
/// whisper window. Kept separate from the FFI above so it can be unit-tested.
fn merge_and_cap(raw: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in raw {
        match merged.last_mut() {
            // Extend the open chunk while it fits inside one whisper window.
            Some(last)
                if start <= last.1 + MERGE_GAP && end.saturating_sub(last.0) <= MAX_CHUNK =>
            {
                last.1 = last.1.max(end);
            }
            _ => merged.push((start, end)),
        }
    }

    let mut out = Vec::with_capacity(merged.len());
    for (start, end) in merged {
        let mut pos = start;
        while end - pos > MAX_CHUNK {
            out.push((pos, pos + MAX_CHUNK));
            pos += MAX_CHUNK;
        }
        if end > pos {
            out.push((pos, end));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_short_gaps_only() {
        // 0–1 s and 1.5–2 s are half a second apart: one chunk.
        let out = merge_and_cap(vec![(0, RATE), (RATE * 3 / 2, RATE * 2)]);
        assert_eq!(out, vec![(0, RATE * 2)]);

        // A 5 s gap is a real pause: two chunks, and the silence is dropped.
        let out = merge_and_cap(vec![(0, RATE), (RATE * 6, RATE * 7)]);
        assert_eq!(out, vec![(0, RATE), (RATE * 6, RATE * 7)]);
    }

    #[test]
    fn caps_chunks_at_the_whisper_window() {
        let out = merge_and_cap(vec![(0, 70 * RATE)]);
        assert_eq!(out.len(), 3);
        assert!(out.iter().all(|&(s, e)| e - s <= MAX_CHUNK));
        assert_eq!(out[0].0, 0);
        assert_eq!(out.last().unwrap().1, 70 * RATE);
        // Contiguous: no audio is dropped when splitting a long run of speech.
        for pair in out.windows(2) {
            assert_eq!(pair[0].1, pair[1].0);
        }
    }

    #[test]
    fn never_merges_past_the_window() {
        // Two 20 s runs 0.5 s apart: merging would exceed 28 s, so they stay
        // separate.
        let out = merge_and_cap(vec![(0, 20 * RATE), (20 * RATE + RATE / 2, 40 * RATE)]);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn empty_input_yields_no_chunks() {
        assert!(merge_and_cap(vec![]).is_empty());
    }
}
