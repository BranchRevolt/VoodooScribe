// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

import { useState, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useStore } from "@/store";
import { useTranscription } from "@/hooks/useTranscription";
import { DropZone } from "@/components/DropZone";
import { AUDIO_LANGUAGES } from "@/lib/languages";
import { ProgressBar, fmtBytes } from "@/components/ProgressBar";
import { TranscriptPanel } from "@/components/TranscriptPanel";
import { SummaryPanel } from "@/components/SummaryPanel";
import { save } from "@tauri-apps/plugin-dialog";
import { ipc } from "@/lib/ipc";
import { describeError } from "@/lib/errors";
import type { WhisperSize, ExportFormat } from "@/lib/ipc";
import { WHISPER_MODELS } from "@/lib/models";

type ResultTab = "transcript" | "summary";

const TRANSCRIPT_FORMATS: { id: ExportFormat; label: string }[] = [
  { id: "txt",  label: ".txt" },
  { id: "srt",  label: ".srt" },
  { id: "vtt",  label: ".vtt" },
  { id: "md",   label: ".md" },
  { id: "json", label: ".json" },
  { id: "docx", label: ".docx" },
];

// The summary is Markdown prose, so only document formats apply, not the
// subtitle/segment ones (srt/vtt/json). txt/md write the text as-is, docx is
// built.
const SUMMARY_FORMATS: { id: ExportFormat; label: string }[] = [
  { id: "md",   label: ".md" },
  { id: "txt",  label: ".txt" },
  { id: "docx", label: ".docx" },
];

const SELECT_CLS =
  "h-8 px-2.5 rounded-lg border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-900 text-sm text-zinc-800 dark:text-zinc-200 focus:outline-none focus:ring-1 focus:ring-blue-500 appearance-none cursor-pointer";

export function TranscribeTab() {
  const { t } = useTranslation();
  const store = useStore();
  const { start, cancel, summarize, polish, cancelSummarize } = useTranscription();
  const [resultTab, setResultTab] = useState<ResultTab>("transcript");
  const [exportOpen, setExportOpen] = useState(false);

  // Scroll container + quick up/down navigation for long transcripts.
  const scrollRef = useRef<HTMLDivElement>(null);
  const scrollToEdge = (top: number) =>
    scrollRef.current?.scrollTo({ top, behavior: "smooth" });

  const item = store.getActiveItem();
  const isProcessing = item?.status === "processing";
  const isDone       = item?.status === "done";

  // Only models present on disk can be selected.
  const installed = store.modelsStatus?.installed_whisper ?? [];
  const installedModels = WHISPER_MODELS.filter((m) => installed.includes(m.filename));
  const recommended = store.modelsStatus?.recommended_whisper ?? null;

  // The model the next run will use: the user's pick if installed, else the
  // recommended one if installed, else the first installed one.
  const effectiveModel =
    installedModels.find((m) => m.id === store.selectedWhisperModel) ??
    installedModels.find((m) => m.id === recommended) ??
    installedModels[0] ??
    null;

  // Keeps the store and the backend in sync with the effective selection. This is
  // the only place the backend learns which model to load: cmd_transcribe takes no
  // model argument and uses whatever cmd_select_whisper_model last set.
  useEffect(() => {
    if (!effectiveModel) return;
    if (store.selectedWhisperModel !== effectiveModel.id) {
      store.setSelectedWhisperModel(effectiveModel.id);
    }
    ipc.selectModel({ whisper: effectiveModel.id }).catch(() => {});
  }, [effectiveModel?.id]);

  const onFiles = useCallback(
    (paths: string[]) => {
      const id = store.addToQueue(paths[0]);
      store.setActiveItemId(id);
    },
    [store]
  );

  // Saves the current transcript (or summary) in the chosen format. Opens a native
  // save dialog pre-filled with the source file's name and the new extension; the
  // write itself happens in Rust (cmd_export_*).
  const doExport = async (format: ExportFormat) => {
    setExportOpen(false);
    const it = store.getActiveItem();
    if (!it) return;
    const base = it.fileName.replace(/\.[^./\\]+$/, "") || "transcript";
    const isSummary = resultTab === "summary";
    if (isSummary && !it.summary) return;
    if (!isSummary && it.segments.length === 0) return;

    try {
      const path = await save({
        defaultPath: `${base}.${format}`,
        filters: [{ name: format.toUpperCase(), extensions: [format] }],
      });
      if (!path) return; // user cancelled the dialog
      if (isSummary) {
        await ipc.exportSummary(it.summary ?? "", format, path);
      } else {
        // Exports the view currently shown. Both views hold the same segments with
        // the same timecodes, so every transcript format (SRT included) applies to
        // the edited one too.
        const rows = store.readableView && it.polished?.length ? it.polished : it.segments;
        await ipc.exportTranscript(rows, format, path);
      }
    } catch (err) {
      store.setError(describeError(err));
    }
  };

  // Jump-to-top / jump-to-bottom shortcuts, shown once the result is long enough
  // to scroll.
  const showJump = isDone || (item?.segments?.length ?? 0) > 0;

  return (
    <div className="relative h-full">
      <div ref={scrollRef} className="h-full overflow-y-auto">
      <div className="max-w-2xl mx-auto px-6 py-6 flex flex-col gap-5">

        {/* Drop zone or file banner */}
        {!item ? (
          <DropZone onFiles={onFiles} enabled={store.activeTab === "transcribe"} />
        ) : (
          <div className="flex items-center gap-3 px-4 py-3 rounded-xl border border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-900/50">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor"
              className="text-blue-500 shrink-0">
              <path d="M2 2.5A1.5 1.5 0 0 1 3.5 1h9A1.5 1.5 0 0 1 14 2.5v11a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 13.5v-11zm6 3a.5.5 0 0 0-1 0v3.793l-1.146-1.147a.5.5 0 1 0-.708.708l2 2a.5.5 0 0 0 .708 0l2-2a.5.5 0 0 0-.708-.708L8 9.293V5.5z" />
            </svg>
            <span className="text-sm font-medium text-zinc-800 dark:text-zinc-200 flex-1 truncate">
              {item.fileName}
            </span>
            {!isProcessing && (
              <button
                onClick={() => {
                  store.setActiveItemId(null);
                }}
                className="text-xs text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200 transition-colors"
              >
                {t("drop.change")}
              </button>
            )}
          </div>
        )}

        {/* Controls */}
        {item && (
          <div className="flex items-start gap-2">
            {/* Model / language / VAD wrap among themselves; the action button
                stays pinned to the first line. */}
            <div className="flex flex-wrap items-center gap-2 flex-1 min-w-0">
            {/* Model */}
            <div className="flex items-center gap-1.5">
              <label className="text-xs text-zinc-500 dark:text-zinc-400 whitespace-nowrap">
                {t("controls.model")}
              </label>
              {installedModels.length === 0 ? (
                <button
                  onClick={() => store.setActiveTab("models")}
                  className="h-8 px-2.5 rounded-lg border border-amber-300 dark:border-amber-800 bg-amber-50 dark:bg-amber-950/30 text-sm text-amber-700 dark:text-amber-400 hover:bg-amber-100 dark:hover:bg-amber-950/50 transition-colors"
                >
                  {t("controls.no_models")}
                </button>
              ) : (
                <select
                  className={SELECT_CLS}
                  value={effectiveModel?.id ?? ""}
                  onChange={(e) =>
                    store.setSelectedWhisperModel((e.target.value as WhisperSize) || null)
                  }
                  disabled={isProcessing}
                >
                  {installedModels.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.label} ({fmtBytes(m.sizeBytes)})
                    </option>
                  ))}
                </select>
              )}
            </div>

            {/* Language */}
            <div className="flex items-center gap-1.5">
              <label className="text-xs text-zinc-500 dark:text-zinc-400 whitespace-nowrap">
                {t("controls.language")}
              </label>
              <select
                className={SELECT_CLS}
                value={store.transcribeLang ?? ""}
                onChange={(e) => store.setTranscribeLang(e.target.value || null)}
                disabled={isProcessing}
              >
                <option value="">{t("controls.lang_auto")}</option>
                {AUDIO_LANGUAGES.map((l) => (
                  <option key={l.value} value={l.value}>
                    {l.label}
                  </option>
                ))}
              </select>
            </div>

            {/* VAD */}
            <label className="flex items-center gap-1.5 cursor-pointer select-none">
              <input
                type="checkbox"
                checked={store.useVad}
                onChange={(e) => store.setUseVad(e.target.checked)}
                disabled={isProcessing}
                className="rounded border-zinc-300 dark:border-zinc-600 text-blue-500 focus:ring-blue-500"
              />
              <span className="text-xs text-zinc-600 dark:text-zinc-400">{t("controls.vad")}</span>
            </label>
            </div>

            {/* Start / Cancel. Fixed width, so it neither wraps under the controls
                nor resizes when the label changes. */}
            <div className="shrink-0">
            {!isProcessing ? (
              <button
                onClick={start}
                disabled={!item || isProcessing || !effectiveModel || store.queueRunning}
                title={!effectiveModel ? t("controls.no_models") : undefined}
                className="h-8 w-24 rounded-lg bg-blue-500 hover:bg-blue-600 disabled:opacity-40 disabled:cursor-not-allowed text-white text-sm font-medium transition-colors"
              >
                {t("controls.start")}
              </button>
            ) : (
              <button
                onClick={cancel}
                className="h-8 w-24 rounded-lg border border-zinc-300 dark:border-zinc-700 hover:bg-zinc-100 dark:hover:bg-zinc-800 text-sm transition-colors"
              >
                {t("controls.cancel")}
              </button>
            )}
            </div>
          </div>
        )}

        {/* Mixed-language hint */}
        {item && (
          <p className="-mt-3 text-xs italic text-amber-500 dark:text-amber-400">
            {t("controls.multilang_hint")}
          </p>
        )}

        {/* Progress */}
        {isProcessing && (
          <ProgressBar
            percent={item.progress}
            // Decoding and loading emit no percent, and the whisper encoder runs
            // several seconds per 30s chunk before the first one arrives, so the bar
            // is indeterminate until a transcription percent comes in.
            indeterminate={
              store.transcribePhase === "loading" ||
              (store.transcribePhase === "decoding" && (item.progress ?? 0) === 0) ||
              (store.transcribePhase === "transcribing" && (item.progress ?? 0) === 0)
            }
            label={
              store.transcribeDetail
                ? `${t(`progress.${store.transcribePhase}`)} · ${store.transcribeDetail}`
                : t(`progress.${store.transcribePhase}`)
            }
          />
        )}

        {/* Result area */}
        {(isDone || (item?.segments?.length ?? 0) > 0) && (
          <div className="flex flex-col gap-3">
            {/* Sub-tabs + export */}
            <div className="flex items-center border-b border-zinc-200 dark:border-zinc-800">
              {(["transcript", "summary"] as ResultTab[]).map((tab) => (
                <button
                  key={tab}
                  onClick={() => setResultTab(tab)}
                  className={`px-3 py-2 text-sm font-medium border-b-2 -mb-px transition-colors ${
                    resultTab === tab
                      ? "border-blue-500 text-zinc-900 dark:text-zinc-100"
                      : "border-transparent text-zinc-500 hover:text-zinc-800 dark:hover:text-zinc-200"
                  }`}
                >
                  {t(`result.${tab}`)}
                </button>
              ))}
              <div className="flex-1" />

              {/* Export */}
              <div className="relative mb-1">
                <button
                  onClick={() => setExportOpen((v) => !v)}
                  disabled={resultTab === "summary" ? !item?.summary : (item?.segments.length ?? 0) === 0}
                  className="text-xs px-3 py-1.5 rounded-lg border border-zinc-200 dark:border-zinc-700 hover:bg-zinc-100 dark:hover:bg-zinc-800 disabled:opacity-40 disabled:cursor-not-allowed transition-colors flex items-center gap-1"
                >
                  {t("result.export")}
                  <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor">
                    <path d="M2 3.5L5 6.5L8 3.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" fill="none" />
                  </svg>
                </button>
                {exportOpen && (
                  <>
                    <div className="fixed inset-0 z-10" onClick={() => setExportOpen(false)} />
                    <div className="absolute right-0 top-full mt-1 w-28 bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded-xl shadow-lg overflow-hidden z-20">
                      {(resultTab === "summary" ? SUMMARY_FORMATS : TRANSCRIPT_FORMATS).map((f) => (
                        <button
                          key={f.id}
                          onClick={() => doExport(f.id)}
                          className="w-full text-left px-3 py-2 text-sm hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors"
                        >
                          {f.label}
                        </button>
                      ))}
                    </div>
                  </>
                )}
              </div>
            </div>

            {resultTab === "transcript" && (
              <TranscriptPanel
                segments={item?.segments ?? []}
                polished={item?.polished ?? null}
                showReadable={store.readableView}
                onToggleReadable={store.toggleReadableView}
                polishMode={store.polishMode}
                isPolishing={store.isPolishing}
                polishProgress={store.summarizeProgress}
                onPolish={polish}
                onCancelPolish={cancelSummarize}
              />
            )}
            {resultTab === "summary" && (
              <SummaryPanel
                summary={item?.summary ?? null}
                hasTranscript={(item?.segments?.length ?? 0) > 0}
                isSummarizing={store.isSummarizing}
                progress={store.summarizeProgress}
                mode={store.summaryMode}
                onModeChange={store.setSummaryMode}
                onSummarize={summarize}
                onCancel={cancelSummarize}
              />
            )}
          </div>
        )}
      </div>
      </div>

      {/* Jump to top / bottom */}
      {showJump && (
        <>
          <button
            onClick={() => scrollToEdge(0)}
            title={t("result.jump_top")}
            aria-label={t("result.jump_top")}
            className="absolute top-4 right-4 z-10 flex items-center justify-center w-9 h-9 rounded-full bg-transparent border border-zinc-400 dark:border-zinc-600 text-zinc-400 dark:text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300 hover:border-zinc-500 transition-colors"
          >
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M3 9l4-4 4 4" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </button>
          <button
            onClick={() => scrollToEdge(scrollRef.current?.scrollHeight ?? 0)}
            title={t("result.jump_bottom")}
            aria-label={t("result.jump_bottom")}
            className="absolute bottom-4 right-4 z-10 flex items-center justify-center w-9 h-9 rounded-full bg-transparent border border-zinc-400 dark:border-zinc-600 text-zinc-400 dark:text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300 hover:border-zinc-500 transition-colors"
          >
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M3 5l4 4 4-4" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </button>
        </>
      )}
    </div>
  );
}
