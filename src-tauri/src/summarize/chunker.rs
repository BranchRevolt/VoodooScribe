// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

//! Splits text into chunks by character count, snapping cuts to word boundaries.
//!
//! Chunk sizing lives in `llama.rs`: it measures the text's token density (the
//! char≈token ratio differs ~2-3× between English and Russian) and converts a
//! token budget into the `chunk_chars` passed here. This module does the
//! UTF-8-safe windowing only.

/// Window `text` into pieces of at most `chunk_chars` characters, with `overlap_chars`
/// characters shared between consecutive pieces. Cuts snap back to the nearest
/// whitespace so words are never split. Operates on `char`s (never raw byte offsets)
/// so it can't panic on multi-byte UTF-8 (e.g. Cyrillic) text.
pub fn split_chars(text: &str, chunk_chars: usize, overlap_chars: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let chunk_chars = chunk_chars.max(1);
    if chars.len() <= chunk_chars {
        return vec![text.to_string()];
    }
    // Overlap must be strictly smaller than the chunk or the window can't advance.
    let overlap = overlap_chars.min(chunk_chars - 1);

    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < chars.len() {
        let end = (start + chunk_chars).min(chars.len());

        // Snap the end back to a word boundary, but only when there is a later
        // whitespace to snap to and it doesn't land on `start`.
        let mut cut = end;
        if end < chars.len() {
            if let Some(off) = chars[start..end].iter().rposition(|c| c.is_whitespace()) {
                if off > 0 {
                    cut = start + off + 1;
                }
            }
        }

        chunks.push(chars[start..cut].iter().collect());

        if cut >= chars.len() {
            break;
        }

        // Step back by the overlap, while still making forward progress.
        let next = cut.saturating_sub(overlap);
        start = if next > start { next } else { cut };
    }

    chunks
}
