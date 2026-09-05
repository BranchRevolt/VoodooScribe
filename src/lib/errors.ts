// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

import i18n from "@/i18n";

// Shape the Rust backend serializes AppError into (see src-tauri/src/error.rs).
export interface AppErrorPayload {
  code: string;
  message?: string;
  detail?: string | null;
}

/// Turns any rejected-invoke value into a localized message. Uses the
/// `error.<code>` i18n key, interpolating `detail`, and falls back to the
/// backend's English message and then to the raw string.
export function describeError(err: unknown): string {
  if (err && typeof err === "object" && typeof (err as AppErrorPayload).code === "string") {
    const p = err as AppErrorPayload;
    const key = `error.${p.code}`;
    if (i18n.exists(key)) {
      return i18n.t(key, { detail: p.detail ?? "" });
    }
    return p.message ?? p.code;
  }
  return String(err);
}

/// True when a rejected invoke is a user-initiated cancellation.
export function isCancelled(err: unknown): boolean {
  return !!err && typeof err === "object" && (err as AppErrorPayload).code === "cancelled";
}
