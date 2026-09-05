// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { useFileDrop } from "@/hooks/useFileDrop";

// Gates both drag-drop and the file dialog. The list is broad because the ffmpeg
// sidecar decodes nearly anything symphonia can't handle in-process.
const ACCEPT_EXTS = [
  ".mp3", ".mp4", ".wav", ".ogg", ".oga", ".flac", ".m4a", ".m4b",
  ".mkv", ".mka", ".webm", ".aac", ".opus", ".mov", ".avi", ".wma",
  ".amr", ".3gp", ".aiff", ".aif", ".caf", ".wv", ".ac3", ".dts",
  ".mpg", ".mpeg", ".ts", ".flv", ".wmv", ".m4v", ".spx",
];

interface Props {
  onFiles: (paths: string[]) => void;
  multiple?: boolean;
  compact?: boolean;
  /** Subscribe to native drag-drop only when this zone's tab is active. */
  enabled?: boolean;
}

export function DropZone({ onFiles, multiple = false, compact = false, enabled = true }: Props) {
  const { t } = useTranslation();

  const isDragActive = useFileDrop(onFiles, { enabled, multiple, accept: ACCEPT_EXTS });

  const pickFiles = useCallback(async () => {
    const picked = await open({
      multiple,
      directory: false,
      filters: [{ name: "Media", extensions: ACCEPT_EXTS.map((e) => e.slice(1)) }],
    });
    if (!picked) return; // cancelled
    const paths = Array.isArray(picked) ? picked : [picked];
    if (paths.length) onFiles(paths);
  }, [onFiles, multiple]);

  if (compact) {
    return (
      <div
        onClick={pickFiles}
        className={`flex items-center gap-2 px-3 py-2 rounded-lg border border-dashed cursor-pointer transition-colors text-sm ${
          isDragActive
            ? "border-blue-400 bg-blue-50 dark:bg-blue-950/30 text-blue-600"
            : "border-zinc-300 dark:border-zinc-700 text-zinc-500 dark:text-zinc-400 hover:border-blue-400 hover:text-blue-500"
        }`}
      >
        <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor" className="shrink-0">
          <path d="M8 1a.75.75 0 0 1 .75.75v5.5h5.5a.75.75 0 0 1 0 1.5h-5.5v5.5a.75.75 0 0 1-1.5 0v-5.5H1.75a.75.75 0 0 1 0-1.5h5.5V1.75A.75.75 0 0 1 8 1z" />
        </svg>
        <span>{t("queue.add")}</span>
      </div>
    );
  }

  return (
    <div
      onClick={pickFiles}
      className={`flex flex-col items-center justify-center gap-3 rounded-2xl border-2 border-dashed p-12 cursor-pointer transition-colors ${
        isDragActive
          ? "border-blue-400 bg-blue-50 dark:bg-blue-950/20"
          : "border-zinc-200 dark:border-zinc-800 hover:border-blue-400 dark:hover:border-blue-500"
      }`}
    >
      <div className={`w-10 h-10 rounded-xl flex items-center justify-center transition-colors ${
        isDragActive ? "bg-blue-100 dark:bg-blue-900/40" : "bg-zinc-100 dark:bg-zinc-800"
      }`}>
        <svg width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5"
          className={isDragActive ? "text-blue-500" : "text-zinc-400 dark:text-zinc-500"}>
          <path strokeLinecap="round" strokeLinejoin="round"
            d="M3 16.5v.75A.75.75 0 0 0 3.75 18h12.5a.75.75 0 0 0 .75-.75v-.75M10 3v10m0-10L6.5 6.5M10 3l3.5 3.5" />
        </svg>
      </div>
      <div className="text-center">
        <p className={`text-sm font-medium transition-colors ${
          isDragActive ? "text-blue-600 dark:text-blue-400" : "text-zinc-700 dark:text-zinc-300"
        }`}>
          {isDragActive ? t("drop.title") : t("drop.title")}
        </p>
        <p className="text-xs text-zinc-400 dark:text-zinc-500 mt-1">{t("drop.subtitle")}</p>
        <p className="text-xs text-zinc-400 dark:text-zinc-600 mt-2">{t("drop.formats")}</p>
      </div>
    </div>
  );
}
