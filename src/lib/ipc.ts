// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type { UnlistenFn };

// ---------------------------------------------------------------------------
// Model types
// ---------------------------------------------------------------------------

// Ordered worst to best. Mirrors the Rust enum's serde names.
export type WhisperSize =
  | "tiny"
  | "base"
  | "small"
  | "medium"
  | "large_v3_turbo_q5"
  | "large_v3_turbo"
  | "large_v3_q5"
  | "large_v3";

export type LlmSize = "qwen3_4_b" | "qwen3_8_b" | "qwen3_14_b";

// Mirrors the Rust enum's serde names; pinned by a test in models/registry.rs.
export type ModelKind = { whisper: WhisperSize } | { llama: LlmSize };

export interface ModelInfo {
  kind: ModelKind;
  name: string;
  filename: string;
  url: string;
  size_bytes: number;
  ram_required_gb: number;
  sha256: string | null;
}

export interface ModelsStatus {
  whisper_loaded: boolean;
  whisper_path: string | null;
  llm_loaded: boolean;
  llm_path: string | null;
  installed_whisper: string[];      // filenames present on disk
  installed_llm: string[];        // filenames present on disk
  available_vram_mb: number;        // 0 = unknown
  recommended_whisper: WhisperSize; // based on detected VRAM
}

// ---------------------------------------------------------------------------
// Transcription types
// ---------------------------------------------------------------------------

export interface Segment {
  t0: number; // ms
  t1: number; // ms
  text: string;
}

export type ExportFormat = "txt" | "srt" | "vtt" | "json" | "md" | "docx";

// "brief" = short plain retelling; "structured" = detailed report (topic,
// sub-topics, theses, lists).
export type SummaryMode = "brief" | "structured";

/** How far the readability pass may go. See cmd_polish_transcript. */
export type PolishMode = "verbatim" | "edited";

// ---------------------------------------------------------------------------
// Event payload types
// ---------------------------------------------------------------------------

export type TranscribePhase = "decoding" | "loading" | "transcribing";

export interface TranscribeProgressEvent {
  percent: number;
  phase: TranscribePhase;
  segment: Segment | null;
  /// How much audio has been decoded so far ("3:12" / "3:12 / 41:07"). Sent only
  /// during the decoding phase, which has no percentage of its own.
  detail: string | null;
}

export interface SummarizeProgressEvent {
  percent: number;
}

export interface PolishDegradedEvent {
  /** How many lines kept their original text because the answer was unusable. */
  lines: number;
}

export interface DownloadProgressEvent {
  filename: string;
  downloaded: number;
  total: number;
  percent: number;
  phase: "downloading" | "verifying";
  /** Smoothed transfer rate; 0 while unknown or during the integrity check. */
  bytesPerSec: number;
  /** Seconds left at that rate; 0 when either figure is unknown. */
  etaSecs: number;
}

export interface DownloadDoneEvent {
  filename: string;
  cancelled: boolean;
  error: string | null;
}

export interface DownloadFileStatus {
  installed: boolean;
  partial_bytes: number;
}

// ---------------------------------------------------------------------------
// IPC calls
// ---------------------------------------------------------------------------

export const ipc = {
  // models
  getModelsStatus:    ()                    => invoke<ModelsStatus>("cmd_models_status"),
  listWhisperModels:  ()                    => invoke<ModelInfo[]>("cmd_list_whisper_models"),
  listLlmModels:      ()                    => invoke<ModelInfo[]>("cmd_list_llm_models"),
  getDownloadStatus:  (filename: string)    => invoke<DownloadFileStatus>("cmd_download_status", { filename }),
  downloadModel:      (kind: ModelKind)     => invoke<void>("cmd_download_model", { kind }),
  cancelDownload:     ()                    => invoke<void>("cmd_cancel_download"),
  cancelAndDelete:    (filename: string)    => invoke<void>("cmd_cancel_and_delete", { filename }),
  deleteModel:        (filename: string)    => invoke<void>("cmd_delete_model", { filename }),
  selectModel:        (kind: ModelKind)     => invoke<void>("cmd_select_model", { kind }),
  getModelsDir:       ()                    => invoke<string>("cmd_get_models_dir"),
  setModelsDir:       (path: string)        => invoke<string>("cmd_set_models_dir", { path }),
  setModelPath:       (kind: ModelKind, path: string) =>
                        invoke<void>("cmd_set_model_path", { kind, path }),

  // transcription
  transcribe: (
    filePath: string,
    language: string | null,
    useVad: boolean,
    nThreads?: number,
  ) => invoke<Segment[]>("cmd_transcribe", { filePath, language, useVad, nThreads }),

  cancelTranscribe: () => invoke<void>("cmd_cancel_transcribe"),

  // summarization
  // `language` is the ISO 639-1 code chosen on the transcribe screen, or null on
  // auto — the backend then infers it from the transcript's script.
  summarize: (transcript: string, mode: SummaryMode, language: string | null) =>
    invoke<string>("cmd_summarize", { transcript, mode, language }),
  // Readability pass. "verbatim" keeps the words, "edited" also fixes grammar.
  polishTranscript: (segments: Segment[], mode: PolishMode, language: string | null) =>
    invoke<Segment[]>("cmd_polish_transcript", { segments, mode, language }),
  // Cancels a running summarize / polish.
  cancelSummarize: () => invoke<void>("cmd_cancel_summarize"),

  // export
  exportTranscript: (segments: Segment[], format: ExportFormat, outputPath: string) =>
    invoke<void>("cmd_export_transcript", { segments, format, outputPath }),
  exportSummary: (summary: string, format: ExportFormat, outputPath: string) =>
    invoke<void>("cmd_export_summary", { summary, format, outputPath }),
} as const;

// ---------------------------------------------------------------------------
// Event subscriptions
// ---------------------------------------------------------------------------

export const events = {
  onTranscribeProgress: (cb: (e: TranscribeProgressEvent) => void): Promise<UnlistenFn> =>
    listen<TranscribeProgressEvent>("transcribe://progress", (e) => cb(e.payload)),

  onSummarizeProgress: (cb: (e: SummarizeProgressEvent) => void): Promise<UnlistenFn> =>
    listen<SummarizeProgressEvent>("summarize://progress", (e) => cb(e.payload)),

  /** Part of the readability pass came back unedited; see cmd_polish_transcript. */
  onPolishDegraded: (cb: (e: PolishDegradedEvent) => void): Promise<UnlistenFn> =>
    listen<PolishDegradedEvent>("summarize://degraded", (e) => cb(e.payload)),

  onDownloadProgress: (cb: (e: DownloadProgressEvent) => void): Promise<UnlistenFn> =>
    listen<DownloadProgressEvent>("model://download-progress", (e) => cb(e.payload)),

  onDownloadDone: (cb: (e: DownloadDoneEvent) => void): Promise<UnlistenFn> =>
    listen<DownloadDoneEvent>("model://download-done", (e) => cb(e.payload)),
} as const;
