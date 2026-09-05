// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

import { create } from "zustand";
import type { Segment, ModelsStatus, DownloadProgressEvent, WhisperSize, SummaryMode, PolishMode, TranscribePhase } from "@/lib/ipc";
import { resolveUiLanguage } from "@/i18n";

export type AppTab = "transcribe" | "queue" | "models" | "settings";
export type Theme = "light" | "dark" | "system";
export type QueueStatus = "waiting" | "processing" | "done" | "error";

export interface QueueItem {
  id: string;
  filePath: string;
  fileName: string;
  status: QueueStatus;
  progress: number;
  segments: Segment[];
  summary: string | null;
  /** Readable view: the same segments and timecodes with the text cleaned up. */
  polished: Segment[] | null;
  error: string | null;
}

let _uid = 0;
const uid = () => `q${Date.now()}_${++_uid}`;

const basename = (p: string) => p.split(/[\\/]/).pop() ?? p;

interface AppStore {
  activeTab: AppTab;
  setActiveTab: (t: AppTab) => void;

  theme: Theme;
  setTheme: (t: Theme) => void;

  queue: QueueItem[];
  addToQueue: (filePath: string) => string;
  removeFromQueue: (id: string) => void;
  clearCompleted: () => void;
  updateQueueItem: (id: string, patch: Partial<QueueItem>) => void;

  activeItemId: string | null;
  setActiveItemId: (id: string | null) => void;
  getActiveItem: () => QueueItem | null;

  isTranscribing: boolean;
  setIsTranscribing: (v: boolean) => void;
  // Batch queue processing. queueRunning: the sequential runner is active.
  // stopQueue: a stop was requested for the current item.
  queueRunning: boolean;
  setQueueRunning: (v: boolean) => void;
  stopQueue: boolean;
  setStopQueue: (v: boolean) => void;
  // Pipeline stage shown as the progress label (decode, load, transcribe).
  transcribePhase: TranscribePhase;
  setTranscribePhase: (p: TranscribePhase) => void;
  // Heartbeat text for the decoding phase; null when there is nothing to show.
  transcribeDetail: string | null;
  setTranscribeDetail: (d: string | null) => void;
  isSummarizing: boolean;
  setIsSummarizing: (v: boolean) => void;
  // Percent (0–100) of the running summarize / polish operation.
  summarizeProgress: number;
  setSummarizeProgress: (v: number) => void;
  // Which kind of summary to generate: brief retelling or structured report.
  summaryMode: SummaryMode;
  setSummaryMode: (m: SummaryMode) => void;
  /** Transcript sub-view. Kept here rather than in the panel because Export
   *  follows it. */
  readableView: boolean;
  toggleReadableView: () => void;
  setReadableView: (v: boolean) => void;
  polishMode: PolishMode;
  setPolishMode: (m: PolishMode) => void;
  isPolishing: boolean;
  setIsPolishing: (v: boolean) => void;

  modelsStatus: ModelsStatus | null;
  setModelsStatus: (s: ModelsStatus) => void;
  downloadProgress: Record<string, Omit<DownloadProgressEvent, "filename">>;
  setDownloadProgress: (e: DownloadProgressEvent) => void;
  clearDownloadProgress: () => void;

  uiLanguage: string;
  setUiLanguage: (l: string) => void;
  transcribeLang: string | null;
  setTranscribeLang: (l: string | null) => void;
  useVad: boolean;
  setUseVad: (v: boolean) => void;
  selectedWhisperModel: WhisperSize | null;
  setSelectedWhisperModel: (m: WhisperSize | null) => void;

  error: string | null;
  setError: (e: string | null) => void;
  warning: string | null;
  setWarning: (w: string | null) => void;

  // First-run onboarding: shown once, then remembered.
  onboarded: boolean;
  setOnboarded: (v: boolean) => void;
}

export const useStore = create<AppStore>((set, get) => ({
  activeTab: "transcribe",
  setActiveTab: (t) => set({ activeTab: t }),

  theme: (localStorage.getItem("theme") as Theme | null) ?? "system",
  setTheme: (t) => {
    localStorage.setItem("theme", t);
    set({ theme: t });
  },

  queue: [],
  addToQueue: (filePath) => {
    const id = uid();
    set((s) => ({
      queue: [
        ...s.queue,
        {
          id,
          filePath,
          fileName: basename(filePath),
          status: "waiting" as const,
          progress: 0,
          segments: [],
          summary: null,
          polished: null,
          error: null,
        },
      ],
    }));
    return id;
  },
  removeFromQueue: (id) =>
    set((s) => ({ queue: s.queue.filter((i) => i.id !== id) })),
  clearCompleted: () =>
    set((s) => ({
      queue: s.queue.filter((i) => i.status !== "done" && i.status !== "error"),
    })),
  updateQueueItem: (id, patch) =>
    set((s) => ({
      queue: s.queue.map((i) => (i.id === id ? { ...i, ...patch } : i)),
    })),

  activeItemId: null,
  setActiveItemId: (id) => set({ activeItemId: id }),
  getActiveItem: () => {
    const { queue, activeItemId } = get();
    return queue.find((i) => i.id === activeItemId) ?? null;
  },

  isTranscribing: false,
  setIsTranscribing: (v) => set({ isTranscribing: v }),
  queueRunning: false,
  setQueueRunning: (v) => set({ queueRunning: v }),
  stopQueue: false,
  setStopQueue: (v) => set({ stopQueue: v }),
  transcribePhase: "decoding",
  setTranscribePhase: (p) => set({ transcribePhase: p }),
  transcribeDetail: null,
  setTranscribeDetail: (d) => set({ transcribeDetail: d }),
  isSummarizing: false,
  setIsSummarizing: (v) => set({ isSummarizing: v }),
  summarizeProgress: 0,
  setSummarizeProgress: (v) => set({ summarizeProgress: v }),
  summaryMode: "brief",
  setSummaryMode: (m) => set({ summaryMode: m }),
  readableView: true,
  toggleReadableView: () => set((s) => ({ readableView: !s.readableView })),
  setReadableView: (v) => set({ readableView: v }),
  // Verbatim by default: rewriting the grammar of a record of what someone said
  // has to be an explicit choice.
  polishMode: "verbatim",
  setPolishMode: (m) => set({ polishMode: m }),
  isPolishing: false,
  setIsPolishing: (v) => set({ isPolishing: v }),

  modelsStatus: null,
  setModelsStatus: (s) => set({ modelsStatus: s }),
  downloadProgress: {},
  setDownloadProgress: ({ filename, ...rest }) =>
    set((s) => ({
      downloadProgress: { ...s.downloadProgress, [filename]: rest },
    })),
  clearDownloadProgress: () => set({ downloadProgress: {} }),

  uiLanguage: resolveUiLanguage(localStorage.getItem("lang")) ?? "en",
  setUiLanguage: (l) => {
    localStorage.setItem("lang", l);
    set({ uiLanguage: l });
  },
  transcribeLang: null,
  setTranscribeLang: (l) => set({ transcribeLang: l }),
  // On by default: without the VAD whisper is fed silence and fills it with the
  // subtitle boilerplate it was trained on ("Thanks for watching!").
  useVad: localStorage.getItem("useVad") !== "0",
  setUseVad: (v) => {
    localStorage.setItem("useVad", v ? "1" : "0");
    set({ useVad: v });
  },
  selectedWhisperModel: null,
  setSelectedWhisperModel: (m) => set({ selectedWhisperModel: m }),

  error: null,
  setError: (e) => set({ error: e }),
  warning: null,
  setWarning: (w) => set({ warning: w }),

  onboarded: localStorage.getItem("onboarded") === "1",
  setOnboarded: (v) => {
    localStorage.setItem("onboarded", v ? "1" : "0");
    set({ onboarded: v });
  },
}));
