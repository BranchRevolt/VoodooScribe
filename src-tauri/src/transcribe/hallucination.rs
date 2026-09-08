// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

//! Post-filter for whisper's silence hallucinations: the YouTube subtitle
//! boilerplate it was trained on and emits when handed silence or noise.
//!
//! whisper.cpp's own no-speech gate does not catch these. It drops a segment only
//! when `no_speech_prob > no_speech_thold` and `avg_logprobs < logprob_thold`, and
//! the model is confident about boilerplate, so the second condition never holds.
//!
//! Matching is per clause, not per segment: whisper glues its filler onto real
//! speech from the same window ("Right, thanks for watching!"), so a segment can
//! be neither kept nor dropped as a unit.

use super::Segment;

/// Phrases whisper produces on silence, already normalized (lowercase, no
/// punctuation, collapsed whitespace, `ё` → `е`). Matched against a whole clause,
/// so real speech containing the same words survives ("thanks for watching the
/// recording, everyone" splits into two clauses, neither of which matches).
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
    // English
    "thanks for watching",
    "thank you for watching",
    "thanks for watching and see you next time",
    "please subscribe to the channel",
    "please subscribe",
    "subscribe to my channel",
    "blank audio",
    // German
    "vielen dank fürs zuschauen",
    "vielen dank für s zuschauen",
    "danke fürs zuschauen",
    "danke für s zuschauen",
    "abonniert den kanal",
    "abonniere den kanal",
    "abonniert unseren kanal",
    // French. Apostrophes normalize to spaces, hence "d avoir", "n oubliez".
    "merci d avoir regardé cette vidéo",
    "merci d avoir regardé",
    "abonnez vous à la chaîne",
    "n oubliez pas de vous abonner",
    // Spanish
    "gracias por ver el video",
    "gracias por ver este video",
    "gracias por vernos",
    "suscríbete al canal",
    "suscríbanse al canal",
    "no olvides suscribirte",
    // Italian
    "grazie per aver guardato il video",
    "grazie per la visione",
    "iscriviti al canale",
    // Portuguese
    "obrigado por assistir",
    "obrigada por assistir",
    "obrigado por assistirem",
    "inscreva se no canal",
    // Dutch
    "bedankt voor het kijken",
    "bedankt voor het bekijken",
    "abonneer je op het kanaal",
    // Polish
    "dziękuję za obejrzenie",
    "dziękuję za oglądanie",
    "subskrybuj kanał",
    // Turkish
    "izlediğiniz için teşekkürler",
    "izlediğiniz için teşekkür ederim",
    "abone olmayı unutmayın",
    "kanala abone olun",
    // Arabic. The translator credit is listed in full: the bare word for
    // "translation" opens plenty of real sentences.
    "شكرا لمشاهدتكم",
    "شكرا على المشاهدة",
    "اشترك في القناة",
    "ترجمة نانسي قنقر",
    // Chinese. No spaces to collapse, so these match as written.
    "感谢观看",
    "谢谢观看",
    "谢谢大家观看",
    "请订阅",
    "订阅我的频道",
    "字幕由amara org社区提供",
    // Japanese
    "ご視聴ありがとうございました",
    "ご視聴ありがとうございます",
    "最後までご視聴いただきありがとうございました",
    "チャンネル登録お願いします",
    "チャンネル登録よろしくお願いします",
    // Korean
    "시청해주셔서 감사합니다",
    "시청해 주셔서 감사합니다",
    "구독과 좋아요 부탁드립니다",
];

/// Subtitle credits. The name changes from window to window ("Stephanie Geiges",
/// "DimaTorzok", "the Amara.org community"), so these are matched as prefixes
/// rather than in full. Safe as a prefix rule because no one opens a
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
    "untertitel von",
    "untertitel im auftrag",
    "untertitelung des",
    "untertitelung im auftrag",
    "sous titres réalisés par",
    "sous titrage",
    "subtítulos realizados por",
    "subtitulado por",
    "sottotitoli e revisione a cura di",
    "sottotitoli a cura di",
    "sottotitoli creati dalla comunità",
    "legendas pela comunidade",
    "legendado por",
    "ondertiteling door",
    "ondertiteld door",
    "napisy stworzone przez",
    "字幕由",
    "字幕製作",
];

// Deliberately in neither list: a bare thanks or farewell in any language, and
// bare "music" / "laughter". Whisper emits those on silence, but they are also
// ordinary speech, and deleting real speech is worse than leaving filler in. Each
// list entry above therefore carries the part that makes it subtitle boilerplate
// ("thanks for watching", not "thanks"), never the greeting alone.
// Bracketed forms like "(music)" are still caught by `is_bracketed_marker`.

/// Clause boundaries. The comma is included because whisper attaches its filler
/// to real speech with one as often as with a full stop.
/// The CJK and Arabic marks are here for the same reason as the Latin ones: those
/// scripts never use `.` or `,`, so without them a Japanese or Arabic segment is a
/// single clause and the filler cannot be separated from real speech.
const CLAUSE_SEPARATORS: &[char] = &[
    '.', '!', '?', '…', ';', ',', '\n', // Latin and Cyrillic
    '。', '、', '！', '？', '，', '；', // CJK
    '،', '؛', '؟', // Arabic
];

/// Lowercases, maps `ё` → `е`, strips everything that isn't a letter or digit,
/// and collapses whitespace, so punctuation and casing can't hide a match.
///
/// Combining marks are dropped rather than treated as separators: lowercasing
/// Turkish `İ` yields `i` plus U+0307, which is not alphanumeric, so counting it as
/// a boundary would split "İzlediğiniz" into two words.
fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for ch in text.chars().flat_map(|c| c.to_lowercase()) {
        let ch = if ch == 'ё' { 'е' } else { ch };
        if matches!(ch, '\u{0300}'..='\u{036F}') {
            continue;
        }
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

/// True when the whole segment is a non-speech marker like `(music)`,
/// `[BLANK_AUDIO]` or `*sighs*`: whisper's own annotation, not spoken words.
fn is_bracketed_marker(text: &str) -> bool {
    let t = text.trim();
    let pairs = [('(', ')'), ('[', ']'), ('{', '}'), ('*', '*'), ('♪', '♪')];
    pairs.iter().any(|&(open, close)| {
        if t.chars().count() < 2 || !t.starts_with(open) || !t.ends_with(close) {
            return false;
        }
        // The brackets must wrap the whole segment: "(laughs) Yes, ..." closes
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
/// the invented credits are full of initials ("A.Smith"), and treating those dots
/// as boundaries would cut the phrase away from its own prefix.
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

    for (i, &(idx, ch)) in chars.iter().enumerate() {
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
        Segment {
            t0,
            t1: t0 + 1000,
            text: text.into(),
        }
    }

    #[test]
    fn drops_known_boilerplate() {
        assert!(is_hallucination("Thanks for watching!"));
        assert!(is_hallucination("  thanks for watching  "));
        assert!(is_hallucination(
            "Thanks for watching! Thanks for watching!"
        ));
        assert!(is_hallucination("Please subscribe to the channel."));
    }

    #[test]
    fn drops_credits_whatever_name_they_carry() {
        // The name changes from window to window, so these are prefix matches.
        assert!(is_hallucination("Subtitles by the Amara.org community"));
        assert!(is_hallucination("Subtitles by DimaTorzok"));
        assert!(is_hallucination("Transcription by Someone Else"));
        assert!(is_hallucination("Untertitel von Stephanie Geiges"));
        // Real speech that opens with the same word is not credits.
        assert!(!is_hallucination(
            "Subtitles are what we need in every language."
        ));
    }

    #[test]
    fn drops_markers_and_empties() {
        assert!(is_hallucination(""));
        assert!(is_hallucination("   "));
        // Punctuation-only output over a silent stretch.
        assert!(is_hallucination("."));
        assert!(is_hallucination(" ... "));
        assert!(is_hallucination("—"));
        assert!(is_hallucination("(music)"));
        assert!(is_hallucination("[BLANK_AUDIO]"));
        assert!(is_hallucination("*sighs*"));
    }

    #[test]
    fn keeps_bare_words_that_are_also_real_speech() {
        // Common one-word replies. Whisper hallucinates these on silence too, but
        // deleting them would cost real dialogue.
        assert!(!is_hallucination("Thanks."));
        assert!(!is_hallucination("Thank you!"));
        assert!(!is_hallucination("Goodbye."));
        assert!(!is_hallucination("Bye."));
        // The full subtitle phrase still goes.
        assert!(is_hallucination("Thank you for watching!"));
    }

    #[test]
    fn covers_every_language_the_picker_offers() {
        assert!(is_hallucination("Спасибо за просмотр!"));
        assert!(is_hallucination("Продолжение следует..."));
        assert!(is_hallucination("Дякую за перегляд!"));
        assert!(is_hallucination("Дякую за увагу."));
        assert!(is_hallucination("Vielen Dank fürs Zuschauen!"));
        assert!(is_hallucination("Merci d\'avoir regardé cette vidéo."));
        assert!(is_hallucination("¡Gracias por ver el video!"));
        assert!(is_hallucination("Grazie per aver guardato il video."));
        assert!(is_hallucination("Obrigado por assistir!"));
        assert!(is_hallucination("Bedankt voor het kijken."));
        assert!(is_hallucination("Dziękuję za obejrzenie."));
        assert!(is_hallucination("İzlediğiniz için teşekkürler."));
        assert!(is_hallucination("شكرا لمشاهدتكم"));
        assert!(is_hallucination("感谢观看"));
        assert!(is_hallucination("ご視聴ありがとうございました"));
        assert!(is_hallucination("시청해주셔서 감사합니다"));

        // Credits carry a name that changes, so they match on prefix.
        assert!(is_hallucination(
            "Редактор субтитров А.Синецкая Корректор А.Егорова"
        ));
        assert!(is_hallucination("Субтитри від Іван Іванов"));
        assert!(is_hallucination(
            "Sous-titres réalisés par la communauté d\'Amara.org"
        ));
        assert!(is_hallucination("Legendas pela comunidade Amara.org"));

        // A bare thank-you in any of them is speech, not boilerplate.
        assert!(!is_hallucination("Спасибо."));
        assert!(!is_hallucination("Дякую!"));
        assert!(!is_hallucination("Danke."));
        assert!(!is_hallucination("Merci !"));
        assert!(!is_hallucination("Gracias."));
        assert!(!is_hallucination("Obrigado."));
        assert!(!is_hallucination("ありがとう"));
        assert!(!is_hallucination("감사합니다"));
    }

    #[test]
    fn splits_clauses_on_cjk_punctuation() {
        // Japanese uses 。and 、 rather than . and , — without them the filler and
        // the real sentence stay one clause and neither can be removed alone.
        let out = strip_hallucinations("そうですね。ご視聴ありがとうございました。");
        assert_eq!(out, "そうですね。");
    }

    #[test]
    fn strips_filler_glued_onto_real_speech() {
        // One real word and one hallucination in one segment: dropping the segment
        // loses the word, keeping it shows the filler.
        assert_eq!(strip_hallucinations("Right, thanks for watching!"), "Right");
        assert_eq!(
            strip_hallucinations("Thanks for watching! Let us move on."),
            "Let us move on."
        );
        assert_eq!(
            strip_hallucinations("Yes, of course. Thanks for watching! Moving on."),
            "Yes, of course. Moving on."
        );
    }

    #[test]
    fn keeps_real_speech_containing_the_words() {
        assert!(!is_hallucination(
            "Thanks for watching the recording, everyone."
        ));
        assert!(!is_hallucination("Tell me about your work experience."));
        assert!(!is_hallucination(
            "Thanks for making the time. Let us start with your background."
        ));
        // A bracketed aside inside a real sentence is speech, not a marker.
        assert!(!is_hallucination("(laughs) Yes, that was a hard project."));
        // Untouched text comes back byte-identical, including a segment ending
        // mid-sentence on a comma.
        for text in [
            "Thanks for watching the recording, everyone.",
            "Tell me about your work experience.",
            "(laughs) Yes, that was a hard project.",
            "It is saved now, you can click straight through to it,",
        ] {
            assert_eq!(strip_hallucinations(text), text);
        }
    }

    #[test]
    fn collapses_only_long_runs() {
        let looped = vec![
            seg(0, "Thanks for watching!"),
            seg(1000, "Thanks for watching!"),
            seg(2000, "Thanks for watching!"),
            seg(3000, "Real text."),
        ];
        // Boilerplate goes; the real line stays.
        assert_eq!(clean(looped).len(), 1);
        // A repeated bare "Thanks." is handled by collapsing, not filtering:
        // 3+ identical lines in a row keep one.
        let bare = vec![
            seg(0, "Thanks."),
            seg(1000, "Thanks."),
            seg(2000, "Thanks."),
        ];
        assert_eq!(clean(bare).len(), 1);

        let twice = vec![
            seg(0, "Yes, of course."),
            seg(1000, "Yes, of course."),
            seg(2000, "Next."),
        ];
        assert_eq!(collapse_repeats(twice).len(), 3);

        // Normalization makes the run match despite casing and punctuation.
        let thrice = vec![
            seg(0, "Yes, of course."),
            seg(1000, "Yes, of course!"),
            seg(2000, "yes of course"),
            seg(3000, "Next."),
        ];
        let out = collapse_repeats(thrice);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].t0, 0);
        assert_eq!(out[1].text, "Next.");
    }

    #[test]
    fn clean_keeps_the_interview() {
        let segs = vec![
            seg(0, "Thanks for watching!"),
            seg(30_000, "Thanks for watching!"),
            seg(60_000, "Thanks for watching!"),
            seg(120_000, "Hello, tell me about yourself."),
            seg(125_000, "(music)"),
            seg(130_000, "Right, thanks for watching!"),
            seg(135_000, "I worked as a backend developer for five years."),
        ];
        let out = clean(segs);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].text, "Hello, tell me about yourself.");
        // The filler goes, the real word it was glued to stays.
        assert_eq!(out[1].text, "Right");
        assert_eq!(out[1].t0, 130_000);
        assert_eq!(out[2].t0, 135_000);
    }
}
