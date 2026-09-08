// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

//! Resolves which language the LLM should write in.
//!
//! The transcribe screen already knows the language: the user either picked one or
//! left it on auto. That choice is passed down as an ISO 639-1 code and turned into
//! an English language name here, because the prompts name the target language in
//! plain words ("Write the ENTIRE summary in German only") rather than by code.
//!
//! When the user leaves the language on auto there is no code to pass, and the
//! script of the transcript is the only signal available. A script identifies the
//! language only where it is used by one of the offered languages; Latin covers
//! eight of them, so a Latin transcript yields `None` and the prompt goes out
//! without a language directive rather than with a wrong one.

/// English name of an ISO 639-1 code, for the languages the picker offers.
/// `None` for anything else: whisper accepts ~99 languages through auto-detection,
/// and naming one this table has never seen is worse than saying nothing.
pub fn name_for_code(code: &str) -> Option<&'static str> {
    let code = code.trim().to_ascii_lowercase();
    // Whisper takes bare codes, but a stored setting may carry a region ("pt-BR").
    let base = code.split(['-', '_']).next().unwrap_or("");
    Some(match base {
        "en" => "English",
        "ru" => "Russian",
        "de" => "German",
        "fr" => "French",
        "es" => "Spanish",
        "it" => "Italian",
        "pt" => "Portuguese",
        "nl" => "Dutch",
        "pl" => "Polish",
        "uk" => "Ukrainian",
        "tr" => "Turkish",
        "ar" => "Arabic",
        "zh" => "Chinese",
        "ja" => "Japanese",
        "ko" => "Korean",
        _ => return None,
    })
}

/// Language guessed from the script the transcript is written in, for when the
/// user left the language on auto.
///
/// Only scripts used by exactly one offered language identify it. Latin is used by
/// eight of them, so it yields `None`.
pub fn detect_by_script(text: &str) -> Option<&'static str> {
    let (mut cyrillic, mut han, mut kana, mut hangul, mut arabic) = (0usize, 0, 0, 0, 0);
    // Ukrainian is Cyrillic like Russian; these four letters are in its alphabet
    // and not in the Russian one.
    let mut ukrainian_only = 0usize;

    for c in text.chars() {
        match c {
            'і' | 'ї' | 'є' | 'ґ' | 'І' | 'Ї' | 'Є' | 'Ґ' => {
                ukrainian_only += 1;
                cyrillic += 1;
            }
            '\u{0400}'..='\u{04FF}' => cyrillic += 1,
            // Kana before Han: Japanese text mixes both, Chinese has no kana.
            '\u{3040}'..='\u{30FF}' => kana += 1,
            '\u{4E00}'..='\u{9FFF}' => han += 1,
            '\u{AC00}'..='\u{D7AF}' | '\u{1100}'..='\u{11FF}' => hangul += 1,
            '\u{0600}'..='\u{06FF}' | '\u{0750}'..='\u{077F}' => arabic += 1,
            _ => {}
        }
    }

    let scripts = [
        (kana, "Japanese"),
        (hangul, "Korean"),
        (arabic, "Arabic"),
        (cyrillic, "Russian"),
        (han, "Chinese"),
    ];
    let (count, name) = scripts.into_iter().max_by_key(|(n, _)| *n)?;
    if count == 0 {
        return None;
    }
    if name == "Russian" && ukrainian_only > 0 {
        return Some("Ukrainian");
    }
    Some(name)
}

/// Resolves the language to write in: the user's pick when there is one, otherwise
/// whatever the script gives away.
pub fn resolve(code: Option<&str>, text: &str) -> Option<&'static str> {
    code.and_then(name_for_code)
        .or_else(|| detect_by_script(text))
}

/// Whether a language is written in the Latin script.
///
/// Used to decide whether a stray-Latin-word check makes sense: in a Latin-script
/// language, Latin letters are the norm and finding them means nothing.
pub fn uses_latin_script(lang: &str) -> bool {
    matches!(
        lang,
        "English"
            | "German"
            | "French"
            | "Spanish"
            | "Italian"
            | "Portuguese"
            | "Dutch"
            | "Polish"
            | "Turkish"
    )
}

/// Whether a language writes without spaces between words, so word counts have to
/// be taken in characters instead.
pub fn is_scriptio_continua(lang: &str) -> bool {
    matches!(lang, "Chinese" | "Japanese")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_offered_code_has_a_name() {
        for code in [
            "en", "ru", "de", "fr", "es", "it", "pt", "nl", "pl", "uk", "tr", "ar", "zh", "ja",
            "ko",
        ] {
            assert!(name_for_code(code).is_some(), "{code} has no name");
        }
        assert_eq!(name_for_code("pt-BR"), Some("Portuguese"));
        assert_eq!(name_for_code("ZH"), Some("Chinese"));
        assert_eq!(name_for_code("sw"), None);
    }

    #[test]
    fn script_identifies_the_unambiguous_languages() {
        assert_eq!(
            detect_by_script("это обычный русский текст"),
            Some("Russian")
        );
        assert_eq!(
            detect_by_script("це звичайний український текст"),
            Some("Ukrainian")
        );
        assert_eq!(detect_by_script("これは日本語の文章です"), Some("Japanese"));
        assert_eq!(detect_by_script("这是一段中文文本"), Some("Chinese"));
        assert_eq!(detect_by_script("이것은 한국어 문장입니다"), Some("Korean"));
        assert_eq!(detect_by_script("هذا نص عربي"), Some("Arabic"));
    }

    #[test]
    fn latin_script_is_not_guessed_at() {
        // Eight offered languages share the Latin script, so no directive is better
        // than naming the wrong one.
        assert_eq!(detect_by_script("Das ist ein deutscher Satz."), None);
        assert_eq!(detect_by_script("This is an English sentence."), None);
        assert_eq!(detect_by_script("   "), None);
    }

    #[test]
    fn the_users_pick_wins_over_the_script() {
        // A Ukrainian recording quoting a Russian passage stays Ukrainian.
        assert_eq!(
            resolve(Some("uk"), "это цитата на русском"),
            Some("Ukrainian")
        );
        // Auto falls back to the script.
        assert_eq!(resolve(None, "это цитата на русском"), Some("Russian"));
        // An unknown code falls back too, rather than poisoning the prompt.
        assert_eq!(resolve(Some("sw"), "これは日本語です"), Some("Japanese"));
    }
}
