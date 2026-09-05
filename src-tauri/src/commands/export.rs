// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

use docx_rs::*;

use crate::error::{AppError, AppResult};
use crate::transcribe::Segment;

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Txt,
    Srt,
    Vtt,
    Json,
    Md,
    Docx,
}

#[tauri::command]
pub fn cmd_export_transcript(
    segments: Vec<Segment>,
    format: ExportFormat,
    output_path: String,
) -> Result<(), AppError> {
    match format {
        ExportFormat::Txt => std::fs::write(&output_path, to_txt(&segments))?,
        ExportFormat::Srt => std::fs::write(&output_path, to_srt(&segments))?,
        ExportFormat::Vtt => std::fs::write(&output_path, to_vtt(&segments))?,
        ExportFormat::Json => std::fs::write(&output_path, to_json(&segments)?)?,
        ExportFormat::Md => std::fs::write(&output_path, to_md(&segments))?,
        ExportFormat::Docx => write_docx(&segments, &output_path)?,
    }
    Ok(())
}

#[tauri::command]
pub fn cmd_export_summary(
    summary: String,
    format: ExportFormat,
    output_path: String,
) -> Result<(), AppError> {
    match format {
        // The summary is Markdown prose; txt/md write it as-is.
        ExportFormat::Docx => write_summary_docx(&summary, &output_path)?,
        _ => std::fs::write(&output_path, summary)?,
    }
    Ok(())
}

// ---------------------------------------------------------------------------

fn to_txt(segs: &[Segment]) -> String {
    segs.iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

fn to_srt(segs: &[Segment]) -> String {
    segs.iter()
        .enumerate()
        .map(|(i, s)| {
            format!(
                "{}\n{} --> {}\n{}\n",
                i + 1,
                ms_to_srt(s.t0),
                ms_to_srt(s.t1),
                s.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn to_vtt(segs: &[Segment]) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for (i, s) in segs.iter().enumerate() {
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            ms_to_vtt(s.t0),
            ms_to_vtt(s.t1),
            s.text
        ));
    }
    out
}

fn to_json(segs: &[Segment]) -> AppResult<String> {
    serde_json::to_string_pretty(segs).map_err(|e| AppError::Other(e.to_string()))
}

fn to_md(segs: &[Segment]) -> String {
    let mut out = String::from("# Transcript\n\n");
    for s in segs {
        out.push_str(&format!("**[{}]** {}\n\n", ms_to_srt(s.t0), s.text));
    }
    out
}

/// Builds a real .docx: a bold title, then one paragraph per segment with the
/// timecode in bold followed by the text. Writes straight to the chosen path.
fn write_docx(segs: &[Segment], path: &str) -> AppResult<()> {
    let mut docx = Docx::new()
        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("Transcript").bold().size(32)));
    for s in segs {
        docx = docx.add_paragraph(
            Paragraph::new()
                .add_run(
                    Run::new()
                        .add_text(format!("[{}] ", ms_to_srt(s.t0)))
                        .bold(),
                )
                .add_run(Run::new().add_text(s.text.as_str())),
        );
    }
    let file = std::fs::File::create(path)?;
    docx.build()
        .pack(file)
        .map_err(|e| AppError::Other(e.to_string()))?;
    Ok(())
}

/// Builds a .docx from a plain/Markdown summary: one paragraph per line (blank
/// lines become empty paragraphs, preserving the spacing between points).
fn write_summary_docx(summary: &str, path: &str) -> AppResult<()> {
    let mut docx = Docx::new()
        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("Summary").bold().size(32)));
    for line in summary.lines() {
        docx = docx.add_paragraph(Paragraph::new().add_run(Run::new().add_text(line)));
    }
    let file = std::fs::File::create(path)?;
    docx.build()
        .pack(file)
        .map_err(|e| AppError::Other(e.to_string()))?;
    Ok(())
}

fn ms_to_srt(ms: i64) -> String {
    let h = ms / 3_600_000;
    let m = (ms % 3_600_000) / 60_000;
    let s = (ms % 60_000) / 1_000;
    let millis = ms % 1_000;
    format!("{h:02}:{m:02}:{s:02},{millis:03}")
}

fn ms_to_vtt(ms: i64) -> String {
    let h = ms / 3_600_000;
    let m = (ms % 3_600_000) / 60_000;
    let s = (ms % 60_000) / 1_000;
    let millis = ms % 1_000;
    format!("{h:02}:{m:02}:{s:02}.{millis:03}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Segment> {
        vec![
            Segment {
                t0: 0,
                t1: 1_500,
                text: "Hello world".into(),
            },
            // 1h 2m 3s 456ms: exercises padding and the hours field. The text is
            // multi-byte so the writers are exercised on more than ASCII.
            Segment {
                t0: 3_723_456,
                t1: 3_725_000,
                text: "Grüße, Welt".into(),
            },
        ]
    }

    // Unique temp path so parallel tests don't collide; removed by the caller.
    fn temp_path(ext: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let id = N.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "voodooscribe_test_{}_{id}.{ext}",
            std::process::id()
        ))
    }

    #[test]
    fn timecodes_format_with_padding_and_hours() {
        // SRT uses a comma before millis, VTT a dot.
        assert_eq!(ms_to_srt(0), "00:00:00,000");
        assert_eq!(ms_to_vtt(0), "00:00:00.000");
        assert_eq!(ms_to_srt(3_723_456), "01:02:03,456");
        assert_eq!(ms_to_vtt(3_723_456), "01:02:03.456");
    }

    #[test]
    fn txt_joins_text_only() {
        assert_eq!(to_txt(&sample()), "Hello world Grüße, Welt");
    }

    #[test]
    fn srt_has_index_arrow_and_text() {
        let out = to_srt(&sample());
        assert!(out.starts_with("1\n00:00:00,000 --> 00:00:01,500\nHello world"));
        assert!(out.contains("2\n01:02:03,456 --> 01:02:05,000\nGrüße, Welt"));
        assert!(out.contains(" --> "));
    }

    #[test]
    fn vtt_starts_with_header() {
        let out = to_vtt(&sample());
        assert!(out.starts_with("WEBVTT\n\n"));
        assert!(out.contains("00:00:00.000 --> 00:00:01.500"));
    }

    #[test]
    fn json_round_trips_to_segments() {
        let json = to_json(&sample()).unwrap();
        let back: Vec<Segment> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].text, "Hello world");
        assert_eq!(back[1].t0, 3_723_456);
    }

    #[test]
    fn md_has_title_and_bold_timecode() {
        let out = to_md(&sample());
        assert!(out.starts_with("# Transcript\n\n"));
        assert!(out.contains("**[00:00:00,000]** Hello world"));
    }

    // .docx is a ZIP package: the file must be non-empty and start with the ZIP
    // magic "PK".
    fn assert_valid_docx(path: &std::path::Path) {
        let bytes = std::fs::read(path).unwrap();
        assert!(
            bytes.len() > 100,
            "docx unexpectedly tiny: {} bytes",
            bytes.len()
        );
        assert_eq!(&bytes[..2], b"PK", "not a ZIP/docx file");
    }

    #[test]
    fn transcript_docx_is_valid_zip() {
        let path = temp_path("docx");
        write_docx(&sample(), path.to_str().unwrap()).unwrap();
        assert_valid_docx(&path);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn summary_docx_is_valid_zip() {
        let path = temp_path("docx");
        write_summary_docx("Line one\n\nLine two", path.to_str().unwrap()).unwrap();
        assert_valid_docx(&path);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn cmd_export_transcript_writes_every_format() {
        for (fmt, ext) in [
            (ExportFormat::Txt, "txt"),
            (ExportFormat::Srt, "srt"),
            (ExportFormat::Vtt, "vtt"),
            (ExportFormat::Json, "json"),
            (ExportFormat::Md, "md"),
            (ExportFormat::Docx, "docx"),
        ] {
            let path = temp_path(ext);
            let p = path.to_str().unwrap().to_string();
            cmd_export_transcript(sample(), fmt, p).unwrap();
            assert!(
                std::fs::metadata(&path).unwrap().len() > 0,
                ".{ext} is empty"
            );
            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn cmd_export_summary_writes_text_and_docx() {
        for (fmt, ext) in [
            (ExportFormat::Md, "md"),
            (ExportFormat::Txt, "txt"),
            (ExportFormat::Docx, "docx"),
        ] {
            let path = temp_path(ext);
            let p = path.to_str().unwrap().to_string();
            cmd_export_summary("## Summary\n\nIt is about cats.".into(), fmt, p).unwrap();
            assert!(
                std::fs::metadata(&path).unwrap().len() > 0,
                ".{ext} is empty"
            );
            let _ = std::fs::remove_file(&path);
        }
    }
}
