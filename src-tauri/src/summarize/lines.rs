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
pub fn plausible(original: &str, edited: &str) -> bool {
    let edited = edited.trim();
    if edited.is_empty() {
        return false;
    }
    let src = original.split_whitespace().count();
    let ans = edited.split_whitespace().count();
    if src == 0 {
        return true;
    }
    let min = (src as f64 * MIN_KEPT_WORDS) as usize;
    let max = (src as f64 * MAX_GROWN_WORDS) as usize + WORD_SLACK;
    ans >= min.saturating_sub(WORD_SLACK) && ans <= max
}

/// Applies the parsed answer to a chunk. Every segment comes back with its own
/// timecodes; the count and the order are those of the input, always.
/// Returns the segments and how many of them had to keep their original text.
pub fn apply(segments: &[Segment], answers: &[Option<String>]) -> (Vec<Segment>, usize) {
    let mut rejected = 0usize;
    let out = segments
        .iter()
        .enumerate()
        .map(|(i, seg)| {
            let text = match answers.get(i).and_then(|a| a.as_deref()) {
                Some(edited) if plausible(&seg.text, edited) => edited.to_string(),
                _ => {
                    rejected += 1;
                    seg.text.clone()
                }
            };
            Segment { t0: seg.t0, t1: seg.t1, text }
        })
        .collect();
    (out, rejected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(t0: i64, t1: i64, text: &str) -> Segment {
        Segment { t0, t1, text: text.to_string() }
    }

    #[test]
    fn numbers_from_one() {
        let s = [seg(0, 1, "привет"), seg(1, 2, "как дела")];
        assert_eq!(number(&s), "1. привет\n2. как дела");
    }

    #[test]
    fn parses_both_numbering_styles() {
        let got = parse("1. Первый.\n2) Второй.", 2);
        assert_eq!(got, vec![Some("Первый.".into()), Some("Второй.".into())]);
    }

    #[test]
    fn glues_wrapped_continuations_back_on() {
        let got = parse("1. Начало строки\nи её продолжение.\n2. Вторая.", 2);
        assert_eq!(got[0].as_deref(), Some("Начало строки и её продолжение."));
    }

    #[test]
    fn ignores_chatter_and_out_of_range_numbers() {
        let got = parse("Here is the edited text:\n1. Готово.\n7. Лишняя.", 2);
        assert_eq!(got, vec![Some("Готово.".into()), None]);
    }

    #[test]
    fn a_missing_number_keeps_the_original_line() {
        let src = [seg(0, 1, "первая строка тут"), seg(1, 2, "вторая строка тут")];
        let (out, rejected) = apply(&src, &parse("1. Первая строка тут.", 2));
        assert_eq!(out.len(), 2);
        assert_eq!(out[1].text, "вторая строка тут");
        assert_eq!(rejected, 1);
    }

    #[test]
    fn timecodes_and_order_always_survive() {
        let src = [seg(0, 1500, "раз"), seg(1500, 3000, "два"), seg(3000, 4200, "три")];
        let (out, _) = apply(&src, &parse("2. Два.\n1. Раз.\n3. Три.", 3));
        assert_eq!(out.iter().map(|s| (s.t0, s.t1)).collect::<Vec<_>>(), vec![(0, 1500), (1500, 3000), (3000, 4200)]);
        assert_eq!(out[1].text, "Два.");
    }

    #[test]
    fn rejects_a_line_the_model_summarized() {
        let long = "мы поехали в город и там было очень много людей на площади";
        assert!(!plausible(long, "Поехали."));
        assert!(plausible(long, "Мы поехали в город, и там было очень много людей на площади."));
    }

    #[test]
    fn rejects_a_line_the_model_expanded_into_its_own_text() {
        assert!(!plausible("ну да", "Да, разумеется, я полностью с вами согласен по этому вопросу."));
    }

    #[test]
    fn filler_only_lines_may_shrink_a_lot() {
        // Short lines get the fixed slack: "э-э, ну" may become "Ну".
        assert!(plausible("э-э ну как бы", "Ну, как бы"));
    }

    #[test]
    fn an_empty_answer_is_never_used() {
        assert!(!plausible("что-то было сказано", "   "));
    }
}
