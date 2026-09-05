// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./en";
import zhCN from "./zh-CN";
import fr from "./fr";
import de from "./de";
import ptBR from "./pt-BR";
import ru from "./ru";
import es from "./es";

/** Interface languages, in the order the settings dropdown shows them. */
export const UI_LANGUAGES = [
  { value: "en",    label: "English" },
  { value: "zh-CN", label: "中文（简体）" },
  { value: "fr",    label: "Français" },
  { value: "de",    label: "Deutsch" },
  { value: "pt-BR", label: "Português (Brasil)" },
  { value: "ru",    label: "Русский" },
  { value: "es",    label: "Español" },
] as const;

const resources = {
  en,
  "zh-CN": zhCN,
  fr,
  de,
  "pt-BR": ptBR,
  ru,
  es,
};

/** Maps a stored or system tag that is no longer shipped ("zh", "pt-PT"). */
export function resolveUiLanguage(tag: string | null): string | undefined {
  if (!tag) return undefined;
  const exact = UI_LANGUAGES.find((l) => l.value.toLowerCase() === tag.toLowerCase());
  if (exact) return exact.value;
  const base = tag.split("-")[0].toLowerCase();
  return UI_LANGUAGES.find((l) => l.value.split("-")[0] === base)?.value;
}

i18n.use(initReactI18next).init({
  resources,
  lng: resolveUiLanguage(localStorage.getItem("lang")) ?? "en",
  fallbackLng: "en",
  interpolation: { escapeValue: false },
});

export default i18n;
