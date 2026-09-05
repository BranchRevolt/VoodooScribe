// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

use tauri::AppHandle;
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

use crate::error::{AppError, AppResult};

/// Decodes any container/codec ffmpeg understands straight into the format
/// whisper wants: 16 kHz, mono, f32. ffmpeg does the resample, so the result needs
/// no further processing. Fallback for anything symphonia can't handle natively
/// (Opus, AC-3, DTS, AMR, AVI, …).
///
/// stdout must be read as raw bytes: the shell plugin's default line mode and
/// `.output()` split on `\n`/`\r` and re-insert `\n`, corrupting binary PCM. Hence
/// `set_raw_out(true)` plus `spawn()` and manual chunk collection.
pub async fn decode_16k_mono(
    app: &AppHandle,
    file_path: &str,
    on_progress: &mut (dyn FnMut(u64) + Send),
) -> AppResult<Vec<f32>> {
    // sidecar() takes the base name; Tauri resolves it to binaries/ffmpeg-<triple>.
    let command = app
        .shell()
        .sidecar("ffmpeg")
        .map_err(|e| AppError::AudioDecode(format!("ffmpeg sidecar unavailable: {e}")))?
        .args([
            "-hide_banner",
            "-loglevel", "error",
            "-nostdin",
            "-i", file_path,
            "-vn",            // ignore any video stream
            "-ac", "1",       // mono
            "-ar", "16000",   // 16 kHz
            "-f", "f32le",    // raw 32-bit float little-endian
            "-",              // write to stdout
        ])
        .set_raw_out(true);

    let (mut rx, _child) = command
        .spawn()
        .map_err(|e| AppError::AudioDecode(format!("ffmpeg failed to run: {e}")))?;

    let mut pcm: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();
    let mut code: Option<i32> = None;
    let mut last_report = std::time::Instant::now();

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => {
                pcm.extend(bytes);
                // Heartbeat for the UI. 4 bytes per f32 sample at 16 kHz.
                if last_report.elapsed() >= std::time::Duration::from_millis(400) {
                    last_report = std::time::Instant::now();
                    on_progress(pcm.len() as u64 / 4);
                }
            }
            CommandEvent::Stderr(bytes) => stderr.extend(bytes),
            CommandEvent::Terminated(payload) => code = payload.code,
            CommandEvent::Error(e) => {
                return Err(AppError::AudioDecode(format!("ffmpeg error: {e}")))
            }
            _ => {}
        }
    }

    if code != Some(0) {
        let msg = String::from_utf8_lossy(&stderr);
        let last = msg.lines().last().unwrap_or("unknown ffmpeg error");
        return Err(AppError::AudioDecode(format!("ffmpeg (exit {code:?}): {last}")));
    }

    if pcm.is_empty() {
        return Err(AppError::AudioDecode("ffmpeg produced no audio".into()));
    }

    let samples = pcm
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    Ok(samples)
}
