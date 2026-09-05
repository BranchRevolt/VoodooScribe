// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use voodooscribe_lib::audio::{decode_to_pcm, resample_to_16k};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

// Decode a file, verify it's non-empty and has a valid sample rate.
// Returns mono f32 samples resampled to 16 kHz.
fn full_pipeline(filename: &str) -> Vec<f32> {
    let path = fixture(filename);
    let pcm = decode_to_pcm(&path, &AtomicBool::new(false))
        .unwrap_or_else(|e| panic!("decode {filename}: {e}"));

    assert!(!pcm.samples.is_empty(), "{filename}: no samples decoded");
    assert!(pcm.sample_rate > 0, "{filename}: zero sample_rate");

    let out = resample_to_16k(pcm.samples, pcm.sample_rate)
        .unwrap_or_else(|e| panic!("resample {filename}: {e}"));

    assert!(!out.is_empty(), "{filename}: empty after resample");
    out
}

// RMS energy of a slice, used to confirm the audio isn't pure silence.
fn rms(samples: &[f32]) -> f32 {
    let sq: f32 = samples.iter().map(|s| s * s).sum();
    (sq / samples.len() as f32).sqrt()
}

#[test]
fn decode_wav() {
    let out = full_pipeline("tone.wav");
    // 0.5 s × 16 000 Hz = 8 000 samples ±10 %
    assert!(out.len() > 7_000 && out.len() < 9_000, "WAV length {}", out.len());
    assert!(rms(&out) > 0.1, "WAV too quiet");
}

#[test]
fn decode_flac() {
    let out = full_pipeline("tone.flac");
    assert!(out.len() > 7_000 && out.len() < 9_000, "FLAC length {}", out.len());
    assert!(rms(&out) > 0.1, "FLAC too quiet");
}

#[test]
fn decode_ogg() {
    let out = full_pipeline("tone.ogg");
    assert!(out.len() > 7_000 && out.len() < 9_000, "OGG length {}", out.len());
    assert!(rms(&out) > 0.05, "OGG too quiet");
}

#[test]
fn decode_aiff() {
    let out = full_pipeline("tone.aiff");
    assert!(out.len() > 7_000 && out.len() < 9_000, "AIFF length {}", out.len());
    assert!(rms(&out) > 0.1, "AIFF too quiet");
}

#[test]
fn decode_mp3() {
    let out = full_pipeline("tone.mp3");
    // MP3 encoder adds/trims a few frames, allow wider window
    assert!(out.len() > 6_000 && out.len() < 10_000, "MP3 length {}", out.len());
    assert!(rms(&out) > 0.05, "MP3 too quiet");
}

#[test]
fn resample_passthrough_when_already_16k() {
    let src: Vec<f32> = (0..16_000)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16_000.0).sin())
        .collect();
    let out = resample_to_16k(src.clone(), 16_000).unwrap();
    assert_eq!(out.len(), src.len(), "passthrough must not change length");
}

#[test]
fn resample_44100_to_16k_duration() {
    // 44100 Hz, 1 second → 16 000 samples ± 1 %
    let src: Vec<f32> = (0..44_100)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44_100.0).sin())
        .collect();
    let out = resample_to_16k(src, 44_100).unwrap();
    let expected = 16_000usize;
    // FftFixedIn has lookahead latency (~2%), allow up to 5 %
    let tolerance = expected / 20;
    assert!(
        out.len().abs_diff(expected) <= tolerance,
        "expected ~{expected} samples at 16 kHz, got {}",
        out.len()
    );
}
