// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";

interface Opts {
  /** Only subscribe while true — prevents multiple mounted drop zones double-handling one drop. */
  enabled?: boolean;
  multiple?: boolean;
  /** Lowercase extensions WITH leading dot, e.g. ".mp3". */
  accept: string[];
}

/**
 * Native Tauri v2 drag-drop. Unlike the browser File API (react-dropzone), the
 * native window event carries absolute paths on disk, which is what the backend
 * needs to open the file. Returns whether a drag is currently over the window.
 */
export function useFileDrop(onFiles: (paths: string[]) => void, opts: Opts): boolean {
  const { enabled = true, multiple = false, accept } = opts;
  const [isOver, setIsOver] = useState(false);

  useEffect(() => {
    if (!enabled) {
      setIsOver(false);
      return;
    }
    let unlisten: (() => void) | undefined;
    let active = true;

    getCurrentWebview()
      .onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === "enter" || p.type === "over") {
          setIsOver(true);
        } else if (p.type === "leave") {
          setIsOver(false);
        } else if (p.type === "drop") {
          setIsOver(false);
          let paths = (p.paths ?? []).filter((path) =>
            accept.some((ext) => path.toLowerCase().endsWith(ext))
          );
          if (!multiple) paths = paths.slice(0, 1);
          if (paths.length) onFiles(paths);
        }
      })
      .then((fn) => {
        if (active) unlisten = fn;
        else fn();
      });

    return () => {
      active = false;
      unlisten?.();
    };
  }, [enabled, multiple, onFiles, accept]);

  return isOver;
}
