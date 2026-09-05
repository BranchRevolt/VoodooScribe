// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;

use crate::error::{AppError, AppResult};

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    filename: String,
    downloaded: u64,
    total: u64,
    percent: u8,
    phase: &'static str,
    /// Smoothed transfer rate; 0 while unknown or during the SHA-256 pass.
    bytes_per_sec: u64,
    /// Seconds left at that rate; 0 when the total size or the rate is unknown.
    eta_secs: u64,
}

/// Path of the small config file that records a user-chosen models directory.
fn models_dir_config(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Other(e.to_string()))?;
    Ok(dir.join("models_dir.txt"))
}

/// The default models directory (`app_data_dir/models`), ignoring any override.
fn default_models_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Other(e.to_string()))?
        .join("models");
    Ok(path)
}

/// Where models live: the user override if one is set and still points at an
/// existing directory, otherwise the default app_data location.
pub fn models_dir(app: &AppHandle) -> AppResult<PathBuf> {
    if let Ok(cfg) = models_dir_config(app) {
        if let Ok(raw) = std::fs::read_to_string(&cfg) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                let p = PathBuf::from(trimmed);
                if p.is_dir() {
                    return Ok(p);
                }
            }
        }
    }
    default_models_dir(app)
}

/// Persist a custom models directory. Creates the target dir and the config dir.
/// Pass an empty string to clear the override (back to the default location).
pub fn set_models_dir(app: &AppHandle, path: &str) -> AppResult<()> {
    let cfg = models_dir_config(app)?;
    if let Some(parent) = cfg.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let trimmed = path.trim();
    if trimmed.is_empty() {
        if cfg.exists() {
            std::fs::remove_file(&cfg)?;
        }
        return Ok(());
    }
    std::fs::create_dir_all(trimmed)?;
    std::fs::write(&cfg, trimmed)?;
    Ok(())
}

pub async fn download_model(
    app: AppHandle,
    url: &str,
    dest_dir: &Path,
    filename: &str,
    expected_sha256: Option<&str>,
    cancel: Arc<AtomicBool>,
) -> AppResult<PathBuf> {
    let dest = dest_dir.join(filename);
    let tmp  = dest.with_extension("tmp");

    if dest.exists() {
        if let Some(hash) = expected_sha256 {
            if verify_sha256(&dest, hash, filename, &app, &cancel).await? {
                return Ok(dest);
            }
            tracing::warn!("SHA-256 mismatch for {filename}, re-downloading");
            tokio::fs::remove_file(&dest).await?;
        } else {
            return Ok(dest);
        }
    }

    tokio::fs::create_dir_all(dest_dir).await?;

    let resume_from = if tmp.exists() {
        tokio::fs::metadata(&tmp).await?.len()
    } else {
        0
    };

    let client = reqwest::Client::builder()
        .user_agent("voodooscribe/0.1")
        .build()
        .map_err(|e| AppError::ModelDownload(e.to_string()))?;

    let mut req = client.get(url);
    if resume_from > 0 {
        req = req.header("Range", format!("bytes={resume_from}-"));
    }

    let resp = req
        .send()
        .await
        .map_err(|e| AppError::ModelDownload(e.to_string()))?;

    if !resp.status().is_success() && resp.status().as_u16() != 206 {
        return Err(AppError::ModelDownload(format!(
            "HTTP {}: {url}",
            resp.status()
        )));
    }

    let server_supports_resume = resp.status().as_u16() == 206;
    let total = if server_supports_resume {
        parse_total_from_content_range(
            resp.headers()
                .get("content-range")
                .and_then(|v| v.to_str().ok())
                .unwrap_or(""),
        )
        .unwrap_or(0)
    } else {
        resp.content_length().unwrap_or(0)
    };

    tracing::info!("[dl] {filename}: resume_from={resume_from} total={total} status={}", resp.status().as_u16());

    let (mut file, mut downloaded) = if server_supports_resume && resume_from > 0 {
        let f = tokio::fs::OpenOptions::new()
            .append(true)
            .open(&tmp)
            .await?;
        (f, resume_from)
    } else {
        let f = tokio::fs::File::create(&tmp).await?;
        (f, 0u64)
    };

    let mut stream = resp.bytes_stream();
    let mut last_emitted_pct: u8 = 255;
    let mut last_emitted_bytes: u64 = 0;
    const EMIT_EVERY_BYTES: u64 = 4 * 1024 * 1024;
    // Percent steps are too coarse for a speed readout: 1% of a 9 GB model is
    // 90 MB, about ten seconds without an update. Hence the timer tick as well.
    const EMIT_EVERY: Duration = Duration::from_millis(400);
    // Raw per-window rates jump with every TCP hiccup, so the figure is smoothed.
    const SPEED_SMOOTHING: f64 = 0.3;

    let mut window_start = Instant::now();
    let mut window_bytes: u64 = 0;
    let mut speed: f64 = 0.0;

    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            drop(file);
            tracing::info!("[dl] {filename}: cancelled");
            return Err(AppError::Cancelled);
        }

        let chunk = chunk.map_err(|e| AppError::ModelDownload(e.to_string()))?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        window_bytes += chunk.len() as u64;

        let percent = if total > 0 {
            ((downloaded * 100) / total).min(100) as u8
        } else {
            0
        };

        // The rate is measured over a full window only, never over the short gap
        // left by a percent-change tick.
        let elapsed = window_start.elapsed();
        let window_due = elapsed >= EMIT_EVERY;
        if window_due {
            let rate = window_bytes as f64 / elapsed.as_secs_f64();
            speed = if speed == 0.0 {
                rate
            } else {
                speed + (rate - speed) * SPEED_SMOOTHING
            };
            window_start = Instant::now();
            window_bytes = 0;
        }

        let should_emit = window_due
            || if total > 0 {
                percent != last_emitted_pct
            } else {
                downloaded.saturating_sub(last_emitted_bytes) >= EMIT_EVERY_BYTES
            };

        if should_emit {
            last_emitted_pct = percent;
            last_emitted_bytes = downloaded;
            let _ = app.emit(
                "model://download-progress",
                DownloadProgress {
                    filename: filename.to_string(),
                    downloaded,
                    total,
                    percent,
                    phase: "downloading",
                    bytes_per_sec: speed as u64,
                    eta_secs: if total > downloaded && speed > 1.0 {
                        ((total - downloaded) as f64 / speed) as u64
                    } else {
                        0
                    },
                },
            );
        }

        // Exit once all expected bytes have arrived, without waiting for the
        // server to close the connection: some CDNs keep the stream open.
        if total > 0 && downloaded >= total {
            tracing::info!("[dl] {filename}: got all {downloaded} bytes, exiting loop early");
            break;
        }
    }

    tracing::info!("[dl] {filename}: loop exited, cancel={}", cancel.load(Ordering::Relaxed));

    if cancel.load(Ordering::Relaxed) {
        drop(file);
        tracing::info!("[dl] {filename}: returning Cancelled");
        return Err(AppError::Cancelled);
    }

    tracing::info!("[dl] {filename}: flushing file...");
    file.flush().await?;
    drop(file);
    tracing::info!("[dl] {filename}: flush done, renaming {tmp:?} -> {dest:?}");

    // A stream can end early without erroring (a proxy closing the connection, a
    // CDN truncating a range). Renaming what arrived would install a model that
    // only fails later inside ggml, so the length is checked first. The `.tmp` is
    // kept so the next attempt resumes instead of starting over.
    if total > 0 && downloaded < total {
        return Err(AppError::ModelDownload(format!(
            "{filename}: received {downloaded} of {total} bytes, download is incomplete"
        )));
    }

    if let Some(hash) = expected_sha256 {
        let file_size = tokio::fs::metadata(&tmp).await?.len();
        let _ = app.emit(
            "model://download-progress",
            DownloadProgress {
                filename: filename.to_string(),
                downloaded: 0,
                total: file_size,
                percent: 0,
                phase: "verifying",
                bytes_per_sec: 0,
                eta_secs: 0,
            },
        );
        if !verify_sha256(&tmp, hash, filename, &app, &cancel).await? {
            tokio::fs::remove_file(&tmp).await.ok();
            return Err(AppError::ModelDownload(format!(
                "SHA-256 mismatch for {filename}: please re-download"
            )));
        }
    }

    tokio::fs::rename(&tmp, &dest).await?;
    tracing::info!("[dl] {filename}: rename done, returning Ok");
    Ok(dest)
}

// ---------------------------------------------------------------------------

fn parse_total_from_content_range(header: &str) -> Option<u64> {
    header
        .split('/')
        .nth(1)
        .and_then(|s| s.trim().parse::<u64>().ok())
}

async fn verify_sha256(
    path: &Path,
    expected_hex: &str,
    filename: &str,
    app: &AppHandle,
    cancel: &Arc<AtomicBool>,
) -> AppResult<bool> {
    use tokio::io::AsyncReadExt;

    let file_size = tokio::fs::metadata(path).await?.len();
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 65_536];
    let mut read: u64 = 0;
    let mut last_pct: u8 = 255;

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        read += n as u64;

        if file_size > 0 {
            let pct = ((read * 100) / file_size).min(100) as u8;
            if pct != last_pct {
                last_pct = pct;
                let _ = app.emit(
                    "model://download-progress",
                    DownloadProgress {
                        filename: filename.to_string(),
                        downloaded: read,
                        total: file_size,
                        percent: pct,
                        phase: "verifying",
                        bytes_per_sec: 0,
                        eta_secs: 0,
                    },
                );
            }
        }
    }

    let digest = hex::encode(hasher.finalize());
    Ok(digest.eq_ignore_ascii_case(expected_hex))
}
