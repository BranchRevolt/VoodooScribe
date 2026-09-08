// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

/**
 * Audio languages offered for recognition, shared by the transcribe tab and the
 * settings tab so the two can't drift apart.
 *
 * Whisper knows ~99 languages and picks one itself on "Auto"; this is the
 * shortlist worth pinning by hand. Codes are ISO 639-1, as whisper expects.
 */
export const AUDIO_LANGUAGES = [
  { value: "en", label: "English" },
  { value: "ru", label: "Русский" },
  { value: "de", label: "Deutsch" },
  { value: "fr", label: "Français" },
  { value: "es", label: "Español" },
  { value: "it", label: "Italiano" },
  { value: "pt", label: "Português" },
  { value: "nl", label: "Nederlands" },
  { value: "pl", label: "Polski" },
  { value: "uk", label: "Українська" },
  { value: "tr", label: "Türkçe" },
  { value: "ar", label: "العربية" },
  { value: "zh", label: "中文" },
  { value: "ja", label: "日本語" },
  { value: "ko", label: "한국어" },
] as const;
