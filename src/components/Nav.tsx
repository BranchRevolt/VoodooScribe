// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

import { useTranslation } from "react-i18next";
import { useStore, type AppTab, type Theme } from "@/store";

const TABS: AppTab[] = ["transcribe", "queue", "models", "settings"];

const SunIcon = () => (
  <svg width="15" height="15" viewBox="0 0 15 15" fill="currentColor">
    <path d="M7.5 0a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-1 0v-1a.5.5 0 0 1 .5-.5zm4.743 2.257a.5.5 0 0 1 0 .707l-.707.707a.5.5 0 1 1-.707-.707l.707-.707a.5.5 0 0 1 .707 0zM15 7.5a.5.5 0 0 1-.5.5h-1a.5.5 0 0 1 0-1h1a.5.5 0 0 1 .5.5zm-2.257 4.743a.5.5 0 0 1-.707 0l-.707-.707a.5.5 0 0 1 .707-.707l.707.707a.5.5 0 0 1 0 .707zM7.5 13a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-1 0v-1a.5.5 0 0 1 .5-.5zm-4.743-1.257a.5.5 0 0 1 .707 0l.707.707a.5.5 0 0 1-.707.707l-.707-.707a.5.5 0 0 1 0-.707zM0 7.5a.5.5 0 0 1 .5-.5h1a.5.5 0 0 1 0 1h-1A.5.5 0 0 1 0 7.5zm3.964-4.536a.5.5 0 0 1 0 .707l-.707.707a.5.5 0 0 1-.707-.707l.707-.707a.5.5 0 0 1 .707 0zM7.5 4a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7z" />
  </svg>
);

const MoonIcon = () => (
  <svg width="14" height="14" viewBox="0 0 15 15" fill="currentColor">
    <path d="M2.89 0.907A6.5 6.5 0 0 0 7.5 14c3.59 0 6.5-2.91 6.5-6.5a6.48 6.48 0 0 0-1.03-3.519C11.3 5.587 9.5 7 7.5 7A4.5 4.5 0 0 1 3 2.5c0-.558.099-1.094.278-1.59L2.89.907z" />
  </svg>
);

const SystemIcon = () => (
  <svg width="14" height="14" viewBox="0 0 15 15" fill="currentColor">
    <path d="M1 3.5A1.5 1.5 0 0 1 2.5 2h10A1.5 1.5 0 0 1 14 3.5v7A1.5 1.5 0 0 1 12.5 12H9v1h1a.5.5 0 0 1 0 1H5a.5.5 0 0 1 0-1h1v-1H2.5A1.5 1.5 0 0 1 1 10.5v-7zM2.5 3a.5.5 0 0 0-.5.5v7a.5.5 0 0 0 .5.5h10a.5.5 0 0 0 .5-.5v-7a.5.5 0 0 0-.5-.5h-10z" />
  </svg>
);

const themeIcons: Record<Theme, JSX.Element> = {
  light: <SunIcon />,
  dark: <MoonIcon />,
  system: <SystemIcon />,
};

const themeOrder: Theme[] = ["system", "light", "dark"];

export function Nav() {
  const { activeTab, setActiveTab, theme, setTheme, queue } = useStore();
  const { t } = useTranslation();

  const cycleTheme = () => {
    const next = themeOrder[(themeOrder.indexOf(theme) + 1) % themeOrder.length];
    setTheme(next);
  };

  const queueCount = queue.length;

  return (
    <header className="flex items-stretch border-b border-zinc-200 dark:border-zinc-800 h-11 shrink-0 select-none">
      <nav className="flex items-stretch flex-1">
        {TABS.map((tab) => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            className={`relative flex items-center gap-1.5 px-4 text-sm font-medium transition-colors border-b-2 -mb-px ${
              activeTab === tab
                ? "border-blue-500 text-zinc-900 dark:text-zinc-100"
                : "border-transparent text-zinc-500 dark:text-zinc-400 hover:text-zinc-800 dark:hover:text-zinc-200"
            }`}
          >
            {t(`nav.${tab}`)}
            {tab === "queue" && queueCount > 0 && (
              <span className="flex items-center justify-center h-4 min-w-4 px-1 rounded-full bg-blue-500 text-white text-[10px] font-semibold leading-none">
                {queueCount}
              </span>
            )}
          </button>
        ))}
      </nav>

      <button
        onClick={cycleTheme}
        title={theme}
        className="w-11 flex items-center justify-center text-zinc-500 dark:text-zinc-400 hover:text-zinc-800 dark:hover:text-zinc-200 transition-colors"
      >
        {themeIcons[theme]}
      </button>
    </header>
  );
}
