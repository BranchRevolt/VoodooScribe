// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Audio decode error: {0}")]
    AudioDecode(String),

    #[error("Resample error: {0}")]
    Resample(String),

    #[error("Whisper model is not loaded")]
    WhisperModelNotLoaded,

    #[error("Transcription error: {0}")]
    Transcription(String),

    #[error("LLM model is not loaded")]
    LlmModelNotLoaded,

    #[error("Summarization error: {0}")]
    Summarization(String),

    /// A summarize/polish is already running. The llama backend and the cached
    /// model are process-wide, so only one operation can use them at a time.
    #[error("Another summarization is already running")]
    LlmBusy,

    #[error("Model download error: {0}")]
    ModelDownload(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Not enough GPU memory: need ~{required_mb} MB, {free_mb} MB free")]
    InsufficientMemory { required_mb: u32, free_mb: u32 },

    #[error("Operation cancelled")]
    Cancelled,

    #[error("{0}")]
    Other(String),
}

impl AppError {
    /// Stable identifier the frontend maps to a localized message. I/O errors are
    /// classified further (disk full / permission denied / missing file) so each
    /// gets its own message.
    pub fn code(&self) -> &'static str {
        match self {
            AppError::AudioDecode(_) => "audio_decode",
            AppError::Resample(_) => "resample",
            AppError::WhisperModelNotLoaded => "whisper_not_loaded",
            AppError::Transcription(_) => "transcription",
            AppError::LlmModelNotLoaded => "llm_not_loaded",
            AppError::Summarization(_) => "summarization",
            AppError::LlmBusy => "llm_busy",
            AppError::ModelDownload(_) => "model_download",
            AppError::FileNotFound(_) => "file_not_found",
            AppError::Io(e) => io_code(e),
            AppError::InsufficientMemory { .. } => "insufficient_memory",
            AppError::Cancelled => "cancelled",
            AppError::Other(_) => "other",
        }
    }

    /// Dynamic part of the error (a filename, the underlying message, …) for the
    /// frontend to interpolate into its localized template. None when the message
    /// is fully static.
    pub fn detail(&self) -> Option<String> {
        match self {
            AppError::AudioDecode(s)
            | AppError::Resample(s)
            | AppError::Transcription(s)
            | AppError::Summarization(s)
            | AppError::ModelDownload(s)
            | AppError::FileNotFound(s)
            | AppError::Other(s) => Some(s.clone()),
            AppError::Io(e) => Some(e.to_string()),
            AppError::InsufficientMemory { required_mb, free_mb } => Some(format!(
                "need ~{:.1} GB, free {:.1} GB",
                *required_mb as f32 / 1024.0,
                *free_mb as f32 / 1024.0
            )),
            AppError::WhisperModelNotLoaded
            | AppError::LlmModelNotLoaded
            | AppError::LlmBusy
            | AppError::Cancelled => None,
        }
    }
}

/// Maps a std::io::Error to a specific code, so disk-full / permission / missing
/// file each get their own message instead of a generic "I/O error".
fn io_code(e: &std::io::Error) -> &'static str {
    use std::io::ErrorKind;
    match e.kind() {
        ErrorKind::NotFound => "file_not_found",
        ErrorKind::PermissionDenied => "permission_denied",
        _ => match e.raw_os_error() {
            Some(28) => "disk_full",  // ENOSPC (Linux/macOS)
            Some(112) => "disk_full", // ERROR_DISK_FULL (Windows)
            _ => "io",
        },
    }
}

/// Serialized to the frontend as `{ code, message, detail }`. `code` drives
/// localization; `message` is the English fallback if no translation exists;
/// `detail` carries the dynamic part for interpolation.
#[derive(Serialize)]
struct ErrorPayload {
    code: &'static str,
    message: String,
    detail: Option<String>,
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ErrorPayload {
            code: self.code(),
            message: self.to_string(),
            detail: self.detail(),
        }
        .serialize(serializer)
    }
}

pub type AppResult<T> = Result<T, AppError>;
