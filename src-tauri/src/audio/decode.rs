// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::error::{AppError, AppResult};

pub struct PcmData {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Decodes without progress reporting (CLI and tests).
pub fn decode_to_pcm(path: &Path, cancel: &AtomicBool) -> AppResult<PcmData> {
    decode_to_pcm_with_progress(path, cancel, &mut |_, _, _| {})
}

/// Same, but reports `(decoded_frames, total_frames_if_known, source_rate)` as it
/// goes. Decoding scales with file length and emits nothing of its own, so the UI
/// needs the heartbeat to tell a slow decode from a hang.
pub fn decode_to_pcm_with_progress(
    path: &Path,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u64, Option<u64>, u32),
) -> AppResult<PcmData> {
    let file = std::fs::File::open(path)
        .map_err(|_| AppError::FileNotFound(path.display().to_string()))?;

    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let meta_opts = MetadataOptions::default();
    let fmt_opts = FormatOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(|e| AppError::AudioDecode(e.to_string()))?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or_else(|| AppError::AudioDecode("No audio track found".into()))?;

    let track_id = track.id;
    // The container-declared rate/layout is only a starting guess: HE-AAC (SBR)
    // in mp4 declares e.g. 24 kHz while the decoder outputs 48 kHz, and some
    // containers omit the fields entirely. Resampling from the declared rate would
    // stretch or squeeze the audio. The authoritative values come from the first
    // decoded buffer's spec, below.
    let mut sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let mut channels = track
        .codec_params
        .channels
        .map(|c| c.count() as u16)
        .unwrap_or(1);

    // Declared length (in source frames), when the container states it.
    let total_frames = track.codec_params.n_frames;

    let dec_opts = DecoderOptions::default();
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .map_err(|e| AppError::AudioDecode(e.to_string()))?;

    let mut all_samples: Vec<f32> = Vec::new();
    let mut spec_known = false;
    // A reader that keeps asking for a reset without ever yielding a packet would
    // spin this loop forever, so give up after a run of resets with no progress
    // in between.
    const MAX_CONSECUTIVE_RESETS: u32 = 64;
    let mut resets: u32 = 0;
    let mut last_report = std::time::Instant::now();

    loop {
        // Checked per packet so a long decode aborts promptly on cancel.
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }

        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::ResetRequired) => {
                resets += 1;
                if resets > MAX_CONSECUTIVE_RESETS {
                    return Err(AppError::AudioDecode(
                        "decoder kept requesting a reset without producing audio".into(),
                    ));
                }
                decoder.reset();
                continue;
            }
            Err(symphonia::core::errors::Error::IoError(_)) => break,
            Err(e) => return Err(AppError::AudioDecode(e.to_string())),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = decoder
            .decode(&packet)
            .map_err(|e| AppError::AudioDecode(e.to_string()))?;

        // The decoder's own spec wins over the container header (see above).
        if !spec_known {
            let spec = decoded.spec();
            sample_rate = spec.rate;
            channels = spec.channels.count().max(1) as u16;
            spec_known = true;
        }

        collect_samples(&decoded, channels, &mut all_samples);
        resets = 0;

        // Throttled: the UI needs a heartbeat, not every packet.
        if last_report.elapsed() >= std::time::Duration::from_millis(400) {
            last_report = std::time::Instant::now();
            on_progress(all_samples.len() as u64, total_frames, sample_rate);
        }
    }
    on_progress(all_samples.len() as u64, total_frames, sample_rate);

    Ok(PcmData {
        samples: all_samples,
        sample_rate,
        channels,
    })
}

fn collect_samples(buf: &AudioBufferRef<'_>, channels: u16, out: &mut Vec<f32>) {
    // A mid-stream spec change would make `chan(ch)` panic on a narrower buffer,
    // so clamp to what this buffer carries.
    let channels = channels
        .min(buf.spec().channels.count().max(1) as u16)
        .max(1);
    match buf {
        AudioBufferRef::F32(b) => {
            let frames = b.frames();
            for i in 0..frames {
                let mut sum = 0.0f32;
                for ch in 0..channels as usize {
                    sum += b.chan(ch)[i];
                }
                out.push(sum / channels as f32);
            }
        }
        AudioBufferRef::S16(b) => {
            let frames = b.frames();
            for i in 0..frames {
                let mut sum = 0.0f32;
                for ch in 0..channels as usize {
                    sum += b.chan(ch)[i] as f32 / i16::MAX as f32;
                }
                out.push(sum / channels as f32);
            }
        }
        AudioBufferRef::S32(b) => {
            let frames = b.frames();
            for i in 0..frames {
                let mut sum = 0.0f32;
                for ch in 0..channels as usize {
                    sum += b.chan(ch)[i] as f32 / i32::MAX as f32;
                }
                out.push(sum / channels as f32);
            }
        }
        AudioBufferRef::U8(b) => {
            let frames = b.frames();
            for i in 0..frames {
                let mut sum = 0.0f32;
                for ch in 0..channels as usize {
                    sum += (b.chan(ch)[i] as f32 - 128.0) / 128.0;
                }
                out.push(sum / channels as f32);
            }
        }
        _ => {
            // convert less common formats to f32 via symphonia's built-in converter
            let mut tmp: symphonia::core::audio::AudioBuffer<f32> =
                symphonia::core::audio::AudioBuffer::new(buf.frames() as u64, *buf.spec());
            buf.convert(&mut tmp);
            let frames = tmp.frames();
            for i in 0..frames {
                let mut sum = 0.0f32;
                for ch in 0..channels as usize {
                    sum += tmp.chan(ch)[i];
                }
                out.push(sum / channels as f32);
            }
        }
    }
}
