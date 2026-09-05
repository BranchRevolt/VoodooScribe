// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import { useStore, type QueueItem } from "@/store";
import { useTranscription } from "@/hooks/useTranscription";
import { DropZone } from "@/components/DropZone";
import { ProgressBar } from "@/components/ProgressBar";

const STATUS_COLORS: Record<QueueItem["status"], string> = {
  waiting:    "text-zinc-400 dark:text-zinc-500",
  processing: "text-blue-500",
  done:       "text-emerald-500",
  error:      "text-red-500",
};

const STATUS_DOT: Record<QueueItem["status"], string> = {
  waiting:    "bg-zinc-300 dark:bg-zinc-600",
  processing: "bg-blue-500 animate-pulse",
  done:       "bg-emerald-500",
  error:      "bg-red-500",
};

function QueueRow({ item }: { item: QueueItem }) {
  const { t } = useTranslation();
  const { removeFromQueue, setActiveItemId, setActiveTab } = useStore();

  const viewResult = () => {
    setActiveItemId(item.id);
    setActiveTab("transcribe");
  };

  return (
    <div className="flex items-center gap-3 px-4 py-3 group">
      <div className={`w-2 h-2 rounded-full shrink-0 ${STATUS_DOT[item.status]}`} />

      <div className="flex-1 min-w-0">
        <p className="text-sm font-medium text-zinc-800 dark:text-zinc-200 truncate">
          {item.fileName}
        </p>
        {item.status === "processing" && (
          <div className="mt-1.5">
            <ProgressBar percent={item.progress} />
          </div>
        )}
        {item.status === "error" && item.error && (
          <p className="text-xs text-red-500 mt-0.5 truncate">{item.error}</p>
        )}
      </div>

      <span className={`text-xs font-medium shrink-0 ${STATUS_COLORS[item.status]}`}>
        {t(`queue.${item.status}`)}
      </span>

      {item.status === "done" && (
        <button
          onClick={viewResult}
          className="text-xs px-2.5 py-1 rounded-lg border border-zinc-200 dark:border-zinc-700
            hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors shrink-0"
        >
          {t("queue.view")}
        </button>
      )}

      <button
        onClick={() => removeFromQueue(item.id)}
        disabled={item.status === "processing"}
        className="opacity-0 group-hover:opacity-100 text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200
          transition-all disabled:pointer-events-none"
        aria-label="Remove"
      >
        <svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor">
          <path d="M2.22 2.22a.75.75 0 0 1 1.06 0L7 5.94l3.72-3.72a.75.75 0 1 1 1.06 1.06L8.06 7l3.72 3.72a.75.75 0 1 1-1.06 1.06L7 8.06l-3.72 3.72a.75.75 0 0 1-1.06-1.06L5.94 7 2.22 3.28a.75.75 0 0 1 0-1.06z" />
        </svg>
      </button>
    </div>
  );
}

export function QueueTab() {
  const { t } = useTranslation();
  const { queue, addToQueue, setActiveItemId, clearCompleted, activeTab, queueRunning } = useStore();
  const { runQueue, stopBatch } = useTranscription();

  const onFiles = useCallback(
    (paths: string[]) => {
      let lastId = "";
      for (const p of paths) lastId = addToQueue(p);
      if (lastId) setActiveItemId(lastId);
    },
    [addToQueue, setActiveItemId]
  );

  const hasDone = queue.some((i) => i.status === "done" || i.status === "error");
  const waitingCount = queue.filter((i) => i.status === "waiting").length;
  const finishedCount = queue.filter((i) => i.status === "done" || i.status === "error").length;

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-2xl mx-auto px-6 py-6 flex flex-col gap-5">

        {/* Toolbar */}
        <div className="flex items-center gap-3">
          <DropZone onFiles={onFiles} multiple compact enabled={activeTab === "queue"} />
          <div className="flex-1" />
          {queueRunning ? (
            <>
              <span className="text-xs text-zinc-500 dark:text-zinc-400 tabular-nums">
                {t("queue.processing_n", { done: finishedCount, total: queue.length })}
              </span>
              <button
                onClick={stopBatch}
                className="h-8 px-4 rounded-lg border border-zinc-300 dark:border-zinc-700 hover:bg-zinc-100 dark:hover:bg-zinc-800 text-sm transition-colors"
              >
                {t("queue.stop")}
              </button>
            </>
          ) : (
            <>
              {hasDone && (
                <button
                  onClick={clearCompleted}
                  className="text-xs text-zinc-500 dark:text-zinc-400 hover:text-zinc-800 dark:hover:text-zinc-200 transition-colors"
                >
                  {t("queue.clear")}
                </button>
              )}
              {waitingCount > 0 && (
                <button
                  onClick={runQueue}
                  className="h-8 px-4 rounded-lg bg-blue-500 hover:bg-blue-600 text-white text-sm font-medium transition-colors"
                >
                  {t("queue.process")}
                </button>
              )}
            </>
          )}
        </div>

        {/* List */}
        {queue.length === 0 ? (
          <div className="flex flex-col items-center justify-center gap-2 py-16 text-center">
            <p className="text-sm font-medium text-zinc-500 dark:text-zinc-400">
              {t("queue.empty_title")}
            </p>
            <p className="text-xs text-zinc-400 dark:text-zinc-600">{t("queue.empty_hint")}</p>
          </div>
        ) : (
          <div className="rounded-xl border border-zinc-200 dark:border-zinc-800 overflow-hidden divide-y divide-zinc-100 dark:divide-zinc-800/60">
            {queue.map((item) => (
              <QueueRow key={item.id} item={item} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
