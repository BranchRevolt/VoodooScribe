// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

import { useTranslation } from "react-i18next";
import ReactMarkdown from "react-markdown";
import { ProgressBar } from "@/components/ProgressBar";
import type { SummaryMode } from "@/lib/ipc";

interface Props {
  summary: string | null;
  hasTranscript: boolean;
  isSummarizing: boolean;
  progress: number;
  mode: SummaryMode;
  onModeChange: (m: SummaryMode) => void;
  onSummarize: () => void;
  onCancel: () => void;
}

const MODES: { id: SummaryMode; key: string }[] = [
  { id: "brief", key: "result.summary_mode_brief" },
  { id: "structured", key: "result.summary_mode_structured" },
];

export function SummaryPanel({ summary, hasTranscript, isSummarizing, progress, mode, onModeChange, onSummarize, onCancel }: Props) {
  const { t } = useTranslation();

  if (!hasTranscript) {
    return (
      <div className="flex items-center justify-center h-32 text-sm text-zinc-400 dark:text-zinc-600">
        {t("result.no_summary")}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      {/* Mode toggle + generate / regenerate */}
      <div className="flex items-center gap-3 flex-wrap">
        {/* Segmented toggle: brief retelling vs detailed structured report */}
        <div className="inline-flex rounded-lg border border-zinc-200 dark:border-zinc-700 p-0.5 bg-zinc-100 dark:bg-zinc-800/50">
          {MODES.map((m) => (
            <button
              key={m.id}
              onClick={() => onModeChange(m.id)}
              disabled={isSummarizing}
              className={`px-3 py-1 text-xs font-medium rounded-md transition-colors disabled:cursor-not-allowed ${
                mode === m.id
                  ? "bg-white dark:bg-zinc-900 text-zinc-900 dark:text-zinc-100 shadow-sm"
                  : "text-zinc-500 dark:text-zinc-400 hover:text-zinc-800 dark:hover:text-zinc-200"
              }`}
            >
              {t(m.key)}
            </button>
          ))}
        </div>

        <div className="flex-1" />

        {summary && !isSummarizing && (
          <button
            onClick={() => navigator.clipboard.writeText(summary).catch(() => {})}
            className="text-xs text-zinc-500 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors"
          >
            {t("result.copy_all")}
          </button>
        )}
        {isSummarizing ? (
          <button
            onClick={onCancel}
            className="px-3 py-1.5 rounded-lg border border-zinc-300 dark:border-zinc-700 hover:bg-zinc-100 dark:hover:bg-zinc-800 text-xs font-medium transition-colors"
          >
            {t("controls.cancel")}
          </button>
        ) : (
          <button
            onClick={onSummarize}
            className="px-3 py-1.5 rounded-lg bg-blue-500 hover:bg-blue-600 text-white text-xs font-medium transition-colors"
          >
            {summary ? t("result.regenerate") : t("controls.summarize")}
          </button>
        )}
      </div>

      {isSummarizing ? (
        <ProgressBar
          percent={progress}
          indeterminate={progress === 0}
          label={t("controls.summarizing")}
        />
      ) : summary ? (
        <div className="prose prose-sm dark:prose-invert max-w-none rounded-xl border border-zinc-200 dark:border-zinc-800 p-4
          prose-headings:font-semibold prose-headings:text-zinc-800 dark:prose-headings:text-zinc-200
          prose-p:text-zinc-700 dark:prose-p:text-zinc-300
          prose-li:text-zinc-700 dark:prose-li:text-zinc-300">
          <ReactMarkdown>{summary}</ReactMarkdown>
        </div>
      ) : (
        <div className="flex items-center justify-center h-24 text-sm text-zinc-400 dark:text-zinc-600">
          {t("result.summary_hint")}
        </div>
      )}
    </div>
  );
}
