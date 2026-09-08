// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

//! Line-preserving readability pass.
//!
//! The model receives whisper's fragments as a numbered list and must return the
//! same numbers with the same utterances, cleaned up: one line in, one line out,
//! timecodes untouched. Merging the fragments into paragraphs would read better as
//! prose but drops the per-fragment timecodes, which is what makes a transcript
//! usable.
//!
//! Validation is per line: a line the model dropped has no answer and keeps its
//! original text, and a line rewritten past recognition is rejected on its own
//! without costing the rest of the chunk.

use crate::transcribe::Segment;

/// An edited line may lose this share of its words (filler, stumbles, a repeated
/// word) and still count as the same utterance. Below it, the model dropped
/// speech.
const MIN_KEPT_WORDS: f64 = 0.5;
/// …and may grow by this share (a split fragment gains a word or two, an inflected
/// language gains a preposition). Beyond it, the model wrote its own text.
const MAX_GROWN_WORDS: f64 = 1.8;
/// Short lines are mostly filler and punctuation, where ratios mean little; allow
/// a fixed slack on top of the ratios above.
const WORD_SLACK: usize = 3;
/// The same slack when the unit of measure is characters rather than words, which
/// is roughly the same amount of text.
const CHAR_SLACK: usize = 6;

/// How the length of a line is measured.
///
/// Chinese and Japanese are written without spaces between words, so
/// `split_whitespace` reports one "word" for a whole sentence and every ratio below
/// collapses to 1 against 1 — the check would pass anything, including an answer
/// that dropped half the utterance.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unit {
    Words,
    Chars,
}

impl Unit {
    fn count(self, text: &str) -> usize {
        match self {
            Unit::Words => text.split_whitespace().count(),
            Unit::Chars => text.chars().filter(|c| !c.is_whitespace()).count(),
        }
    }

    fn slack(self) -> usize {
        match self {
            Unit::Words => WORD_SLACK,
            Unit::Chars => CHAR_SLACK,
        }
    }
}

/// Renders the chunk as the numbered list the model is asked to mirror.
pub fn number(segments: &[Segment]) -> String {
    segments
        .iter()
        .enumerate()
        .map(|(i, s)| format!("{}. {}", i + 1, s.text.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Splits `"12. text"` into its index and its text. Returns `None` for a line
/// with no number: a continuation of the previous one, or the model's chatter.
fn numbered(line: &str) -> Option<(usize, &str)> {
    let line = line.trim_start();
    let digits: String = line.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let rest = line[digits.len()..].trim_start();
    // "1." and "1)" are both common; a bare "1 word" is not a number.
    let rest = rest.strip_prefix('.').or_else(|| rest.strip_prefix(')'))?;
    let idx: usize = digits.parse().ok()?;
    if idx == 0 {
        return None;
    }
    Some((idx - 1, rest.trim()))
}

/// Maps the model's answer back onto `expected` slots. A missing slot stays
/// `None` and the caller keeps the original line for it, so a number the model
/// never mentioned costs no speech.
pub fn parse(answer: &str, expected: usize) -> Vec<Option<String>> {
    let mut out: Vec<Option<String>> = vec![None; expected];
    let mut current: Option<usize> = None;

    for raw in answer.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        match numbered(line) {
            Some((idx, text)) => {
                current = (idx < expected).then_some(idx);
                if let Some(i) = current {
                    // A number seen twice: the first answer wins.
                    if out[i].is_none() {
                        out[i] = Some(text.to_string());
                    } else {
                        current = None;
                    }
                }
            }
            // No number: the model wrapped a long line, so glue it back on.
            None => {
                if let Some(i) = current {
                    if let Some(text) = out[i].as_mut() {
                        text.push(' ');
                        text.push_str(line);
                    }
                }
            }
        }
    }
    out
}

/// Whether `edited` is still the same utterance as `original`, rather than a
/// summary, a refusal or invented text. Checked per line, so one bad answer costs
/// one line.
pub fn plausible(original: &str, edited: &str, unit: Unit) -> bool {
    let edited = edited.trim();
    if edited.is_empty() {
        return false;
    }
    let src = unit.count(original);
    let ans = unit.count(edited);
    if src == 0 {
        return true;
    }
    let slack = unit.slack();
    let min = (src as f64 * MIN_KEPT_WORDS) as usize;
    let max = (src as f64 * MAX_GROWN_WORDS) as usize + slack;
    ans >= min.saturating_sub(slack) && ans <= max
}

/// Applies the parsed answer to a chunk. Every segment comes back with its own
/// timecodes; the count and the order are those of the input, always.
/// Returns the segments and how many of them had to keep their original text.
pub fn apply(
    segments: &[Segment],
    answers: &[Option<String>],
    unit: Unit,
) -> (Vec<Segment>, usize) {
    let mut rejected = 0usize;
    let out = segments
        .iter()
        .enumerate()
        .map(|(i, seg)| {
            let text = match answers.get(i).and_then(|a| a.as_deref()) {
                Some(edited) if plausible(&seg.text, edited, unit) => edited.to_string(),
                _ => {
                    rejected += 1;
                    seg.text.clone()
                }
            };
            Segment {
                t0: seg.t0,
                t1: seg.t1,
                text,
            }
        })
        .collect();
    (out, rejected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(t0: i64, t1: i64, text: &str) -> Segment {
        Segment {
            t0,
            t1,
            text: text.to_string(),
        }
    }

    #[test]
    fn numbers_from_one() {
        let s = [seg(0, 1, "hello"), seg(1, 2, "how are you")];
        assert_eq!(number(&s), "1. hello\n2. how are you");
    }

    #[test]
    fn parses_both_numbering_styles() {
        let got = parse("1. First.\n2) Second.", 2);
        assert_eq!(got, vec![Some("First.".into()), Some("Second.".into())]);
    }

    #[test]
    fn glues_wrapped_continuations_back_on() {
        let got = parse("1. Start of the line\nand its continuation.\n2. Second.", 2);
        assert_eq!(
            got[0].as_deref(),
            Some("Start of the line and its continuation.")
        );
    }

    #[test]
    fn ignores_chatter_and_out_of_range_numbers() {
        let got = parse("Here is the edited text:\n1. Done.\n7. Spare.", 2);
        assert_eq!(got, vec![Some("Done.".into()), None]);
    }

    #[test]
    fn a_missing_number_keeps_the_original_line() {
        let src = [seg(0, 1, "first line here"), seg(1, 2, "second line here")];
        let (out, rejected) = apply(&src, &parse("1. First line here.", 2), Unit::Words);
        assert_eq!(out.len(), 2);
        assert_eq!(out[1].text, "second line here");
        assert_eq!(rejected, 1);
    }

    #[test]
    fn timecodes_and_order_always_survive() {
        let src = [
            seg(0, 1500, "one"),
            seg(1500, 3000, "two"),
            seg(3000, 4200, "three"),
        ];
        let (out, _) = apply(&src, &parse("2. Two.\n1. One.\n3. Three.", 3), Unit::Words);
        assert_eq!(
            out.iter().map(|s| (s.t0, s.t1)).collect::<Vec<_>>(),
            vec![(0, 1500), (1500, 3000), (3000, 4200)]
        );
        assert_eq!(out[1].text, "Two.");
    }

    #[test]
    fn multi_byte_text_is_counted_by_characters_not_bytes() {
        // The pass runs on transcripts in any language, so the word walk must not
        // assume one byte per character. Multi-byte coverage cannot be written in
        // plain ASCII; umlauts are the shortest fixture that exercises it.
        let src = [seg(0, 1, "grüße wie geht es")];
        let (out, rejected) = apply(&src, &parse("1. Grüße, wie geht es?", 1), Unit::Words);
        assert_eq!(out[0].text, "Grüße, wie geht es?");
        assert_eq!(rejected, 0);
    }

    #[test]
    fn rejects_a_line_the_model_summarized() {
        let long = "we drove into town and there were a great many people on the square";
        assert!(!plausible(long, "We drove.", Unit::Words));
        assert!(plausible(
            long,
            "We drove into town, and there were a great many people on the square.",
            Unit::Words
        ));
    }

    #[test]
    fn rejects_a_line_the_model_expanded_into_its_own_text() {
        assert!(!plausible(
            "well yes",
            "Yes, naturally, I agree with you entirely on this question.",
            Unit::Words
        ));
    }

    #[test]
    fn filler_only_lines_may_shrink_a_lot() {
        // Short lines get the fixed slack: "er, well" may become "Well".
        assert!(plausible("er well sort of", "Well, sort of", Unit::Words));
    }

    #[test]
    fn a_dropped_utterance_is_caught_in_a_language_without_spaces() {
        // Japanese and Chinese put no spaces between words, so counting words finds
        // one "word" on each side and every answer passes. Counting characters is
        // what makes the check work there at all.
        let long = "そうですねそれはとても難しいプロジェクトでした";
        let gutted = "はい";
        assert!(
            plausible(long, gutted, Unit::Words),
            "words cannot tell these apart"
        );
        assert!(!plausible(long, gutted, Unit::Chars));
        // A real edit of the same line still passes.
        assert!(plausible(
            long,
            "そうですね、それはとても難しいプロジェクトでした。",
            Unit::Chars
        ));
    }

    #[test]
    fn an_empty_answer_is_never_used() {
        assert!(!plausible("something was said", "   ", Unit::Words));
    }
}
