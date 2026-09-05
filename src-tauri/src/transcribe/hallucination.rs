// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

//! Post-filter for whisper's silence hallucinations.
//!
//! Whisper was trained on YouTube subtitles, so on silence or noise it emits the
//! most frequent phrases from that data: "Спасибо за просмотр!", "Дякую за
//! перегляд!", "Редактор субтитров А.Синецкая", "Subtitles by the Amara.org
//! community". Each 30s window hallucinates independently, so a quiet stretch
//! repeats the same phrase.
//!
//! whisper.cpp's own no-speech gate rarely catches these: it drops a segment only
//! when `no_speech_prob > no_speech_thold` and `avg_logprobs < logprob_thold`, and
//! the model is confident about this boilerplate, so the second condition fails.
//! The Silero VAD keeps most silence away from the model; this filter removes what
//! still gets through.
//!
//! The filter works on clauses, not whole segments: whisper glues its filler onto
//! real speech from the same window ("Так, дякую за перегляд!" is one segment
//! holding one real word and one hallucination), so the segment can be neither
//! kept nor dropped as a unit. Splitting on `. ! ? … ; ,` and removing only the
//! boilerplate clauses keeps the "Так".

use super::Segment;

/// Phrases whisper produces on silence, already normalized (lowercase, no
/// punctuation, collapsed whitespace, `ё` → `е`). Matched against a whole clause,
/// so real speech containing the same words survives ("спасибо за просмотр
/// записи, коллеги" splits into two clauses, neither of which matches).
const HALLUCINATIONS: &[&str] = &[
    // Russian YouTube subtitle boilerplate
    "спасибо за просмотр",
    "спасибо вам за просмотр",
    "спасибо за просмотр видео",
    "спасибо за просмотр и до новых встреч",
    "спасибо за внимание",
    "продолжение следует",
    "продолжение следует в следующей серии",
    "продолжение в следующем видео",
    "подписывайтесь на канал",
    "подпишитесь на канал",
    "ставьте лайки и подписывайтесь на канал",
    "не забудьте подписаться на канал",
    "до новых встреч",
    // Ukrainian: the same filler
    "дякую за перегляд",
    "дякую вам за перегляд",
    "дякую за перегляд відео",
    "дякую за увагу",
    "дякую всім за перегляд",
    "підписуйтесь на канал",
    "підпишіться на канал",
    "ставте лайки і підписуйтесь на канал",
    "до нових зустрічей",
    "продовження далі",
    // English equivalents
    "thanks for watching",
    "thank you for watching",
    "thanks for watching and see you next time",
    "please subscribe to the channel",
    "please subscribe",
    "subscribe to my channel",
    "blank audio",
];

/// Subtitle credits. The name changes from window to window ("А.Синецкая",
/// "А.Семкин", "DimaTorzok", "the Amara.org community"), so these are matched as
/// prefixes rather than in full. Safe as a prefix rule because no one opens a
/// sentence this way in conversation; the list above stays exact because its
/// phrases are things people do say.
const CREDIT_PREFIXES: &[&str] = &[
    "редактор субтитров",
    "субтитры сделал",
    "субтитры делал",
    "субтитры создавал",
    "субтитры создал",
    "субтитры подготовил",
    "субтитры и перевод",
    "субтитры от",
    "субтитри від",
    "субтитри створив",
    "субтитри зробив",
    "переклад субтитрів",
    "subtitles by",
    "subtitles created by",
    "subtitles provided by",
    "transcription by",
    "transcribed by",
    "amara org",
];

// Deliberately in neither list: bare "спасибо" / "до свидания" / "дякую" / "you" /
// "bye" / "музыка" / "смех". Whisper emits those on silence, but they are also
// ordinary speech, and deleting real speech is worse than leaving filler in.
// Bracketed forms like "(музыка)" are still caught by `is_bracketed_marker`.

/// Clause boundaries. The comma is included because whisper attaches its filler
/// to real speech with one as often as with a full stop.
const CLAUSE_SEPARATORS: &[char] = &['.', '!', '?', '…', ';', ',', '\n'];

/// Lowercases, maps `ё` → `е`, strips everything that isn't a letter or digit,
/// and collapses whitespace, so punctuation and casing can't hide a match.
fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for ch in text.chars().flat_map(|c| c.to_lowercase()) {
        let ch = if ch == 'ё' { 'е' } else { ch };
        if ch.is_alphanumeric() {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            out.push(ch);
        } else {
            pending_space = true;
        }
    }
    out
}

/// True when the whole segment is a non-speech marker like `(музыка)`,
/// `[BLANK_AUDIO]` or `*sighs*`: whisper's own annotation, not spoken words.
fn is_bracketed_marker(text: &str) -> bool {
    let t = text.trim();
    let pairs = [('(', ')'), ('[', ']'), ('{', '}'), ('*', '*'), ('♪', '♪')];
    pairs.iter().any(|&(open, close)| {
        if t.chars().count() < 2 || t.chars().next() != Some(open) || t.chars().last() != Some(close)
        {
            return false;
        }
        // The brackets must wrap the whole segment: "(смеётся) Да, ..." closes
        // mid-text, so it is speech with an aside, not a marker.
        let inner = &t[open.len_utf8()..t.len() - close.len_utf8()];
        !inner.contains(open) && !inner.contains(close)
    })
}

/// True when this normalized clause is subtitle boilerplate.
fn is_boilerplate(normalized: &str) -> bool {
    HALLUCINATIONS.contains(&normalized)
        || CREDIT_PREFIXES.iter().any(|p| normalized.starts_with(p))
}

/// Splits into `(body, separator)` pairs, keeping the original text intact so a
/// surviving clause can be put back exactly as whisper wrote it. The separator
/// holds the punctuation and spacing that followed the body and travels with it,
/// so dropping a clause drops its trailing comma too.
///
/// A full stop only ends a clause when what follows it is not a letter or digit:
/// the invented credits are full of initials ("А.Синецкая"), and treating those
/// dots as boundaries would cut the phrase away from its own prefix.
fn split_clauses(text: &str) -> Vec<(&str, &str)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let ends_clause = |i: usize| -> bool {
        match chars[i].1 {
            '.' => chars
                .get(i + 1)
                .is_none_or(|&(_, next)| !next.is_alphanumeric()),
            ch => CLAUSE_SEPARATORS.contains(&ch),
        }
    };

    let mut out = Vec::new();
    let mut body_start = 0usize;
    let mut sep_start: Option<usize> = None;

    for i in 0..chars.len() {
        let (idx, ch) = chars[i];
        // An open separator run swallows the punctuation and spacing that
        // follows, so "..." or "! " stay one boundary.
        let in_separator = CLAUSE_SEPARATORS.contains(&ch) || ch.is_whitespace();
        match sep_start {
            // A run starts at a boundary mark, never at a plain space: spaces
            // inside a clause belong to the clause.
            None if ends_clause(i) => sep_start = Some(idx),
            Some(start) if !in_separator => {
                out.push((&text[body_start..start], &text[start..idx]));
                body_start = idx;
                sep_start = None;
            }
            _ => {}
        }
    }

    match sep_start {
        Some(start) => out.push((&text[body_start..start], &text[start..])),
        None if body_start < text.len() => out.push((&text[body_start..], "")),
        None => {}
    }
    out
}

/// Removes every boilerplate clause and returns what is left of the text.
/// An empty result means the whole segment was hallucinated.
pub fn strip_hallucinations(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() || is_bracketed_marker(trimmed) {
        return String::new();
    }

    let mut out = String::with_capacity(trimmed.len());
    let mut dropped_any = false;
    for (body, sep) in split_clauses(trimmed) {
        let norm = normalize(body);
        // Punctuation-only fragments carry nothing: whisper emits a lone "." for
        // a stretch it heard no words in.
        if norm.is_empty() || is_boilerplate(&norm) || is_bracketed_marker(body) {
            dropped_any = true;
            continue;
        }
        out.push_str(body);
        out.push_str(sep);
    }

    if !dropped_any {
        // Nothing was boilerplate, so the text is returned exactly as whisper
        // wrote it. A segment ending in a comma continues into the next one and is
        // not a leftover.
        return trimmed.to_string();
    }
    // A clause removed from the end leaves the previous one's comma dangling.
    out.trim_end_matches(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .to_string()
}

/// True when the segment carries no real speech at all.
pub fn is_hallucination(text: &str) -> bool {
    strip_hallucinations(text).is_empty()
}

/// Drops runs of an identical line repeated 3+ times in a row, keeping the first.
/// Such runs are the decoder looping on itself: genuine speech practically never
/// produces three byte-identical consecutive segments.
fn collapse_repeats(segments: Vec<Segment>) -> Vec<Segment> {
    const MIN_RUN: usize = 3;
    let mut out: Vec<Segment> = Vec::with_capacity(segments.len());
    let mut i = 0;
    while i < segments.len() {
        let key = normalize(&segments[i].text);
        let mut j = i + 1;
        while j < segments.len() && normalize(&segments[j].text) == key {
            j += 1;
        }
        let run = j - i;
        let keep = if run >= MIN_RUN { 1 } else { run };
        out.extend(segments[i..i + keep].iter().cloned());
        i = j;
    }
    out
}

/// Strips hallucinated clauses, drops segments left with nothing, and collapses
/// looped repeats. Applied to every transcription result, single-pass and
/// per-window alike.
pub fn clean(segments: Vec<Segment>) -> Vec<Segment> {
    let kept: Vec<Segment> = segments
        .into_iter()
        .filter_map(|mut s| {
            let stripped = strip_hallucinations(&s.text);
            if stripped.is_empty() {
                return None;
            }
            s.text = stripped;
            Some(s)
        })
        .collect();
    collapse_repeats(kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(t0: i64, text: &str) -> Segment {
        Segment { t0, t1: t0 + 1000, text: text.into() }
    }

    #[test]
    fn drops_known_boilerplate() {
        assert!(is_hallucination("Спасибо за просмотр!"));
        assert!(is_hallucination("  спасибо за просмотр  "));
        assert!(is_hallucination("Спасибо за просмотр! Спасибо за просмотр!"));
        assert!(is_hallucination("Субтитры сделал DimaTorzok"));
        assert!(is_hallucination("Thanks for watching!"));
        assert!(is_hallucination("Продолжение следует..."));
    }

    #[test]
    fn drops_credits_whatever_name_they_carry() {
        // The name changes from window to window, so these are prefix matches.
        assert!(is_hallucination("Редактор субтитров А.Синецкая Корректор А.Егорова"));
        assert!(is_hallucination("Редактор субтитров А.Семкин Корректор А.Егорова"));
        assert!(is_hallucination("Субтитры от Кто-То Ещё"));
        assert!(is_hallucination("Субтитри від Іван Іванов"));
        assert!(is_hallucination("Subtitles by the Amara.org community"));
        // Real speech that opens with the same word is not credits.
        assert!(!is_hallucination("Субтитры нам нужны на всех языках."));
    }

    #[test]
    fn drops_markers_and_empties() {
        assert!(is_hallucination(""));
        assert!(is_hallucination("   "));
        // Punctuation-only output over a silent stretch.
        assert!(is_hallucination("."));
        assert!(is_hallucination(" ... "));
        assert!(is_hallucination("—"));
        assert!(is_hallucination("(музыка)"));
        assert!(is_hallucination("[BLANK_AUDIO]"));
        assert!(is_hallucination("*sighs*"));
    }

    #[test]
    fn keeps_bare_words_that_are_also_real_speech() {
        // Common one-word replies. Whisper hallucinates these on silence too, but
        // deleting them would cost real dialogue.
        assert!(!is_hallucination("Спасибо."));
        assert!(!is_hallucination("Дякую!"));
        assert!(!is_hallucination("До свидания."));
        assert!(!is_hallucination("До побачення."));
        assert!(!is_hallucination("Bye."));
        // The full subtitle phrases still go.
        assert!(is_hallucination("Дякую за перегляд!"));
        assert!(is_hallucination("Дякую за увагу."));
    }

    #[test]
    fn strips_filler_glued_onto_real_speech() {
        // One real word and one hallucination in one segment: dropping it loses
        // "Так", keeping it shows the filler.
        assert_eq!(strip_hallucinations("Так, дякую за перегляд!"), "Так");
        assert_eq!(
            strip_hallucinations("Дякую за перегляд! Мову на українську замінюємо."),
            "Мову на українську замінюємо."
        );
        assert_eq!(
            strip_hallucinations("Да, конечно. Спасибо за просмотр! Идём дальше."),
            "Да, конечно. Идём дальше."
        );
    }

    #[test]
    fn keeps_real_speech_containing_the_words() {
        assert!(!is_hallucination("Спасибо за просмотр записи, коллеги."));
        assert!(!is_hallucination("Расскажите о своём опыте работы."));
        assert!(!is_hallucination("Спасибо, что нашли время. Начнём с вашего опыта."));
        // A bracketed aside inside a real sentence is speech, not a marker.
        assert!(!is_hallucination("(смеётся) Да, это был сложный проект."));
        // Untouched text comes back byte-identical, including a segment ending
        // mid-sentence on a comma.
        for text in [
            "Спасибо за просмотр записи, коллеги.",
            "Расскажите о своём опыте работы.",
            "(смеётся) Да, это был сложный проект.",
            "Тепер збереглося, можете одразу по ньому натискати,",
            "Ось, наприклад, людина у роботу вам потрапила,",
        ] {
            assert_eq!(strip_hallucinations(text), text);
        }
    }

    #[test]
    fn collapses_only_long_runs() {
        let looped = vec![
            seg(0, "Спасибо за просмотр!"),
            seg(1000, "Спасибо за просмотр!"),
            seg(2000, "Спасибо за просмотр!"),
            seg(3000, "Реальный текст."),
        ];
        // Boilerplate goes; the real line stays.
        assert_eq!(clean(looped).len(), 1);
        // A repeated bare "Спасибо." is handled by collapsing, not filtering:
        // 3+ identical lines in a row keep one.
        let bare = vec![seg(0, "Спасибо."), seg(1000, "Спасибо."), seg(2000, "Спасибо.")];
        assert_eq!(clean(bare).len(), 1);

        let twice = vec![seg(0, "Да, конечно."), seg(1000, "Да, конечно."), seg(2000, "Дальше.")];
        assert_eq!(collapse_repeats(twice).len(), 3);

        let thrice = vec![
            seg(0, "Да, конечно."),
            seg(1000, "Да, конечно!"),
            seg(2000, "да конечно"),
            seg(3000, "Дальше."),
        ];
        let out = collapse_repeats(thrice);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].t0, 0);
        assert_eq!(out[1].text, "Дальше.");
    }

    #[test]
    fn clean_keeps_the_interview() {
        let segs = vec![
            seg(0, "Спасибо за просмотр!"),
            seg(30_000, "Спасибо за просмотр!"),
            seg(60_000, "Спасибо за просмотр!"),
            seg(120_000, "Здравствуйте, расскажите о себе."),
            seg(125_000, "(музыка)"),
            seg(130_000, "Так, дякую за перегляд!"),
            seg(135_000, "Я работал бэкенд-разработчиком пять лет."),
        ];
        let out = clean(segs);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].text, "Здравствуйте, расскажите о себе.");
        // The filler goes, the real word it was glued to stays.
        assert_eq!(out[1].text, "Так");
        assert_eq!(out[1].t0, 130_000);
        assert_eq!(out[2].t0, 135_000);
    }
}
