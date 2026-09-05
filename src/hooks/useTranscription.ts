// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

import { ipc, events, type PolishMode } from "@/lib/ipc";
import { describeError, isCancelled } from "@/lib/errors";
import { useStore } from "@/store";
import i18n from "@/i18n";

// Transcribe a single queue item by id. Reads live state via getState() so it's
// safe to call from the batch runner loop (not bound to a render snapshot).
// Resolves on success, throws on failure (the item is also marked "error").
async function transcribeItem(itemId: string) {
  const s = useStore.getState();
  const item = s.queue.find((i) => i.id === itemId);
  if (!item) return;

  s.setIsTranscribing(true);
  // Decoding and, on the first run, the model load precede any transcription
  // percent, so the stage starts at "decoding".
  s.setTranscribePhase("decoding");
  s.setTranscribeDetail(null);
  s.updateQueueItem(itemId, { status: "processing", progress: 0, segments: [], error: null });

  const unlisten = await events.onTranscribeProgress((e) => {
    const st = useStore.getState();
    // Mirrors the backend pipeline stage in the label.
    st.setTranscribePhase(e.phase);
    st.setTranscribeDetail(e.detail);
    st.updateQueueItem(itemId, { progress: e.percent });
    if (e.segment) {
      const cur = st.queue.find((i) => i.id === itemId);
      if (cur) st.updateQueueItem(itemId, { segments: [...cur.segments, e.segment] });
    }
  });

  try {
    // n_threads is left at the backend default (available_parallelism): with full
    // GPU offload it only affects minor CPU-side ggml ops, so it is not exposed.
    const segments = await ipc.transcribe(item.filePath, s.transcribeLang, s.useVad);
    useStore.getState().updateQueueItem(itemId, { status: "done", segments, progress: 100 });
    // A successful run that found nothing usually means a silent or music-only
    // file.
    if (segments.length === 0) useStore.getState().setWarning(i18n.t("warning.no_speech"));
  } catch (err) {
    // Cancellation is a user action: the item is reset rather than marked errored.
    if (isCancelled(err)) {
      useStore.getState().updateQueueItem(itemId, { status: "waiting", progress: 0, segments: [] });
    } else {
      useStore.getState().updateQueueItem(itemId, { status: "error", error: describeError(err) });
    }
    throw err;
  } finally {
    const st = useStore.getState();
    st.setIsTranscribing(false);
    unlisten();
  }
}

export function useTranscription() {
  const store = useStore();

  const start = async () => {
    const item = store.getActiveItem();
    if (!item || store.isTranscribing || store.queueRunning) return;
    try {
      await transcribeItem(item.id);
    } catch (err) {
      if (!isCancelled(err)) store.setError(describeError(err));
    }
  };

  // Processes every "waiting" item in order, one at a time, since there is one
  // whisper context per GPU. An error marks that item and the run continues; a stop
  // reverts the in-flight item to "waiting" and ends the run.
  const runQueue = async () => {
    const s0 = useStore.getState();
    if (s0.queueRunning || s0.isTranscribing) return;
    s0.setQueueRunning(true);
    s0.setStopQueue(false);
    try {
      while (true) {
        const st = useStore.getState();
        if (st.stopQueue) break;
        const next = st.queue.find((i) => i.status === "waiting");
        if (!next) break;
        st.setActiveItemId(next.id);
        try {
          await transcribeItem(next.id);
        } catch (err) {
          // A cancellation ends the batch; transcribeItem has already reset the
          // item to "waiting". A failure is marked "error" and the run continues.
          if (isCancelled(err)) break;
        }
      }
    } finally {
      const st = useStore.getState();
      st.setQueueRunning(false);
      st.setStopQueue(false);
    }
  };

  // Stop the batch: flag it and cancel the file currently being transcribed.
  const stopBatch = async () => {
    useStore.getState().setStopQueue(true);
    await ipc.cancelTranscribe().catch(() => {});
  };

  const cancel = async () => {
    await ipc.cancelTranscribe().catch(() => {});
  };

  const summarize = async () => {
    const item = store.getActiveItem();
    if (!item || !item.segments.length || store.isSummarizing) return;

    store.setIsSummarizing(true);
    store.setSummarizeProgress(0);
    const transcript = item.segments.map((s) => s.text).join(" ");
    const unlisten = await events.onSummarizeProgress((e) =>
      useStore.getState().setSummarizeProgress(e.percent)
    );

    try {
      const summary = await ipc.summarize(transcript, store.summaryMode, store.transcribeLang);
      store.updateQueueItem(item.id, { summary });
    } catch (err) {
      // Cancellation is a user action, not a failure.
      if (!isCancelled(err)) store.setError(describeError(err));
    } finally {
      store.setIsSummarizing(false);
      unlisten();
    }
  };

  // The mode comes from the button that was pressed, so each press runs that pass
  // rather than only configuring the next one.
  const polish = async (mode: PolishMode) => {
    const item = store.getActiveItem();
    if (!item || !item.segments.length || store.isPolishing) return;
    store.setPolishMode(mode);

    store.setIsPolishing(true);
    store.setSummarizeProgress(0);
    const unlisten = await events.onSummarizeProgress((e) =>
      useStore.getState().setSummarizeProgress(e.percent)
    );
    // The model sometimes drops part of a long chunk; the backend keeps the
    // original text for those lines and reports them here.
    const unlistenDegraded = await events.onPolishDegraded((e) =>
      useStore.getState().setWarning(i18n.t("warning.polish_degraded", { lines: e.lines }))
    );

    try {
      const polished = await ipc.polishTranscript(item.segments, mode, store.transcribeLang);
      store.updateQueueItem(item.id, { polished });
      store.setReadableView(true);
    } catch (err) {
      if (!isCancelled(err)) store.setError(describeError(err));
    } finally {
      store.setIsPolishing(false);
      unlisten();
      unlistenDegraded();
    }
  };

  // Cancel a running summarize or polish (shared backend flag).
  const cancelSummarize = async () => {
    await ipc.cancelSummarize().catch(() => {});
  };

  return { start, cancel, runQueue, stopBatch, summarize, polish, cancelSummarize };
}
