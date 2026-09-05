# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-09-05

First public release.

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

### Known limitations

- Developed and tested on Linux; the Windows and macOS packages are built by CI but have not been run
- No speaker diarization
- Summaries come from a local model, 4B by default, and can paraphrase loosely
