// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

import { useTranslation } from "react-i18next";

interface Props {
  percent: number;
  downloaded?: number;
  total?: number;
  verifying?: boolean;
  label?: string;
  /** Force the sliding indeterminate bar (e.g. while a model loads). */
  indeterminate?: boolean;
  /** Transfer rate for the line under the bar; omit or 0 to hide that line. */
  bytesPerSec?: number;
  /** Seconds left, shown next to the rate; 0 = unknown, so nothing is shown. */
  etaSecs?: number;
}

/**
 * Decimal units, on purpose: every size shown here comes from HuggingFace, which
 * counts in MB and GB, as do the model tables in the README. Formatting the same
 * bytes as MiB under an MB label would show one download as 1.6 GB in one place
 * and 1.5 GB in another.
 */
export function fmtBytes(b: number): string {
  if (b >= 1_000_000_000) return `${(b / 1_000_000_000).toFixed(2)} GB`;
  if (b >= 1_000_000)     return `${(b / 1_000_000).toFixed(1)} MB`;
  if (b >= 1_000)         return `${(b / 1_000).toFixed(0)} KB`;
  return `${b} B`;
}

/** h:mm:ss / m:ss. Digits only, so there are no unit words to translate. */
function fmtDuration(total: number): string {
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

export function ProgressBar({ percent, downloaded, total, verifying, label, indeterminate: forceIndeterminate, bytesPerSec, etaSecs }: Props) {
  const { t } = useTranslation();
  const indeterminate =
    forceIndeterminate ?? (!verifying && total === 0 && (downloaded ?? 0) > 0);
  const pct = Math.min(100, Math.max(0, percent));

  return (
    <div className="flex flex-col gap-1 w-full">
      <div className="flex justify-between items-center text-xs text-zinc-500 dark:text-zinc-400">
        <span>{label ?? ""}</span>
        <span className="tabular-nums">
          {verifying
            ? (downloaded != null && total != null && total > 0
                ? `${fmtBytes(downloaded)} / ${fmtBytes(total)}`
                : "…")
            : downloaded != null && downloaded > 0
              ? total
                ? `${fmtBytes(downloaded)} / ${fmtBytes(total)}`
                : fmtBytes(downloaded)
              : pct > 0
                ? `${Math.round(pct)}%`
                : ""}
        </span>
      </div>

      <div className="h-1.5 bg-zinc-300 dark:bg-zinc-600 rounded-full overflow-hidden">
        {verifying ? (
          /* Amber pulse during the SHA-256 check */
          pct > 0 ? (
            <div
              className="h-full bg-amber-400 rounded-full transition-all duration-200"
              style={{ width: `${pct}%` }}
            />
          ) : (
            <div className="h-full w-1/3 bg-amber-400 rounded-full animate-[slide_1.4s_ease-in-out_infinite]" />
          )
        ) : indeterminate ? (
          <div className="h-full w-1/3 bg-blue-500 rounded-full animate-[slide_1.4s_ease-in-out_infinite]" />
        ) : (
          <div
            className="h-full bg-blue-500 rounded-full transition-all duration-300"
            style={{ width: `${pct}%` }}
          />
        )}
      </div>

      {!verifying && !!bytesPerSec && (
        <div className="flex justify-between items-center text-[10px] text-zinc-400 dark:text-zinc-500 tabular-nums">
          <span>{fmtBytes(bytesPerSec)}/s</span>
          {!!etaSecs && <span>{t("models.eta", { time: fmtDuration(etaSecs) })}</span>}
        </div>
      )}
    </div>
  );
}
