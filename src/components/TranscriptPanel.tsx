// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ProgressBar } from "@/components/ProgressBar";
import type { Segment, PolishMode } from "@/lib/ipc";

interface Props {
  segments: Segment[];
  /** Readable view: the same segments and timecodes with the text cleaned up. */
  polished: Segment[] | null;
  showReadable: boolean;
  onToggleReadable: () => void;
  /** Which pass produced `polished`; its button is marked as applied. */
  polishMode: PolishMode;
  isPolishing: boolean;
  polishProgress: number;
  onPolish: (mode: PolishMode) => void;
  onCancelPolish: () => void;
}

// Wraps every case-insensitive occurrence of `q` in `text` with a highlight mark.
// `q` must already be lowercased; returns the text unchanged when `q` is empty.
function highlight(text: string, q: string): ReactNode {
  if (!q) return text;
  const lower = text.toLowerCase();
  const parts: ReactNode[] = [];
  let i = 0;
  while (i < text.length) {
    const at = lower.indexOf(q, i);
    if (at === -1) {
      parts.push(text.slice(i));
      break;
    }
    if (at > i) parts.push(text.slice(i, at));
    parts.push(
      <mark key={at} className="bg-yellow-200 dark:bg-yellow-500/40 text-inherit rounded-sm">
        {text.slice(at, at + q.length)}
      </mark>
    );
    i = at + q.length;
  }
  return parts;
}

// Two passes, one button each, so pressing a button always runs that pass.
// "verbatim" keeps the words exactly as spoken; "edited" also fixes agreement and
// case endings, which readers of inflected languages expect but which makes the
// result an edited text rather than a literal record.
const POLISH_MODES: { id: PolishMode; key: string }[] = [
  { id: "verbatim", key: "result.polish_verbatim" },
  { id: "edited", key: "result.polish_edited" },
];

function fmtMs(ms: number): string {
  const h = Math.floor(ms / 3_600_000);
  const m = Math.floor((ms % 3_600_000) / 60_000);
  const s = Math.floor((ms % 60_000) / 1_000);
  const f = Math.floor((ms % 1_000) / 10);
  if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}.${String(f).padStart(2, "0")}`;
}

export function TranscriptPanel({
  segments,
  polished,
  showReadable,
  onToggleReadable,
  polishMode,
  isPolishing,
  polishProgress,
  onPolish,
  onCancelPolish,
}: Props) {
  const { t } = useTranslation();
  const [copiedId, setCopiedId] = useState<number | null>(null);
  const [copyMenuOpen, setCopyMenuOpen] = useState(false);
  const [query, setQuery] = useState("");
  const q = query.trim().toLowerCase();

  if (segments.length === 0) {
    return (
      <div className="flex items-center justify-center h-32 text-sm text-zinc-400 dark:text-zinc-600">
        {t("result.no_transcript")}
      </div>
    );
  }

  // Both views hold the same rows with the same timecodes and differ only in text:
  // the pass edits each fragment in place, leaving the transcript's timings, line
  // breaks and length unchanged.
  const readable = !!polished?.length && showReadable;
  const rows: Segment[] = readable ? polished! : segments;

  // Clicking a row copies that row's text (no timecode).
  const copyRow = (row: Segment, idx: number) => {
    navigator.clipboard.writeText(row.text).catch(() => {});
    setCopiedId(idx);
    setTimeout(() => setCopiedId(null), 1500);
  };

  const copyAll = (withTimestamps: boolean) => {
    const text = rows
      .map((s) => (withTimestamps ? `[${fmtMs(s.t0)} → ${fmtMs(s.t1)}] ${s.text}` : s.text))
      .join("\n");
    navigator.clipboard.writeText(text).catch(() => {});
    setCopyMenuOpen(false);
  };

  const matchCount = q ? rows.filter((s) => s.text.toLowerCase().includes(q)).length : 0;

  return (
    <div className="flex flex-col gap-3">
      {/* Search: placeholder left, non-clickable magnifier pinned right. */}
      <div className="relative">
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t("result.search_placeholder")}
          className="w-full h-9 pl-3 pr-9 rounded-lg border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-900 text-sm text-zinc-800 dark:text-zinc-200 placeholder:text-zinc-400 dark:placeholder:text-zinc-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
        />
        <svg
          width="15" height="15" viewBox="0 0 16 16" fill="none"
          stroke="currentColor" strokeWidth="1.6"
          className="absolute right-3 top-1/2 -translate-y-1/2 text-zinc-400 dark:text-zinc-500 pointer-events-none"
        >
          <circle cx="6.75" cy="6.75" r="4.75" />
          <path d="M10.5 10.5L14 14" strokeLinecap="round" />
        </svg>
      </div>

      <div className="flex items-center justify-between gap-3 flex-wrap">
        <span className="text-xs text-zinc-400 dark:text-zinc-500">
          {rows.length} {t("result.segments")}
        </span>
        <div className="flex items-center gap-3 flex-wrap">
          {/* One button per pass; a ✓ marks the pass the current result came
              from. */}
          {isPolishing ? (
            <button
              onClick={onCancelPolish}
              className="text-xs text-zinc-500 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors"
            >
              {t("controls.cancel")}
            </button>
          ) : (
            POLISH_MODES.map((m) => (
              <button
                key={m.id}
                onClick={() => onPolish(m.id)}
                title={t(`result.polish_${m.id}_hint`)}
                className="text-xs text-blue-500 hover:text-blue-600 transition-colors"
              >
                {polished?.length && polishMode === m.id ? "✓ " : ""}
                {t(m.key)}
              </button>
            ))
          )}
          {polished?.length ? (
            <button
              onClick={onToggleReadable}
              className="text-xs text-zinc-500 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors"
            >
              {showReadable ? t("result.show_original") : t("result.show_readable")}
            </button>
          ) : null}
          <div className="relative">
            <button
              onClick={() => setCopyMenuOpen((v) => !v)}
              className="text-xs text-zinc-500 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors"
            >
              {t("result.copy_all")}
            </button>
            {copyMenuOpen && (
              <>
                <div className="fixed inset-0 z-10" onClick={() => setCopyMenuOpen(false)} />
                <div className="absolute right-0 mt-1 z-20 flex flex-col min-w-max rounded-lg border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-900 shadow-lg overflow-hidden">
                  <button
                    onClick={() => copyAll(true)}
                    className="px-3 py-2 text-xs text-left text-zinc-700 dark:text-zinc-300 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors"
                  >
                    {t("result.copy_with_ts")}
                  </button>
                  <button
                    onClick={() => copyAll(false)}
                    className="px-3 py-2 text-xs text-left text-zinc-700 dark:text-zinc-300 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors"
                  >
                    {t("result.copy_without_ts")}
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      </div>

      {isPolishing && (
        <ProgressBar
          percent={polishProgress}
          indeterminate={polishProgress === 0}
          label={t("result.polishing")}
        />
      )}

      {!isPolishing && (q && matchCount === 0 ? (
        <div className="flex items-center justify-center h-24 text-sm text-zinc-400 dark:text-zinc-600">
          {t("result.no_matches")}
        </div>
      ) : (
        <div className="flex flex-col divide-y divide-zinc-100 dark:divide-zinc-800/60 rounded-xl border border-zinc-200 dark:border-zinc-800 overflow-hidden">
          {rows.map((row, idx) => {
            // Case-insensitive filter: hide rows that don't contain the query.
            if (q && !row.text.toLowerCase().includes(q)) return null;
            return (
              <button
                key={idx}
                onClick={() => copyRow(row, idx)}
                title={t("result.copy_line")}
                className="flex items-start gap-3 px-4 py-3 w-full text-left hover:bg-zinc-50 dark:hover:bg-zinc-900/60 transition-colors group cursor-pointer"
              >
                <div className="flex flex-col gap-0.5 shrink-0 pt-0.5">
                  <span className="font-mono text-[11px] tabular-nums text-zinc-400 dark:text-zinc-500 leading-none">
                    {fmtMs(row.t0)}
                  </span>
                  {copiedId === idx && (
                    <span className="text-[10px] text-blue-500">{t("result.copied")}</span>
                  )}
                </div>
                <p className="text-sm text-zinc-800 dark:text-zinc-200 leading-relaxed flex-1">
                  {highlight(row.text, q)}
                </p>
              </button>
            );
          })}
        </div>
      ))}
    </div>
  );
}
