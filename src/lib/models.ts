// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

import type { WhisperSize, LlmSize } from "@/lib/ipc";

/**
 * The catalog the UI shows. It mirrors `src-tauri/src/models/registry.rs`, which
 * stays the authority on URLs and byte counts; these are the display figures.
 *
 * One table shared by the models tab, the transcribe tab and onboarding: a
 * per-tab copy drifts, and a model missing from one of them stays invisible there
 * even once it is downloaded.
 */
export interface ModelMeta {
  label: string;
  /** Download size in bytes, the same number as `size_bytes` in the registry.
   *  Displayed through `fmtBytes` so it matches the progress bar. */
  sizeBytes: number;
  /** VRAM needed, for display. */
  vram: string;
  /** The same figure in MB, for the "won't fit" check. */
  vramMb: number;
  filename: string;
}

/** Ordered worst to best, which is the order the dropdowns use. */
export const WHISPER_MODELS: (ModelMeta & { id: WhisperSize })[] = [
  { id: "tiny",              label: "Tiny",                sizeBytes: 77_691_713, vram: "~390 MB", vramMb: 390,  filename: "ggml-tiny.bin" },
  { id: "base",              label: "Base",                sizeBytes: 147_951_465, vram: "~500 MB", vramMb: 500,  filename: "ggml-base.bin" },
  { id: "small",             label: "Small",               sizeBytes: 487_601_967, vram: "~1 GB",   vramMb: 1000, filename: "ggml-small.bin" },
  { id: "medium",            label: "Medium",              sizeBytes: 1_533_763_059, vram: "~2.6 GB", vramMb: 2600, filename: "ggml-medium.bin" },
  { id: "large_v3_turbo_q5", label: "Large v3 Turbo (Q5)", sizeBytes: 574_041_195, vram: "~1.2 GB", vramMb: 1200, filename: "ggml-large-v3-turbo-q5_0.bin" },
  { id: "large_v3_turbo",    label: "Large v3 Turbo",      sizeBytes: 1_624_555_275, vram: "~2.2 GB", vramMb: 2200, filename: "ggml-large-v3-turbo.bin" },
  { id: "large_v3_q5",       label: "Large v3 (Q5)",       sizeBytes: 1_081_140_203, vram: "~2.4 GB", vramMb: 2400, filename: "ggml-large-v3-q5_0.bin" },
  { id: "large_v3",          label: "Large v3",            sizeBytes: 3_095_033_483, vram: "~4.7 GB", vramMb: 4700, filename: "ggml-large-v3.bin" },
];

/** Qwen3 only: `llama::generate` hard-codes ChatML framing and `/no_think`. */
export const LLM_MODELS: (ModelMeta & { id: LlmSize })[] = [
  { id: "qwen3_4_b",  label: "Qwen3-4B",  sizeBytes: 2_497_280_256, vram: "~4 GB",   vramMb: 4000,  filename: "Qwen3-4B-Q4_K_M.gguf" },
  { id: "qwen3_8_b",  label: "Qwen3-8B",  sizeBytes: 5_027_783_488, vram: "~6.5 GB", vramMb: 6500,  filename: "Qwen3-8B-Q4_K_M.gguf" },
  { id: "qwen3_14_b", label: "Qwen3-14B", sizeBytes: 9_001_752_960, vram: "~11 GB",  vramMb: 11000, filename: "Qwen3-14B-Q4_K_M.gguf" },
];

export const whisperById = (id: WhisperSize) =>
  WHISPER_MODELS.find((m) => m.id === id) ?? WHISPER_MODELS[0];
