// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useStore } from "@/store";

// Bottom-right toast. Errors (red) take priority over warnings (amber). Warnings
// auto-dismiss; errors stay until dismissed and can be copied, since an error that
// disappears on its own can no longer be reported.
export function ErrorToast() {
  const { error, warning, setError, setWarning } = useStore();
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  const isError = !!error;
  const text = error ?? warning;

  useEffect(() => {
    // Only warnings time out.
    if (!text || isError) return;
    const id = setTimeout(() => setWarning(null), 6000);
    return () => clearTimeout(id);
  }, [text, isError, setWarning]);

  useEffect(() => {
    setCopied(false);
  }, [text]);

  if (!text) return null;

  const accent = isError
    ? "border-red-200 dark:border-red-900 text-red-500"
    : "border-amber-200 dark:border-amber-900 text-amber-500";

  return (
    <div className="fixed bottom-4 right-4 z-50 max-w-sm">
      <div className={`flex items-start gap-3 bg-white dark:bg-zinc-900 border rounded-xl shadow-lg p-4 ${
        isError ? "border-red-200 dark:border-red-900" : "border-amber-200 dark:border-amber-900"
      }`}>
        {isError ? (
          <svg
            width="16" height="16" viewBox="0 0 16 16" fill="currentColor"
            className={`mt-0.5 shrink-0 ${accent}`}
          >
            <path d="M8 1a7 7 0 1 0 0 14A7 7 0 0 0 8 1zm.75 4.25a.75.75 0 0 0-1.5 0v3.5a.75.75 0 0 0 1.5 0v-3.5zm-.75 6.5a.875.875 0 1 0 0-1.75.875.875 0 0 0 0 1.75z" />
          </svg>
        ) : (
          <svg
            width="16" height="16" viewBox="0 0 16 16" fill="currentColor"
            className={`mt-0.5 shrink-0 ${accent}`}
          >
            <path d="M7.13 2.06a1 1 0 0 1 1.74 0l6 10.5A1 1 0 0 1 14 14H2a1 1 0 0 1-.87-1.44l6-10.5zM8 5.5a.75.75 0 0 0-.75.75v3a.75.75 0 0 0 1.5 0v-3A.75.75 0 0 0 8 5.5zm0 6.25a.875.875 0 1 0 0-1.75.875.875 0 0 0 0 1.75z" />
          </svg>
        )}
        <div className="flex-1 min-w-0">
          <p className="text-sm text-zinc-700 dark:text-zinc-200 leading-snug break-words">{text}</p>
          {isError && (
            <button
              onClick={() => {
                navigator.clipboard.writeText(text ?? "").then(() => setCopied(true)).catch(() => {});
              }}
              className="mt-2 text-xs text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 transition-colors"
            >
              {copied ? t("error.copied") : t("error.copy")}
            </button>
          )}
        </div>
        <button
          onClick={() => (isError ? setError(null) : setWarning(null))}
          aria-label={t("error.dismiss")}
          className="text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 shrink-0 transition-colors"
        >
          <svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor">
            <path d="M2.22 2.22a.75.75 0 0 1 1.06 0L7 5.94l3.72-3.72a.75.75 0 1 1 1.06 1.06L8.06 7l3.72 3.72a.75.75 0 1 1-1.06 1.06L7 8.06l-3.72 3.72a.75.75 0 0 1-1.06-1.06L5.94 7 2.22 3.28a.75.75 0 0 1 0-1.06z" />
          </svg>
        </button>
      </div>
    </div>
  );
}
