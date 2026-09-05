# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] — unreleased

First public release. Development ran from June to August 2026; this entry summarises the state at
release rather than every step along the way.

### Added

**Transcription**
- Offline transcription with whisper.cpp, GPU-accelerated through Vulkan (Linux/Windows) and Metal (macOS)
- Eight Whisper builds, from Tiny to full Large v3, downloaded on demand, with recommendations based on detected VRAM
- Models tab driven by a dropdown per model family rather than a flat list of every build
- Automatic language detection, re-run per speech window so mixed-language recordings transcribe correctly
- Silero voice-activity detection, embedded in the binary and enabled by default
- Progress reporting split into `decoding`, `loading` and `transcribing` phases, with a decode heartbeat
- Cancellation at every stage, including mid-inference

**Working with the result**
- Timecoded segments, click-to-copy per line, case-insensitive search with highlighting
- Local summarization with a Qwen3 model, in brief and structured modes
- Two readability passes over the transcript, "Fix punctuation" and "Make readable", both editing
  each fragment in place: same segments, same order, same timecodes, only the text changes. The
  first restores punctuation, capitals and drops filler; the second also fixes grammar
- Qwen3-8B and Qwen3-14B as alternatives to Qwen3-4B, for grammar in inflected languages
- The readability pass discards an answer that dropped part of the speech, keeping the original and warning the user
- Export: `txt`, `srt`, `vtt`, `md`, `json`, `docx` for transcripts; `md`, `txt`, `docx` for summaries

**Application**
- Batch queue that processes files sequentially
- Model manager with resumable downloads, pause, cancel, delete and a configurable models directory
- Native drag-and-drop with real file paths, and a file picker; 30+ audio and video formats
- Universal decoding: Symphonia in-process with a bundled FFmpeg sidecar as fallback
- First-run onboarding, an interface in seven languages, human-readable localized errors
- Command-line mode: `voodooscribe transcribe <file> --model <path> [--lang xx] [--vad]`

### Fixed

Notable fixes made during development, kept here because they are easy to reintroduce:

- Whisper hallucinating text over silence, addressed in three layers (VAD, decoder parameters, an
  output filter that deliberately spares ordinary one-word replies)
- Subtitle filler surviving the output filter when whisper glued it onto real speech in the same
  window ("Так, дякую за перегляд!"): the filter now removes single clauses rather than accepting or
  rejecting whole segments, and matches invented credits on prefix so a changing name
  ("Редактор субтитров А.Семкин") no longer slips through
- `Error code: -6` on every file after the first — caused by unsound use of `whisper-rs`'s
  `set_abort_callback_safe`; now covered by a regression test
- Speech played back at the wrong rate for HE-AAC/SBR files, because sample rate was read from the
  container header instead of the decoder's own spec
- Quiet recordings treated as entirely silent by a hardcoded silence threshold, which is now derived
  from the recording itself
- The LLM being reloaded into VRAM on every summarize or polish operation
- VRAM decisions made against total rather than free memory, and eviction that only worked one way
- Out-of-memory conditions surfacing as raw ggml load failures instead of a clear message
- Large models being recommended to machines with no GPU at all
- The webview freezing during GPU inference on Linux (WebKitGTK DMABUF renderer contention)
- Window rendering breaking after a move to a monitor with a different scale factor
- Error toasts auto-dismissing before they could be read
- An unbounded decoder reset loop that could hang decoding forever

### Known limitations

- Developed and tested on Linux; the Windows and macOS packages are built by CI but have not been run
- No speaker diarization
- Summaries come from a local model, 4B by default, and can paraphrase loosely
