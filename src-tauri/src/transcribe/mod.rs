// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

pub mod hallucination;
pub mod speech;
pub mod whisper;
pub use speech::speech_chunks;
pub use whisper::{
    ensure_vad_model, load_context, transcribe, transcribe_auto, transcribe_chunks, transcribe_multilang,
    transcribe_with, Segment, TranscribeOptions,
};
pub use whisper_rs::WhisperContext;
