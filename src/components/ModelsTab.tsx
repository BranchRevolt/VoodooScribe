// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useStore } from "@/store";
import { ipc, events } from "@/lib/ipc";
import { describeError } from "@/lib/errors";
import { ProgressBar, fmtBytes } from "@/components/ProgressBar";
import type { WhisperSize, LlmSize, ModelKind } from "@/lib/ipc";
import { WHISPER_MODELS, LLM_MODELS, type ModelMeta } from "@/lib/models";

const BTN_GREEN = "text-xs px-2.5 py-1 rounded-lg border border-emerald-500 text-emerald-600 dark:text-emerald-400 hover:bg-emerald-50 dark:hover:bg-emerald-950/30 transition-colors disabled:opacity-40 disabled:cursor-not-allowed";
const BTN_RED   = "text-xs px-2.5 py-1 rounded-lg border border-red-300 dark:border-red-800 text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-950/30 transition-colors disabled:opacity-40 disabled:cursor-not-allowed";
const BTN_BLUE  = "text-xs px-2.5 py-1 rounded-lg bg-blue-500 hover:bg-blue-600 text-white transition-colors disabled:opacity-40 disabled:cursor-not-allowed";
const BTN_GHOST = "text-xs px-2.5 py-1 rounded-lg border border-zinc-200 dark:border-zinc-700 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors";

export function ModelsTab() {
  const { t } = useTranslation();
  const store = useStore();

  const [downloading, setDownloading]     = useState<string | null>(null);
  const [pausing, setPausing]             = useState<string | null>(null);
  const [paused, setPaused]               = useState<Set<string>>(new Set());
  // Bytes already on disk for a download that was interrupted, by filename.
  const [partial, setPartial]             = useState<Record<string, number>>({});
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  // Which model each dropdown shows (filename). Null follows the app's own
  // choice, so the card starts on the active model.
  const [pickedWhisper, setPickedWhisper] = useState<string | null>(null);
  const [pickedLlm, setPickedLlm] = useState<string | null>(null);
  const [refreshing, setRefreshing]       = useState(false);

  // The file the current download belongs to, so a late event from a replaced
  // download cannot clear the new one's state.
  const activeRef = useRef<string | null>(null);

  const refreshStatus = async () => {
    setRefreshing(true);
    try {
      const status = await ipc.getModelsStatus();
      store.setModelsStatus(status);

      // A download interrupted by closing the app leaves a `.tmp` the backend
      // resumes from, but the paused state itself lives only in this component.
      // The disk is therefore the source of truth for it after a restart.
      const installed = [...status.installed_whisper, ...status.installed_llm];
      const found: Record<string, number> = {};
      await Promise.all(
        [...WHISPER_MODELS, ...LLM_MODELS]
          .filter((m) => !installed.includes(m.filename))
          .map(async (m) => {
            const st = await ipc.getDownloadStatus(m.filename).catch(() => null);
            if (st && st.partial_bytes > 0) found[m.filename] = st.partial_bytes;
          }),
      );
      // The in-flight download has a `.tmp` too, but it is not paused.
      delete found[activeRef.current ?? ""];
      setPartial(found);
      setPaused(new Set(Object.keys(found)));
    } catch (e) {
      store.setError(describeError(e));
    } finally {
      setRefreshing(false);
    }
  };

  useEffect(() => {
    store.clearDownloadProgress();
    refreshStatus();

    // Every download ends with this event, whether it finished, was cancelled or
    // failed, and it is the only completion signal: polling the file alongside it
    // makes the pause path race between two sources for one fact.
    let unlisten: (() => void) | null = null;
    events.onDownloadDone(async (e) => {
      if (activeRef.current !== null && e.filename !== activeRef.current) return;
      activeRef.current = null;
      store.clearDownloadProgress();
      setDownloading(null);
      setPausing(null);

      if (e.error) {
        store.setError(e.error);
        return;
      }
      if (e.cancelled) {
        // Stopped with a partial `.tmp` on disk: the button becomes "resume" and
        // the bar shows how far it got, read back from the file.
        setPaused((s) => new Set(s).add(e.filename));
        const st = await ipc.getDownloadStatus(e.filename).catch(() => null);
        if (st && st.partial_bytes > 0) {
          setPartial((s) => ({ ...s, [e.filename]: st.partial_bytes }));
        }
        return;
      }
      setPaused((s) => { const n = new Set(s); n.delete(e.filename); return n; });
      refreshStatus();
    }).then((fn) => { unlisten = fn; });

    return () => { unlisten?.(); };
  }, []);

  const recommended   = store.modelsStatus?.recommended_whisper ?? null;
  const activeWhisper = store.selectedWhisperModel ?? recommended ?? "large_v3_turbo";
  const fileOf = (id: string | null) =>
    WHISPER_MODELS.find((m) => m.id === id)?.filename ?? null;
  const activeWhisperFile = fileOf(activeWhisper);
  const recommendedFile   = fileOf(recommended);
  // The backend picks the live LLM (at startup and after every download), so it is
  // read back off the loaded path rather than mirrored here.
  const activeLlm = store.modelsStatus?.llm_path?.split(/[/\\]/).pop() ?? null;
  const vramMb        = store.modelsStatus?.available_vram_mb ?? 0;

  // ── Download lifecycle ──────────────────────────────────────────────────────

  const startDownload = async (filename: string, kind: ModelKind) => {
    if ("whisper" in kind) setPickedWhisper(filename); else setPickedLlm(filename);
    activeRef.current = filename;
    setDownloading(filename);
    setPausing(null);
    setPaused((s) => { const n = new Set(s); n.delete(filename); return n; });

    try {
      await ipc.downloadModel(kind); // returns at once; completion arrives as an event
    } catch (e) {
      activeRef.current = null;
      store.setError(describeError(e));
      setDownloading(null);
    }
  };

  const doPause = async (filename: string) => {
    // Only requests the stop; the done event reports whether the download stopped
    // or finished first.
    setPausing(filename);
    await ipc.cancelDownload().catch((e) => store.setError(describeError(e)));
  };

  const doCancel = async (filename: string) => {
    activeRef.current = null;
    await ipc.cancelAndDelete(filename).catch(() => {});
    setPartial((s) => { const n = { ...s }; delete n[filename]; return n; });
    setPaused((s) => { const n = new Set(s); n.delete(filename); return n; });
    store.clearDownloadProgress();
    setDownloading(null);
    setPausing(null);
  };

  const doDelete = async (filename: string) => {
    await ipc.deleteModel(filename).catch((e) => store.setError(describeError(e)));
    setConfirmDelete(null);
    await refreshStatus();
  };

  // ── Buttons ─────────────────────────────────────────────────────────────────

  const ActionButtons = ({ filename, isInstalled, isActive, kind }: {
    filename: string; isInstalled: boolean; isActive: boolean; kind?: ModelKind;
  }) => {
    const isDown      = downloading === filename;
    const isPaused    = paused.has(filename);
    const isPausing   = pausing === filename;
    const isVerifying = store.downloadProgress[filename]?.phase === "verifying";

    if (isDown || isPaused) {
      return (
        <div className="flex items-center gap-1.5">
          <button
            className={BTN_GREEN}
            disabled={isVerifying || isPausing}
            onClick={isDown
              ? () => doPause(filename)
              : () => kind && startDownload(filename, kind)
            }
          >
            {isDown
              ? (isPausing ? t("models.pausing") : isVerifying ? t("models.verifying") : t("models.pause"))
              : t("models.resume")}
          </button>
          <button
            className={BTN_RED}
            disabled={isVerifying}
            onClick={() => doCancel(filename)}
          >
            {t("controls.cancel")}
          </button>
        </div>
      );
    }

    if (isInstalled) {
      if (confirmDelete === filename) {
        return (
          <div className="flex items-center gap-1.5">
            <span className="text-xs text-zinc-500 dark:text-zinc-400">{t("models.confirm_delete")}</span>
            <button className={BTN_RED}   onClick={() => doDelete(filename)}>{t("models.delete_yes")}</button>
            <button className={BTN_GHOST} onClick={() => setConfirmDelete(null)}>{t("models.delete_no")}</button>
          </div>
        );
      }
      return (
        <div className="flex items-center gap-1.5">
          {kind && !isActive && (
            <button
              className={BTN_GHOST}
              onClick={() => {
                if ("whisper" in kind) store.setSelectedWhisperModel(kind.whisper);
                ipc.selectModel(kind)
                  .then(refreshStatus)
                  .catch((e) => store.setError(describeError(e)));
              }}
            >
              {t("models.use")}
            </button>
          )}
          <button className={BTN_RED} onClick={() => setConfirmDelete(filename)}>
            {t("models.delete")}
          </button>
        </div>
      );
    }

    return (
      <button
        className={BTN_BLUE}
        disabled={!!downloading}
        onClick={() => kind && startDownload(filename, kind)}
      >
        {t("models.download")}
      </button>
    );
  };

  // ── Picker ──────────────────────────────────────────────────────────────────

  /// A dropdown of every shipped model plus a card for the chosen one. A flat list
  /// of eleven rows buries the two or three that matter on a given machine.
  const ModelPicker = ({ models, installed, active, recommended, picked, onPick, kindOf }: {
    models: (ModelMeta & { id: string })[];
    installed: string[];
    /** Filename of the model the app will actually run, if any. */
    active: string | null;
    /** Filename the VRAM check suggests, if any. */
    recommended: string | null;
    picked: string | null;
    onPick: (filename: string) => void;
    kindOf: (id: string) => ModelKind;
  }) => {
    const shown = picked ?? active ?? recommended ?? models[0].filename;
    const m = models.find((x) => x.filename === shown) ?? models[0];

    const isInstalled = installed.includes(m.filename);
    const isActive    = m.filename === active;
    const isRecom     = m.filename === recommended;
    const isDown      = downloading === m.filename;
    const isPaused    = paused.has(m.filename);
    const p           = store.downloadProgress[m.filename];
    // Live progress while the download runs; otherwise the size of a leftover
    // `.tmp` from a previous session.
    const bar = p
      ? { percent: p.percent, downloaded: p.downloaded, total: p.total,
          verifying: p.phase === "verifying",
          bytesPerSec: isDown ? p.bytesPerSec : 0, etaSecs: isDown ? p.etaSecs : 0 }
      : isPaused && partial[m.filename]
        ? { percent: Math.round((partial[m.filename] / m.sizeBytes) * 100),
            downloaded: partial[m.filename], total: m.sizeBytes,
            verifying: false, bytesPerSec: 0, etaSecs: 0 }
        : null;
    // Warn only when the card's size is known; 0 means it couldn't be detected.
    const wontFit     = vramMb > 0 && m.vramMb > vramMb;

    return (
      <div className="flex flex-col gap-2">
        <select
          value={m.filename}
          onChange={(e) => onPick(e.target.value)}
          disabled={!!downloading}
          className="h-9 px-2.5 rounded-lg border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-900 text-sm text-zinc-800 dark:text-zinc-200 focus:outline-none focus:ring-1 focus:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
        >
          {models.map((x) => (
            <option key={x.filename} value={x.filename}>
              {installed.includes(x.filename) ? "✓ " : ""}
              {x.label} · {fmtBytes(x.sizeBytes)}
              {x.filename === recommended ? ` · ${t("models.recommended")}` : ""}
            </option>
          ))}
        </select>

        <div className={`rounded-xl border px-4 py-3 flex items-start gap-3 ${
          isActive && isInstalled
            ? "border-blue-200 dark:border-blue-900 bg-blue-50/50 dark:bg-blue-950/20"
            : "border-zinc-200 dark:border-zinc-800"
        }`}>
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2 flex-wrap">
              <span className="text-sm font-medium text-zinc-800 dark:text-zinc-200">{m.label}</span>
              {isActive && isInstalled && (
                <span className="text-[10px] font-semibold px-1.5 py-0.5 rounded-full bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-400 uppercase tracking-wide">
                  {t("models.active")}
                </span>
              )}
              {isRecom && (
                <span className="text-[10px] font-semibold px-1.5 py-0.5 rounded-full bg-amber-100 dark:bg-amber-900/40 text-amber-700 dark:text-amber-400 uppercase tracking-wide">
                  {t("models.recommended")}
                </span>
              )}
              {isInstalled && (
                <span className="text-[10px] font-semibold px-1.5 py-0.5 rounded-full bg-emerald-100 dark:bg-emerald-900/40 text-emerald-700 dark:text-emerald-400 uppercase tracking-wide">
                  ✓ {t("models.installed")}
                </span>
              )}
              {isPaused && (
                <span className="text-[10px] font-semibold px-1.5 py-0.5 rounded-full bg-zinc-200 dark:bg-zinc-700 text-zinc-600 dark:text-zinc-300 uppercase tracking-wide">
                  {t("models.paused")}
                </span>
              )}
            </div>
            <div className="flex gap-3 mt-0.5 text-xs text-zinc-400 dark:text-zinc-500">
              <span>{t("models.size")} {fmtBytes(m.sizeBytes)}</span>
              <span>VRAM {m.vram}</span>
            </div>
            {wontFit && (
              <p className="mt-1 text-xs text-amber-600 dark:text-amber-400">{t("models.wont_fit")}</p>
            )}
            {bar && (
              <div className="mt-2 flex flex-col gap-0.5">
                <ProgressBar {...bar} />
                {p?.phase === "verifying" && (
                  <span className="text-[10px] text-amber-500 dark:text-amber-400">{t("models.verifying")}…</span>
                )}
              </div>
            )}
          </div>
          <div className="shrink-0 pt-0.5">
            <ActionButtons
              filename={m.filename}
              isInstalled={isInstalled}
              isActive={isActive}
              kind={kindOf(m.id)}
            />
          </div>
        </div>
      </div>
    );
  };

  // ── Render ──────────────────────────────────────────────────────────────────

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-2xl mx-auto px-6 py-6 flex flex-col gap-6">

        {vramMb === 0 && (
          <p className="text-xs text-zinc-400 dark:text-zinc-500">{t("models.no_gpu")}</p>
        )}
        {vramMb > 0 && (
          <p className="text-xs text-zinc-400 dark:text-zinc-500">
            {t("models.vram_detected")}: {vramMb >= 1024 ? `${(vramMb / 1024).toFixed(1)} GB` : `${vramMb} MB`}
          </p>
        )}

        <section className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <h2 className="text-xs font-semibold text-zinc-500 dark:text-zinc-400 uppercase tracking-wider">
              {t("models.whisper_title")}
            </h2>
            <button
              onClick={refreshStatus}
              disabled={refreshing}
              className="text-xs text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200 disabled:opacity-40 transition-colors"
            >
              {refreshing ? "↻ …" : `↻ ${t("models.refresh")}`}
            </button>
          </div>
          <ModelPicker
            models={WHISPER_MODELS}
            installed={store.modelsStatus?.installed_whisper ?? []}
            active={activeWhisperFile}
            recommended={recommendedFile}
            picked={pickedWhisper}
            onPick={setPickedWhisper}
            kindOf={(id) => ({ whisper: id as WhisperSize })}
          />
        </section>

        <section className="flex flex-col gap-3">
          <h2 className="text-xs font-semibold text-zinc-500 dark:text-zinc-400 uppercase tracking-wider">
            {t("models.llm_title")}
          </h2>
          <ModelPicker
            models={LLM_MODELS}
            installed={store.modelsStatus?.installed_llm ?? []}
            active={activeLlm}
            recommended={null}
            picked={pickedLlm}
            onPick={setPickedLlm}
            kindOf={(id) => ({ llama: id as LlmSize })}
          />
        </section>

      </div>
    </div>
  );
}
