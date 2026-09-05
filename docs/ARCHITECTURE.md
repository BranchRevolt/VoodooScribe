# Architecture

This document is for people changing the code. It describes how a file becomes a transcript, why
certain unusual decisions were made, and which parts bite back.

## The stack

| Layer | Choice |
|---|---|
| Shell | Tauri 2 (Rust backend, system WebView frontend) |
| Frontend | React 18, TypeScript, Vite 6, Tailwind 3, Zustand, i18next |
| Speech recognition | whisper.cpp via `whisper-rs` (pinned to `=0.16.0`) |
| Language model | llama.cpp via `llama-cpp-2` |
| Audio decoding | Symphonia in-process, FFmpeg sidecar as fallback |
| Resampling | rubato |
| GPU | Vulkan on Linux/Windows, Metal on macOS |

`whisper-rs` is pinned exactly because its API changes on every minor release.

## The pipeline

```
file ──► decode ──► resample to 16 kHz mono ──► [VAD] ──► whisper ──► segments
                                                             │
                                                             ├──► summarize (Qwen3)
                                                             └──► readability pass (Qwen3)
```

**Decode** (`audio/decode.rs`). Symphonia handles common containers in-process. Anything it rejects
goes to the bundled FFmpeg sidecar (`audio/ffmpeg.rs`).

One subtlety worth preserving: the sample rate and channel count come from the **decoder's** spec,
not from the container header. HE-AAC/SBR in mp4 declares 24 kHz while actually decoding at 48 kHz;
trusting the header made the resampler stretch speech to twice its length, and Whisper heard
gibberish. `collect_samples` also clamps to the buffer's real channel count.

Decoding emits a heartbeat (`decode_to_pcm_with_progress`) so long files report progress instead of
looking frozen. A `ResetRequired` loop is capped at 64 consecutive resets with no packet decoded —
without the cap, a malformed stream spun the decoder forever.

**Resample** (`audio/resample.rs`). Whisper needs 16 kHz mono.

**VAD** (`transcribe/whisper.rs`, `audio/vad.rs`). Voice-activity detection uses whisper.cpp's
built-in Silero integration rather than a hand-rolled energy gate. The model file
(`ggml-silero-v5.1.2.bin`, 865 KB) is embedded in the binary with `include_bytes!` and written to
disk on demand, because whisper.cpp wants a path, not bytes.

Two things to know: `set_vad_model_path` must be called **before** `enable_vad`, which panics on a
null path — they are paired in `apply_vad` for that reason. And whisper.cpp caches the loaded VAD
model on the `WhisperState`, so multi-chunk runs must reuse one state; creating a state per chunk
reloaded Silero dozens of times per file.

`audio/vad.rs` keeps only `split_on_silence`, the pause finder used to pick language-window
boundaries. Its silence threshold is derived from the recording — the 20th percentile of frame
energy times three, clamped — because a fixed threshold read quiet recordings as entirely silent.

**Transcribe** (`transcribe/`). Either a single pass at a fixed language, or `transcribe_multilang`,
which re-detects the language per speech window so code-switching recordings come out right.
Timestamps stay on the original timeline: whisper.cpp maps VAD-processed timestamps back itself, and
the multilang path adds each chunk's offset.

**Summarize / readability passes** (`summarize/`). Token-aware chunking, then a local Qwen3 per
chunk. "Summarize" produces prose in the third person, not a meeting protocol. The readability
passes restore what whisper does badly — its punctuation is weak on Russian even at large sizes.
Prompts live in `src-tauri/resources/prompts/` and are written in English regardless of the
transcript's language.

A readability pass returns *segments*, not a string, and it edits them **in place**: the answer has
the same number of segments, in the same order, with the same timecodes, and only the text changes.
Merging the fragments into paragraphs reads better as prose, but a transcript without per-fragment
timecodes is no longer a transcript, so the paragraph design was dropped.

The mechanism is a numbered list. `lines::number` renders the chunk as `1. …`, `2. …`; the model is
told to hand back the same numbers with the same utterances, cleaned up; `lines::parse` maps the
answer back onto the expected slots. Chunks are cut on segment boundaries, so no chunk holds half a
sentence, and are capped in *length* as well as tokens — past a few dozen lines a 4B model starts
skipping numbers or renumbering from 1. Two consequences worth knowing:

- Validation is per line, not per chunk. A number the model never answered has no slot filled and
  keeps its original text; a line rewritten past recognition is rejected on its own without costing
  the rest of the chunk.
- Because the result is a list of timed segments, the cleaned-up view keeps everything the raw view
  has: search, per-row copy, and export to every transcript format.

The pass has two prompts, surfaced as two buttons. `polish.md` ("Fix punctuation") may not change a
word; `polish_edited.md` ("Make readable") may also fix agreement, case endings and word order, and
is what makes the output readable in inflected languages. The verbatim pass is the conservative one
on purpose: a transcript that silently rewrites what someone said is a liability for the recordings
this app is aimed at. Both prompts forbid inventing a plausible sentence in place of a garbled one —
a visibly broken fragment is the only signal the reader gets that the audio was bad there.

Two details of the prompting are load-bearing, both found by measurement:

- **The rules go after the transcript, not into the system turn.** With the rules first and three
  thousand tokens of speech after them, Qwen3-4B forgets the task and echoes the input back — the two
  passes produced byte-identical output. Moving the rules behind the text made the model actually edit.
- **Each answered line is checked before it is accepted** (`lines::preserves_speech`). Told to fix the
  grammar of a long transcript, the model will quietly drop or summarize part of it, so the edited
  line is compared against its original by word count, with bounds either way and a fixed slack for
  short lines where ratios mean little. A line that fails keeps its source text and the user is told
  how many did (`summarize://degraded` → a warning banner), because handing back an unedited
  transcript without a word looks like the feature did nothing.

Three LLM sizes are offered (`registry::all_llm_models`). The 4B quant is adequate for summaries but
visibly weak on Slavic morphology, which is exactly what the edited pass leans on; the 8B and 14B are
the reason that pass is worth having. `AppState::auto_discover` picks the largest installed LLM that
fits the detected VRAM.

## Suppressing Whisper hallucinations

Whisper invents text over silence — "Thanks for watching", "Продолжение следует" and similar
artifacts of its training data. Three independent layers address this:

1. VAD is on by default, so silence rarely reaches the model at all
2. Decoder parameters (`apply_decoding_params`) discourage the failure mode
3. `transcribe/hallucination.rs` filters known artifacts from the output

The filter works on **clauses**, not whole segments. Whisper glues its filler onto whatever real
speech shared the window — "Так, дякую за перегляд!" is one segment holding one real word and one
hallucination — so the text is split on `. ! ? … ; ,` and only the boilerplate clauses are removed.
A full stop ends a clause only when a non-alphanumeric follows it, so the initials in invented
credits ("Редактор субтитров А.Синецкая") stay attached to the phrase they belong to. If nothing was
removed the text is returned byte-identical, which is what keeps a segment that legitimately ends on
a comma intact.

Two match modes: subtitle phrases people also say ("спасибо за просмотр") must match a whole clause,
while credits carry a name that changes every window ("А.Синецкая", "А.Семкин", "DimaTorzok") and
match on prefix. The filter deliberately excludes bare words like "спасибо", "you", "bye" — those are
ordinary replies, and removing them silently deleted real dialogue.

## Concurrency and cancellation

Every heavy operation runs in `spawn_blocking`; the UI thread is never touched. Progress travels as
Tauri events (`transcribe://progress`, `model://download-progress`, `summarize://progress`).

Cancellation uses `Arc<AtomicBool>` checked at every stage — per packet while decoding, before model
load, and inside inference. An aborted `full()` returns `Err`, so the code checks the cancel flag
**first** and reports `Canceled`; otherwise a user pressing Cancel got an error toast.

> **Landmine:** `whisper-rs` 0.16's `set_abort_callback_safe` is unsound. Using it corrupted state
> so that every file after the first failed with `Error code: -6`. `tests/whisper_reuse.rs` is the
> regression test. Do not reintroduce it.

## Model lifecycle and VRAM

Both the Whisper context and the LLM are cached in `AppState` — reloading Qwen3-4B on every click
cost 3–5 seconds of VRAM streaming. The llama backend is a process-wide `OnceLock` singleton,
because a second live backend returns `BackendAlreadyInitialized`. One LLM operation runs at a time,
guarded by an RAII claim (`try_claim_llm`) that surfaces as `LlmBusy`.

VRAM decisions use **free** memory, not total (`vram::free_vram_mb()` — nvidia-smi on NVIDIA, sysfs
on Linux AMD/Intel, with bigger headroom on platforms where only the total is knowable). Eviction
works in both directions: loading Whisper can evict the LLM and vice versa, only when both will not
fit. A running operation holds its own `Arc`, so eviction can never yank a model out from under it.
Machines with no GPU get recommendations based on system RAM instead.

Downloads are fire-and-forget: the command spawns the transfer and returns immediately so the IPC
call never hangs, and the UI polls `cmd_download_status`. Pause keeps the `.tmp` file; resume
continues with an HTTP Range request.

## Code map

```
src-tauri/src/
  lib.rs              app setup, command registration, environment fixes
  main.rs             entry point; also dispatches the `transcribe` CLI subcommand
  state.rs            AppState: caches, cancel flags, model paths, LLM claim
  error.rs            AppError — every error carries a code the frontend translates
  vram.rs             GPU memory detection and fit arithmetic
  monitor_fix.rs      Linux-only workaround for WebKitGTK after a monitor change
  audio/              decode, ffmpeg fallback, resample, silence splitting
  transcribe/         whisper wrapper, speech windowing, hallucination filter
  summarize/          llama wrapper, chunking, line-preserving readability pass
  models/             registry, downloader
  commands/           thin Tauri commands: transcribe, summarize, export, models

src/
  App.tsx             tab shell; tabs stay mounted and hide via CSS
  components/         one file per tab or panel
  hooks/              useFileDrop (native drag-drop), useTranscription
  store/              Zustand state
  lib/ipc.ts          the only place that talks to Rust
  i18n/               index.ts + one file per interface language (en, zh-CN, fr, de, pt-BR, ru, es)
```

## House rules

- **Typed IPC only.** No `invoke("bare-string")` anywhere; every call goes through `src/lib/ipc.ts`.
- **Thin commands.** Tauri commands parse arguments, call a module, map errors. No logic.
- **Tabs stay mounted.** Switching tabs must never unmount an in-flight download, so tabs hide with
  CSS rather than unmounting.
- **Errors are human.** `AppError` carries a code; the frontend translates it. Errors do not
  auto-dismiss (warnings do) — a decode failure that vanished after six seconds was unreportable.
- **Zero warnings.** Both `cargo build` and `tsc` are kept clean.

## Platform quirks worth knowing

**Drag-and-drop.** Tauri v2's native `onDragDropEvent` gives real absolute paths; the browser File
API (react-dropzone) does not, and the backend needs a path to open the file.

**The webview freezing under GPU load.** WebKitGTK's DMABUF renderer competes with Vulkan inference
for the GPU and loses in bursts, freezing page content while the native window still drags. `lib.rs`
sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` on Linux before GTK initializes. Threading does not help —
this is device-level contention.

**Wayland window icons.** A window cannot set its own icon; the compositor reads it from a
`.desktop` file matched by `app_id`. `enableGTKAppId` pins the `app_id` to the bundle identifier, and
`scripts/install-linux-desktop-entry.sh` installs a matching entry for development builds.

**Monitor changes.** Dragging the window to a display with a different scale factor left WebKitGTK
laid out for the old screen. `monitor_fix.rs` watches window events and performs a zoom reset plus a
one-pixel resize round-trip, throttled and guarded against re-entry.

## Deliberate non-goals

**Speaker diarization** is not implemented. It would mean adding an ONNX runtime and two more models
for a feature that is CPU-only in that configuration and unreliable exactly where people want it
most — overlapping speech, similar voices, meetings recorded on one microphone. Whisper's segment
boundaries also do not line up with diarization boundaries, so even a correct result looks wrong at
the seams. If it is ever added, it must be opt-in, must accept a known speaker count, and must let
the user rename and reassign speakers by hand.

**Cloud anything.** No telemetry, no crash reporting, no remote inference. The only network traffic
the app ever makes is downloading models you asked for.
