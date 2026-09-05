// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useStore } from "@/store";
import { ipc, events } from "@/lib/ipc";
import { describeError } from "@/lib/errors";
import { ProgressBar, fmtBytes } from "@/components/ProgressBar";
import type { WhisperSize } from "@/lib/ipc";
import { whisperById } from "@/lib/models";


// First-run welcome: gets an empty install to a downloaded model in two clicks.
// Rendered only while `!onboarded`, and sets that flag once the model is installed
// or the screen is skipped.
export function Onboarding() {
  const { t } = useTranslation();
  const store = useStore();
  const status = store.modelsStatus;
  const [downloading, setDownloading] = useState(false);
  // The model this screen started downloading, matched against the done event.
  const activeRef = useRef<string | null>(null);
  const pickedRef = useRef<WhisperSize | null>(null);

  // The same completion signal the models tab uses: emitted for every outcome, so
  // nothing here watches the file system.
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    events.onDownloadDone(async (e) => {
      if (e.filename !== activeRef.current) return;
      activeRef.current = null;
      if (e.error || e.cancelled) {
        setDownloading(false);
        if (e.error) store.setError(e.error);
        return;
      }
      store.setModelsStatus(await ipc.getModelsStatus());
      await ipc.selectModel({ whisper: pickedRef.current! }).catch(() => {});
      store.setOnboarded(true);
    }).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, []);

  const installed = status?.installed_whisper ?? [];

  // First run with a model already present (reinstall, shared models dir):
  // nothing to onboard, so mark it done silently.
  useEffect(() => {
    if (status && installed.length > 0) store.setOnboarded(true);
  }, [status, installed.length]);

  // Status not loaded yet, or a model is already installed: render nothing, the
  // effect above dismisses the screen on the next tick.
  if (!status || installed.length > 0) return null;

  const recommended: WhisperSize =
    status.recommended_whisper && whisperById(status.recommended_whisper as WhisperSize)
      ? (status.recommended_whisper as WhisperSize)
      : "small";
  const model = whisperById(recommended);
  const progress = store.downloadProgress[model.filename];
  const verifying = progress?.phase === "verifying";

  const download = async () => {
    setDownloading(true);
    activeRef.current = model.filename;
    pickedRef.current = recommended;
    try {
      await ipc.downloadModel({ whisper: recommended }); // completion arrives as an event
    } catch (e) {
      activeRef.current = null;
      setDownloading(false);
      store.setError(describeError(e));
    }
  };

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-white/80 dark:bg-zinc-950/85 backdrop-blur-sm p-6">
      <div className="w-full max-w-md rounded-2xl border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xl p-6 flex flex-col gap-5">
        <div className="flex flex-col gap-2">
          <h1 className="text-lg font-semibold text-zinc-900 dark:text-zinc-100">
            {t("onboarding.title")}
          </h1>
          <p className="text-sm text-zinc-500 dark:text-zinc-400 leading-relaxed">
            {t("onboarding.subtitle")}
          </p>
        </div>

        {/* Recommended model */}
        <div className="rounded-xl border border-zinc-200 dark:border-zinc-800 p-4 flex flex-col gap-3">
          <div className="flex items-baseline justify-between gap-3">
            <span className="text-sm font-medium text-zinc-800 dark:text-zinc-200">
              {model.label}
            </span>
            <span className="text-xs text-zinc-400 dark:text-zinc-500 tabular-nums">{fmtBytes(model.sizeBytes)}</span>
          </div>
          <span className="text-xs text-zinc-400 dark:text-zinc-500">{t("onboarding.recommended")}</span>

          {downloading && (
            <ProgressBar
              percent={progress?.percent ?? 0}
              indeterminate={!progress || verifying}
              label={verifying ? t("onboarding.verifying") : t("onboarding.downloading")}
              bytesPerSec={progress?.bytesPerSec}
              etaSecs={progress?.etaSecs}
            />
          )}
        </div>

        <div className="flex items-center justify-between">
          <button
            onClick={() => store.setOnboarded(true)}
            className="text-sm text-zinc-500 dark:text-zinc-400 hover:text-zinc-800 dark:hover:text-zinc-200 transition-colors"
          >
            {t("onboarding.skip")}
          </button>
          <button
            onClick={download}
            disabled={downloading}
            className="h-9 px-4 rounded-lg bg-blue-500 hover:bg-blue-600 disabled:opacity-50 disabled:cursor-not-allowed text-white text-sm font-medium transition-colors"
          >
            {downloading ? t("onboarding.downloading") : t("onboarding.download")}
          </button>
        </div>

        <p className="text-xs text-zinc-400 dark:text-zinc-600 leading-relaxed">
          {t("onboarding.hint")}
        </p>
      </div>
    </div>
  );
}
