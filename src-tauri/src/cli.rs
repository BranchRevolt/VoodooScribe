// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::audio::{decode_to_pcm, resample_to_16k};
use crate::error::AppError;
use crate::transcribe::{ensure_vad_model, transcribe, TranscribeOptions};

pub fn run_transcribe(args: &[String]) -> Result<(), AppError> {
    let mut file: Option<String> = None;
    let mut model: Option<String> = None;
    let mut lang: Option<String> = None;
    let mut use_vad = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--model" | "-m" => {
                i += 1;
                if i < args.len() {
                    model = Some(args[i].clone());
                }
            }
            "--lang" | "-l" => {
                i += 1;
                if i < args.len() {
                    lang = Some(args[i].clone());
                }
            }
            "--vad" => use_vad = true,
            arg if !arg.starts_with('-') && file.is_none() => {
                file = Some(arg.to_string());
            }
            other => eprintln!("warning: unknown argument '{other}', ignoring"),
        }
        i += 1;
    }

    let file = file.ok_or_else(|| {
        AppError::Other(
            "Usage: transcribe <file> [--model path/to/ggml-*.bin] [--lang en] [--vad]".into(),
        )
    })?;

    let model_path = model.ok_or_else(|| {
        AppError::Other(
            "No model specified. Pass --model path/to/ggml-*.bin\n\
             Download from: https://huggingface.co/ggerganov/whisper.cpp"
                .into(),
        )
    })?;

    // --- 1/3  decode -------------------------------------------------------
    eprint!("[1/3] Decoding {}... ", file);
    let pcm = decode_to_pcm(Path::new(&file), &AtomicBool::new(false))?;
    let duration_s = pcm.samples.len() as f64 / pcm.sample_rate as f64;
    eprintln!(
        "ok — {:.2}s, {} Hz, {} ch → mono f32",
        duration_s, pcm.sample_rate, pcm.channels
    );

    // --- 2/3  resample -----------------------------------------------------
    eprint!("[2/3] Resampling to 16 kHz... ");
    let samples = resample_to_16k(pcm.samples, pcm.sample_rate)?;
    eprintln!("ok — {} samples", samples.len());

    // --vad enables whisper.cpp's built-in Silero VAD (handled inside transcription).
    let vad_model_path = if use_vad {
        let p = ensure_vad_model(&std::env::temp_dir())?;
        eprintln!("      VAD enabled (Silero, built-in)");
        Some(p)
    } else {
        None
    };

    // --- 3/3  transcribe ---------------------------------------------------
    let model_name = Path::new(&model_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    eprintln!("[3/3] Transcribing with {}...", model_name);

    let cancel = Arc::new(AtomicBool::new(false));
    let opts = TranscribeOptions {
        language: lang,
        vad_model_path,
        ..Default::default()
    };

    let segments = transcribe(Path::new(&model_path), &samples, &opts, cancel, |pct| {
        eprint!("\r      progress: {pct:3}%  ");
    })?;
    eprintln!("\r      done                ");
    println!();

    for seg in &segments {
        println!(
            "[{} --> {}]  {}",
            fmt_ms(seg.t0),
            fmt_ms(seg.t1),
            seg.text
        );
    }

    eprintln!("\n{} segment(s) written to stdout.", segments.len());
    Ok(())
}

fn fmt_ms(ms: i64) -> String {
    let h = ms / 3_600_000;
    let m = (ms % 3_600_000) / 60_000;
    let s = (ms % 60_000) / 1_000;
    let frac = ms % 1_000;
    if h > 0 {
        format!("{h:02}:{m:02}:{s:02}.{frac:03}")
    } else {
        format!("{m:02}:{s:02}.{frac:03}")
    }
}
