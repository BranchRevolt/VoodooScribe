// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

use rubato::{FftFixedIn, Resampler};
use crate::error::{AppError, AppResult};

const TARGET_RATE: u32 = 16_000;

pub fn resample_to_16k(samples: Vec<f32>, src_rate: u32) -> AppResult<Vec<f32>> {
    if src_rate == TARGET_RATE {
        return Ok(samples);
    }

    let chunk_size = 1024;
    let mut resampler = FftFixedIn::<f32>::new(
        src_rate as usize,
        TARGET_RATE as usize,
        chunk_size,
        2,
        1,
    )
    .map_err(|e| AppError::Resample(e.to_string()))?;

    let mut output = Vec::with_capacity(
        (samples.len() as f64 * TARGET_RATE as f64 / src_rate as f64) as usize + 1024,
    );

    let mut pos = 0usize;
    while pos + chunk_size <= samples.len() {
        let chunk = vec![samples[pos..pos + chunk_size].to_vec()];
        let out = resampler
            .process(&chunk, None)
            .map_err(|e| AppError::Resample(e.to_string()))?;
        output.extend_from_slice(&out[0]);
        pos += chunk_size;
    }

    // handle the trailing partial chunk
    if pos < samples.len() {
        let mut tail = samples[pos..].to_vec();
        tail.resize(chunk_size, 0.0);
        let chunk = vec![tail];
        let out = resampler
            .process(&chunk, None)
            .map_err(|e| AppError::Resample(e.to_string()))?;
        let valid = ((samples.len() - pos) as f64 * TARGET_RATE as f64 / src_rate as f64) as usize;
        output.extend_from_slice(&out[0][..valid.min(out[0].len())]);
    }

    Ok(output)
}
