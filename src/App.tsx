// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect } from "react";
import { useStore } from "@/store";
import { events } from "@/lib/ipc";
import { Nav } from "@/components/Nav";
import { TranscribeTab } from "@/components/TranscribeTab";
import { QueueTab } from "@/components/QueueTab";
import { ModelsTab } from "@/components/ModelsTab";
import { SettingsTab } from "@/components/SettingsTab";
import { ErrorToast } from "@/components/ErrorToast";
import { Onboarding } from "@/components/Onboarding";
import "@/i18n";

export default function App() {
  const { theme, activeTab, setDownloadProgress, onboarded } = useStore();

  // Global download-progress listener; survives tab switches.
  useEffect(() => {
    const unsub = events.onDownloadProgress(setDownloadProgress);
    return () => { unsub.then((fn) => fn()); };
  }, []);

  useEffect(() => {
    const root = document.documentElement;

    if (theme === "dark") {
      root.classList.add("dark");
      return;
    }
    if (theme === "light") {
      root.classList.remove("dark");
      return;
    }

    // system
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    root.classList.toggle("dark", mq.matches);
    const handler = (e: MediaQueryListEvent) => root.classList.toggle("dark", e.matches);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [theme]);

  return (
    <div className="h-screen flex flex-col bg-white dark:bg-zinc-950 text-zinc-900 dark:text-zinc-100 overflow-hidden">
      <Nav />
      <main className="flex-1 overflow-hidden">
        <div className={activeTab === "transcribe" ? "h-full" : "hidden"}><TranscribeTab /></div>
        <div className={activeTab === "queue"      ? "h-full" : "hidden"}><QueueTab /></div>
        <div className={activeTab === "models"     ? "h-full" : "hidden"}><ModelsTab /></div>
        <div className={activeTab === "settings"   ? "h-full" : "hidden"}><SettingsTab /></div>
      </main>
      {!onboarded && <Onboarding />}
      <ErrorToast />
    </div>
  );
}
