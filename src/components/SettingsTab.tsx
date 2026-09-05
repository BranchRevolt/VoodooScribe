// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { useStore, type Theme } from "@/store";
import { ipc } from "@/lib/ipc";
import { describeError } from "@/lib/errors";
import i18n, { UI_LANGUAGES } from "@/i18n";
import { AUDIO_LANGUAGES } from "@/lib/languages";

const THEMES: { value: Theme; key: string }[] = [
  { value: "light",  key: "settings.theme_light" },
  { value: "dark",   key: "settings.theme_dark" },
  { value: "system", key: "settings.theme_system" },
];

const SECTION = "flex flex-col gap-4 pb-6 border-b border-zinc-200 dark:border-zinc-800 last:border-0 last:pb-0";
const LABEL   = "text-xs font-semibold text-zinc-500 dark:text-zinc-400 uppercase tracking-wider mb-3";
const ROW     = "flex items-center justify-between gap-4";
const KEY_TXT = "text-sm text-zinc-700 dark:text-zinc-300";
const SELECT  = "h-8 px-2.5 rounded-lg border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-900 text-sm text-zinc-800 dark:text-zinc-200 focus:outline-none focus:ring-1 focus:ring-blue-500 appearance-none cursor-pointer";

export function SettingsTab() {
  const { t } = useTranslation();
  const store = useStore();

  const changeUiLang = (lang: string) => {
    store.setUiLanguage(lang);
    i18n.changeLanguage(lang);
  };

  const [modelsDir, setModelsDir] = useState<string>("");

  useEffect(() => {
    ipc.getModelsDir().then(setModelsDir).catch((e) => store.setError(describeError(e)));
  }, []);

  const changeModelsDir = async () => {
    try {
      const picked = await open({ directory: true, multiple: false });
      if (typeof picked !== "string") return; // cancelled
      const applied = await ipc.setModelsDir(picked);
      setModelsDir(applied);
      // Re-scan: the loaded models came from the old dir.
      const status = await ipc.getModelsStatus();
      store.setModelsStatus(status);
    } catch (e) {
      store.setError(describeError(e));
    }
  };

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-lg mx-auto px-6 py-6 flex flex-col gap-6">

        {/* Appearance */}
        <section className={SECTION}>
          <h2 className={LABEL}>{t("settings.appearance")}</h2>

          {/* Theme */}
          <div className={ROW}>
            <span className={KEY_TXT}>{t("settings.theme")}</span>
            <div className="flex rounded-lg border border-zinc-200 dark:border-zinc-700 overflow-hidden">
              {THEMES.map(({ value, key }) => (
                <button
                  key={value}
                  onClick={() => store.setTheme(value)}
                  className={`px-3 h-8 text-xs font-medium transition-colors ${
                    store.theme === value
                      ? "bg-zinc-900 dark:bg-white text-white dark:text-zinc-900"
                      : "bg-white dark:bg-zinc-900 text-zinc-600 dark:text-zinc-400 hover:bg-zinc-100 dark:hover:bg-zinc-800"
                  }`}
                >
                  {t(key)}
                </button>
              ))}
            </div>
          </div>

          {/* UI language */}
          <div className={ROW}>
            <span className={KEY_TXT}>{t("settings.ui_lang")}</span>
            <select
              className={SELECT}
              value={store.uiLanguage}
              onChange={(e) => changeUiLang(e.target.value)}
            >
              {UI_LANGUAGES.map((l) => (
                <option key={l.value} value={l.value}>{l.label}</option>
              ))}
            </select>
          </div>
        </section>

        {/* Transcription */}
        <section className={SECTION}>
          <h2 className={LABEL}>{t("settings.transcription")}</h2>

          {/* Audio language */}
          <div className={ROW}>
            <span className={KEY_TXT}>{t("settings.trans_lang")}</span>
            <select
              className={SELECT}
              value={store.transcribeLang ?? ""}
              onChange={(e) => store.setTranscribeLang(e.target.value || null)}
            >
              <option value="">{t("settings.trans_lang_auto")}</option>
              {AUDIO_LANGUAGES.map((l) => (
                <option key={l.value} value={l.value}>
                  {l.label}
                </option>
              ))}
            </select>
          </div>

          {/* VAD default */}
          <div className={ROW}>
            <span className={KEY_TXT}>{t("settings.vad_default")}</span>
            <button
              role="switch"
              aria-checked={store.useVad}
              onClick={() => store.setUseVad(!store.useVad)}
              className={`relative w-9 h-5 rounded-full transition-colors ${
                store.useVad ? "bg-blue-500" : "bg-zinc-200 dark:bg-zinc-700"
              }`}
            >
              <span
                className={`absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white shadow-sm transition-transform ${
                  store.useVad ? "translate-x-4" : "translate-x-0"
                }`}
              />
            </button>
          </div>

        </section>

        {/* Storage */}
        <section className={SECTION}>
          <h2 className={LABEL}>{t("settings.storage")}</h2>
          <div className={ROW}>
            <div className="flex flex-col gap-0.5 min-w-0">
              <span className={KEY_TXT}>{t("settings.models_dir")}</span>
              <span className="text-xs text-zinc-400 dark:text-zinc-600 font-mono truncate" title={modelsDir}>
                {modelsDir || "…"}
              </span>
            </div>
            <button
              onClick={changeModelsDir}
              className="text-xs text-zinc-500 dark:text-zinc-400 hover:text-zinc-800 dark:hover:text-zinc-200 transition-colors whitespace-nowrap"
            >
              {t("settings.change")}
            </button>
          </div>
        </section>
      </div>
    </div>
  );
}
