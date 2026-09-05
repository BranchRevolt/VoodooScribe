// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

//! Energy-based pause finder used to split long audio into per-window chunks for
//! language detection. Silence removal itself is done by whisper.cpp's built-in
//! Silero VAD, see `transcribe::whisper::apply_vad`.

const FRAME_SIZE: usize = 160; // 10 ms at 16 kHz

/// Upper bound for what may count as silence: mean square 0.001 ≈ −30 dBFS RMS.
/// As a fixed threshold it read quiet recordings (speech at −35 dBFS) as silence
/// throughout; it now only caps the adaptive threshold, so loud files behave as
/// they did before.
const MAX_SILENCE_ENERGY: f32 = 0.001;
/// Floor, so a digitally-silent file doesn't end up with a zero threshold that
/// nothing can fall below.
const MIN_SILENCE_ENERGY: f32 = 1e-7;

/// Splits audio into chunks for per-window language detection, cutting inside
/// pauses rather than on a fixed timer. Keeps each chunk under whisper's 30s
/// window without slicing through a word, and lets each phrase get its own
/// auto-detected language. Returns (start, end) sample ranges covering the whole
/// buffer; falls back to a hard cut when a run of speech exceeds the max with no
/// pause to cut at.
pub fn split_on_silence(samples: &[f32]) -> Vec<(usize, usize)> {
    const MIN_SILENCE_FRAMES: usize = 40; // 400 ms pause qualifies as a cut point
    const MAX_CHUNK: usize = 28 * 16_000; // stay under whisper's 30s window
    const MIN_CHUNK: usize = 5 * 16_000; // don't make tiny chunks

    let total = samples.len();
    if total <= MAX_CHUNK {
        return vec![(0, total)];
    }

    let frame_count = total / FRAME_SIZE;

    let energies: Vec<f32> = (0..frame_count)
        .map(|i| {
            let frame = &samples[i * FRAME_SIZE..(i + 1) * FRAME_SIZE];
            frame.iter().map(|s| s * s).sum::<f32>() / FRAME_SIZE as f32
        })
        .collect();
    let threshold = silence_threshold(&energies);

    // Midpoints (in samples) of silence runs >= MIN_SILENCE_FRAMES: the candidate
    // cut points.
    let mut cuts: Vec<usize> = Vec::new();
    let mut run_start: Option<usize> = None;
    for (i, &energy) in energies.iter().enumerate() {
        let silent = energy <= threshold;
        match (silent, run_start) {
            (true, None) => run_start = Some(i),
            (false, Some(rs)) => {
                if i - rs >= MIN_SILENCE_FRAMES {
                    cuts.push(((rs + i) / 2) * FRAME_SIZE);
                }
                run_start = None;
            }
            _ => {}
        }
    }

    // Greedily build chunks: from `start`, take the latest pause within
    // [start+MIN, start+MAX]; if none, hard-cut at start+MAX.
    let mut ranges = Vec::new();
    let mut start = 0usize;
    while total - start > MAX_CHUNK {
        let limit = start + MAX_CHUNK;
        let cut = cuts
            .iter()
            .copied()
            .filter(|&c| c > start + MIN_CHUNK && c <= limit)
            .max_by_key(|&c| c)
            .unwrap_or(limit);
        ranges.push((start, cut));
        start = cut;
    }
    ranges.push((start, total));
    ranges
}

/// Derives the silence threshold from the recording's own quiet end, so a quiet
/// recording gets a quiet threshold.
///
/// Takes the 20th percentile of frame energy (inside the pauses of any real
/// speech), lifts it slightly and clamps it into `[MIN, MAX]`.
fn silence_threshold(energies: &[f32]) -> f32 {
    if energies.is_empty() {
        return MAX_SILENCE_ENERGY;
    }
    let mut sorted: Vec<f32> = energies.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p20 = sorted[sorted.len() / 5];
    (p20 * 3.0).clamp(MIN_SILENCE_ENERGY, MAX_SILENCE_ENERGY)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds frame energies: `quiet` frames of near-silence, `loud` frames of speech.
    fn energies(quiet: usize, loud: usize, quiet_level: f32, loud_level: f32) -> Vec<f32> {
        let mut v = vec![quiet_level; quiet];
        v.extend(std::iter::repeat(loud_level).take(loud));
        v
    }

    #[test]
    fn loud_recording_keeps_the_old_threshold() {
        // Speech well above −30 dBFS with a normal noise floor: the cap applies.
        let e = energies(300, 700, 0.0005, 0.02);
        assert_eq!(silence_threshold(&e), MAX_SILENCE_ENERGY);
    }

    #[test]
    fn quiet_recording_gets_a_quiet_threshold() {
        // Speech at ~-40 dBFS: a fixed 0.001 would call the entire file silent,
        // the adaptive threshold lands between noise floor and speech.
        let e = energies(300, 700, 1e-6, 1e-4);
        let t = silence_threshold(&e);
        assert!(t < 1e-4, "threshold {t} must stay below the speech level");
        assert!(t > 1e-6, "threshold {t} must stay above the noise floor");
    }

    #[test]
    fn digital_silence_stays_above_zero() {
        assert!(silence_threshold(&[0.0; 100]) >= MIN_SILENCE_ENERGY);
        assert_eq!(silence_threshold(&[]), MAX_SILENCE_ENERGY);
    }

    #[test]
    fn split_covers_the_whole_buffer_without_gaps() {
        // 60 s of alternating speech and pauses at a quiet level.
        let mut samples = Vec::new();
        for block in 0..60 {
            let level = if block % 5 == 0 { 0.0 } else { 0.01 };
            samples.extend(std::iter::repeat(level).take(16_000));
        }
        let ranges = split_on_silence(&samples);
        assert!(ranges.len() > 1, "a 60 s buffer must be split");
        assert_eq!(ranges[0].0, 0);
        assert_eq!(ranges.last().unwrap().1, samples.len());
        for pair in ranges.windows(2) {
            assert_eq!(pair[0].1, pair[1].0, "ranges must be contiguous");
        }
    }
}
