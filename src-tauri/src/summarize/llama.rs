// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

use llama_cpp_2::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{params::LlamaModelParams, AddBos, LlamaModel},
    sampling::LlamaSampler,
    TokenToStringError,
};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use super::chunker::split_chars;
use super::language;
use super::lines;
use crate::error::{AppError, AppResult};
use crate::transcribe::Segment;

pub struct SummarizeOptions {
    pub n_threads: i32,
    pub ctx_size: u32,
    pub max_new_tokens: i32,
    pub temp: f32,
    /// 1.0 = off. High values push the model away from words already in context:
    /// wrong for verbatim tasks (polish), acceptable when paraphrasing (summary).
    pub repeat_penalty: f32,
    pub penalty_last_n: i32,
}

impl Default for SummarizeOptions {
    fn default() -> Self {
        Self {
            n_threads: std::thread::available_parallelism()
                .map(|n| n.get().min(8) as i32)
                .unwrap_or(4),
            ctx_size: 8192,
            max_new_tokens: 1024,
            // A high repeat penalty on a short summary penalizes tokens the
            // language reuses heavily, which makes the model code-switch into
            // English mid-sentence. This value is low enough to avoid that and
            // still high enough to stop greedy decoding looping on one phrase.
            temp: 0.5,
            repeat_penalty: 1.15,
            penalty_last_n: 128,
        }
    }
}

/// The llama backend is a process-wide singleton: `LlamaBackend::init()` flips an
/// atomic flag and returns `BackendAlreadyInitialized` on any second call, with
/// only `Drop` clearing it. One backend per operation would therefore break as
/// soon as two LLM operations overlap, so it is initialized once and kept for the
/// life of the process.
fn backend() -> AppResult<&'static LlamaBackend> {
    static BACKEND: OnceLock<Result<LlamaBackend, String>> = OnceLock::new();
    BACKEND
        .get_or_init(|| LlamaBackend::init().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e: &String| AppError::Summarization(e.clone()))
}

/// Loads a GGUF into VRAM. Slow (~3–5 s for Qwen3-4B), hence the cache in
/// `AppState.llm_cache`.
pub fn load_model(model_path: &Path) -> AppResult<LlamaModel> {
    // Offload all layers to the GPU (Vulkan/Metal). llama clamps to the model's
    // layer count, so an over-large value means "everything".
    let model_params = LlamaModelParams::default().with_n_gpu_layers(999);
    LlamaModel::load_from_file(backend()?, model_path, &model_params).map_err(|e| {
        // As in whisper's loader: report a VRAM shortage as such.
        let raw = e.to_string();
        let required = crate::vram::model_file_size_mb(model_path);
        match crate::vram::classify_load_failure(required, &raw) {
            Some((required_mb, free_mb)) => AppError::InsufficientMemory {
                required_mb,
                free_mb,
            },
            None => AppError::Summarization(raw),
        }
    })
}

pub fn summarize(
    model: &LlamaModel,
    prompt_template: &str,
    transcript: &str,
    lang_code: Option<&str>,
    opts: &SummarizeOptions,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u32, u32),
) -> AppResult<String> {
    let backend = backend()?;

    // The output language is pinned by name. Qwen3-4B code-switches into English
    // when the transcript contains stray English (a channel name, say), and a
    // "language of the transcript" rule is not specific enough to prevent it.
    // `lang_code` is what the user selected on the transcribe screen; on auto it is
    // None and the script decides (see `language::resolve`).
    let lang = language::resolve(lang_code, transcript);
    let system = match lang {
        Some(l) => format!("{}\n\n{}", prompt_template.trim(), language_directive(l)),
        None => prompt_template.to_string(),
    };

    // Per-chunk data budget in real tokens: the context window minus the model's
    // output, the system prompt and the ChatML framing. Sizing chunks by tokens
    // rather than characters keeps a prompt from overflowing the decode batch and
    // the KV cache, which aborts the whole run.
    let system_tokens = token_len(model, &system);
    let data_budget = (opts.ctx_size as usize)
        .saturating_sub(opts.max_new_tokens as usize)
        .saturating_sub(system_tokens)
        .saturating_sub(48)
        .max(256);

    let chunks = chunk_by_tokens(model, transcript, data_budget, 200);
    let mut chunk_summaries: Vec<String> = Vec::new();

    // Progress covers the map phase (one step per chunk), which dominates the
    // wall-clock time; the combine/cleanup is the last step.
    let total = chunks.len();
    let steps = (total + 1) as u32; // chunks + one combine/cleanup step
    for (i, chunk) in chunks.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }
        progress(i as u32, steps);
        let user = format!("[Part {} of {}]\n\n{}", i + 1, total, chunk);
        let summary = generate(backend, model, &system, &user, opts, cancel)?;
        chunk_summaries.push(summary);
    }

    progress(total as u32, steps);

    // Combined in passes that each stay within the budget: a single join of many
    // chunk summaries would overflow the context like an oversized chunk.
    let mut summary = reduce_summaries(
        backend,
        model,
        &system,
        chunk_summaries,
        data_budget,
        opts,
        cancel,
    )?;

    // Prompt-level rules don't fully stop Qwen3-4B from dropping the odd English
    // word into a non-English summary. Stray Latin words are detected
    // deterministically and passed to one focused corrector run that replaces only
    // those words; its result is kept only if it reduced the count.
    //
    // Only for languages not written in the Latin script: elsewhere Latin letters
    // are the language's own and finding them says nothing.
    if let Some(target) = lang.filter(|l| !language::uses_latin_script(l)) {
        let strays = latin_words(&summary);
        if !strays.is_empty() {
            if let Ok(fixed) = fix_foreign_words(backend, model, &summary, &strays, target, cancel)
            {
                if latin_words(&fixed).len() < strays.len() {
                    summary = fixed;
                }
            }
        }
    }

    progress(steps, steps);
    Ok(summary)
}

/// Number of tokens `text` produces with this model's tokenizer (no BOS). Chunks
/// are sized by real tokens because the char≈token ratio differs ~2-3× between
/// English and Russian. Falls back to a char estimate if tokenization fails.
fn token_len(model: &LlamaModel, text: &str) -> usize {
    model
        .str_to_token(text, AddBos::Never)
        .map(|t| t.len())
        .unwrap_or_else(|_| text.chars().count() / 3)
}

/// Splits `text` into chunks of at most `target_tokens` tokens, with
/// `overlap_tokens` shared between neighbours. Measures the text's token density
/// once and converts the token budget into the char budget the windowing uses,
/// with a safety factor for denser-than-average regions.
fn chunk_by_tokens(
    model: &LlamaModel,
    text: &str,
    target_tokens: usize,
    overlap_tokens: usize,
) -> Vec<String> {
    let n_tokens = token_len(model, text);
    let n_chars = text.chars().count();
    if n_tokens == 0 || n_chars == 0 || n_tokens <= target_tokens {
        return vec![text.to_string()];
    }
    let chars_per_token = n_chars as f64 / n_tokens as f64;
    let chunk_chars = (target_tokens as f64 * chars_per_token * 0.85) as usize;
    let overlap_chars = (overlap_tokens as f64 * chars_per_token) as usize;
    split_chars(text, chunk_chars.max(1), overlap_chars)
}

/// Map-reduce the per-chunk summaries down to one, combining in passes that each
/// stay within `data_budget` tokens. Terminates because every chunk summary is
/// bounded by `max_new_tokens`, which the budget formula keeps well below
/// `data_budget`, so each pass strictly shrinks the list.
fn reduce_summaries(
    backend: &LlamaBackend,
    model: &LlamaModel,
    system: &str,
    mut summaries: Vec<String>,
    data_budget: usize,
    opts: &SummarizeOptions,
    cancel: &AtomicBool,
) -> AppResult<String> {
    loop {
        if summaries.len() <= 1 {
            return Ok(summaries.pop().unwrap_or_default());
        }

        let mut next: Vec<String> = Vec::new();
        let mut group: Vec<String> = Vec::new();
        let mut group_tokens = 0usize;

        for s in summaries {
            let t = token_len(model, &s);
            if !group.is_empty() && group_tokens + t > data_budget {
                next.push(combine_group(backend, model, system, &group, opts, cancel)?);
                group.clear();
                group_tokens = 0;
            }
            group_tokens += t;
            group.push(s);
        }
        if !group.is_empty() {
            next.push(combine_group(backend, model, system, &group, opts, cancel)?);
        }

        summaries = next;
    }
}

/// Combines one group of summaries into a single summary. A group of one passes
/// through unchanged rather than being paraphrased again.
fn combine_group(
    backend: &LlamaBackend,
    model: &LlamaModel,
    system: &str,
    group: &[String],
    opts: &SummarizeOptions,
    cancel: &AtomicBool,
) -> AppResult<String> {
    if group.len() == 1 {
        return Ok(group[0].clone());
    }
    let combined = group.join("\n\n---\n\n");
    let user = format!("[Combined chunk summaries — produce the final summary]\n\n{combined}");
    generate(backend, model, system, &user, opts, cancel)
}

fn language_directive(lang: &str) -> String {
    format!(
        "IMPORTANT: The transcript's main language is {lang}. Write the ENTIRE summary in {lang} only. \
         The transcript may contain a few foreign words or names (e.g. a channel or brand name); ignore their \
         language — still write every word, including such names, in {lang} (transliterate if needed). \
         Do not switch languages even once."
    )
}

/// Distinct runs of >=2 ASCII letters: stray foreign words in Cyrillic text.
fn latin_words(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in text.chars() {
        if c.is_ascii_alphabetic() {
            cur.push(c);
        } else {
            if cur.chars().count() >= 2 {
                out.push(std::mem::take(&mut cur));
            }
            cur.clear();
        }
    }
    if cur.chars().count() >= 2 {
        out.push(cur);
    }
    out.sort();
    out.dedup();
    out
}

/// Focused corrector pass: rewrites a Russian summary replacing only the given
/// stray English words, leaving everything else identical.
fn fix_foreign_words(
    backend: &LlamaBackend,
    model: &LlamaModel,
    text: &str,
    words: &[String],
    lang: &str,
    cancel: &AtomicBool,
) -> AppResult<String> {
    let list = words.join(", ");
    let system = format!(
        "You are a {lang} proofreader. The text below is in {lang} but accidentally contains a few \
         English words: {list}. Replace every such English word with its natural {lang} equivalent, \
         transliterating names of people, places and brands into the {lang} script. \
         Do NOT change anything else — keep the meaning, wording, sentences and structure identical. \
         The result must contain NO Latin letters at all. Output ONLY the corrected {lang} text."
    );
    // The corrector reproduces the whole text, swapping only the stray words, so
    // it needs at least as many tokens as the input: a structured report can be
    // long and a fixed cap would truncate it.
    let needed = token_len(model, text);
    let opts = SummarizeOptions {
        temp: 0.2,
        repeat_penalty: 1.05,
        penalty_last_n: 64,
        max_new_tokens: (needed + needed / 4 + 64) as i32,
        ..SummarizeOptions::default()
    };
    generate(backend, model, &system, text, &opts, cancel)
}

/// What a readability pass produced, plus how much of it had to be thrown away.
pub struct PolishResult {
    pub segments: Vec<Segment>,
    /// Lines whose answer was missing or unusable and kept their original text.
    /// Surfaced in the UI, since unedited lines otherwise look like the pass did
    /// nothing.
    pub rejected_lines: usize,
}

/// Readability pass: re-runs the transcript through the LLM to punctuate and
/// clean up each fragment in place. The result has the same number of segments as
/// the input, in the same order, with the same timecodes; only the text changes.
/// See `lines` for why the pass does not reflow the transcript into paragraphs.
pub fn polish(
    model: &LlamaModel,
    prompt_template: &str,
    segments: &[Segment],
    lang_code: Option<&str>,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u32, u32),
) -> AppResult<PolishResult> {
    if segments.is_empty() {
        return Ok(PolishResult {
            segments: Vec::new(),
            rejected_lines: 0,
        });
    }
    let backend = backend()?;

    // Output ≈ input length, hence the large token allowance. A low temperature
    // and a weak repeat penalty keep the model reproducing the text instead of
    // rewording it.
    let opts = SummarizeOptions {
        max_new_tokens: 4096,
        temp: 0.2,
        repeat_penalty: 1.05,
        penalty_last_n: 64,
        ..SummarizeOptions::default()
    };

    // Polish reproduces the whole chunk plus punctuation, so its output is
    // slightly longer than its input. The input is bounded by the context (input
    // plus output must fit the KV cache) and by the output budget (so the model
    // can't run out of `max_new_tokens` mid-chunk and drop the tail). Chunks must
    // not overlap: the model returns the full text, so an overlap would duplicate
    // it.
    let system_tokens = token_len(model, prompt_template) + 32;
    let kv_budget = (opts.ctx_size as usize)
        .saturating_sub(opts.max_new_tokens as usize)
        .saturating_sub(system_tokens)
        .saturating_sub(48);
    let out_budget = (opts.max_new_tokens as f64 / 1.5) as usize;
    let data_budget = kv_budget.min(out_budget).max(256);

    // The prompt file's first paragraph is the role and input description;
    // everything after it is the task, repeated per chunk.
    let (role, rules) = prompt_template
        .split_once("\n\n")
        .unwrap_or((prompt_template, prompt_template));

    // Naming the language beats the prompt file's "keep the original language":
    // told the transcript is Ukrainian, the model stops drifting into Russian, the
    // nearest language it knows better.
    let lang = language::resolve(lang_code, &join_segments(segments));
    let role = match lang {
        Some(l) => format!(
            "{}\n\nThe transcript is in {l}. Answer in {l}.",
            role.trim()
        ),
        None => role.to_string(),
    };
    // Chinese and Japanese are written without spaces, so the per-line safety net
    // has to measure characters or it cannot tell a gutted answer from a good one.
    let unit = match lang {
        Some(l) if language::is_scriptio_continua(l) => lines::Unit::Chars,
        _ => lines::Unit::Words,
    };

    let chunks = chunk_segments(model, segments, data_budget);
    let steps = chunks.len() as u32;
    let mut out: Vec<Segment> = Vec::with_capacity(segments.len());
    let mut rejected_lines = 0usize;
    for (i, chunk) in chunks.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }
        progress(i as u32, steps);

        let text = lines::number(chunk);
        // The rules go after the transcript rather than into the system turn: a 4B
        // model anchors on the last instruction it read, and with three thousand
        // tokens of speech in between it echoes the input back instead of editing
        // it.
        let answer = generate(
            backend,
            model,
            &role,
            &format!("{text}\n\n---\n\n{rules}"),
            &opts,
            cancel,
        )?;

        // Per line, not per chunk: a number the model never answered, or answered
        // with a summary of its own, keeps its original text.
        let (edited, rejected) = lines::apply(chunk, &lines::parse(&answer, chunk.len()), unit);
        if rejected > 0 {
            tracing::warn!(
                "readability pass: {rejected} of {} lines came back unusable — keeping the originals",
                chunk.len(),
            );
        }
        rejected_lines += rejected;
        out.extend(edited);
    }
    progress(steps, steps);
    Ok(PolishResult {
        segments: out,
        rejected_lines,
    })
}

/// The chunk's speech as one string, used only to measure token density. What the
/// model receives is the numbered list built by `lines::number`.
fn join_segments(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|s| s.text.trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A model mirroring a numbered list loses count on long ones: past a few dozen
/// lines it skips numbers or renumbers from 1. Caps the list length on top of its
/// token budget.
const MAX_LINES_PER_CHUNK: usize = 25;

/// Groups segments into chunks of at most `target_tokens`. Cutting on segment
/// boundaries rather than by character count keeps every chunk a whole number of
/// utterances, so the model never punctuates half a sentence and every chunk maps
/// back onto its own timestamps.
fn chunk_segments<'a>(
    model: &LlamaModel,
    segments: &'a [Segment],
    target_tokens: usize,
) -> Vec<&'a [Segment]> {
    let text = join_segments(segments);
    let n_tokens = token_len(model, &text);
    if n_tokens <= target_tokens && segments.len() <= MAX_LINES_PER_CHUNK {
        return vec![segments];
    }
    // As in chunk_by_tokens: measure token density once and work in characters
    // from there, with a safety factor for denser regions.
    let chars_per_token = text.chars().count() as f64 / n_tokens.max(1) as f64;
    let budget = (target_tokens as f64 * chars_per_token * 0.85).max(1.0) as usize;

    let mut out = Vec::new();
    let mut start = 0usize;
    let mut len = 0usize;
    for (i, seg) in segments.iter().enumerate() {
        let seg_chars = seg.text.chars().count() + 1;
        // At least one segment per chunk, however long it is.
        if (len + seg_chars > budget || i - start >= MAX_LINES_PER_CHUNK) && i > start {
            out.push(&segments[start..i]);
            start = i;
            len = 0;
        }
        len += seg_chars;
    }
    if start < segments.len() {
        out.push(&segments[start..]);
    }
    out
}

/// Runs one generation. `system` is the instruction, `user` the data. Qwen3 needs
/// ChatML framing or it emits its (English) chain-of-thought instead of an answer;
/// `/no_think` disables its reasoning mode. The sampler uses a repeat penalty
/// because plain greedy decoding loops on a single phrase indefinitely.
fn generate(
    backend: &LlamaBackend,
    model: &LlamaModel,
    system: &str,
    user: &str,
    opts: &SummarizeOptions,
    cancel: &AtomicBool,
) -> AppResult<String> {
    let prompt = format!(
        "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{} /no_think<|im_end|>\n<|im_start|>assistant\n",
        system.trim(),
        user.trim(),
    );

    // n_batch caps how many tokens one llama_decode call accepts. At the default
    // (2048) a longer prompt trips `GGML_ASSERT(n_tokens <= n_batch)`, which aborts
    // the process. Raised to the full context, so any prompt that fits the KV cache
    // decodes in one call.
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(std::num::NonZeroU32::new(opts.ctx_size))
        .with_n_batch(opts.ctx_size)
        .with_n_threads(opts.n_threads)
        .with_n_threads_batch(opts.n_threads);

    let mut ctx = model
        .new_context(backend, ctx_params)
        .map_err(|e| AppError::Summarization(e.to_string()))?;

    // ChatML carries its own control tokens; AddBos::Never avoids a stray BOS.
    let mut tokens = model
        .str_to_token(&prompt, AddBos::Never)
        .map_err(|e| AppError::Summarization(e.to_string()))?;

    // The prompt plus the generated tokens must fit the KV cache or llama.cpp
    // aborts. Chunk sizing stays well under this, so the truncation below only
    // guards pathological inputs.
    let max_prompt = (opts.ctx_size as usize)
        .saturating_sub(opts.max_new_tokens as usize + 16)
        .max(16);
    if tokens.len() > max_prompt {
        tokens.truncate(max_prompt);
    }

    let n_tokens = tokens.len();
    let mut batch = LlamaBatch::new(n_tokens.max(512), 1);

    for (i, &token) in tokens.iter().enumerate() {
        let is_last = i == n_tokens - 1;
        batch
            .add(token, i as i32, &[0], is_last)
            .map_err(|e| AppError::Summarization(e.to_string()))?;
    }

    ctx.decode(&mut batch)
        .map_err(|e| AppError::Summarization(e.to_string()))?;

    let mut output_tokens = Vec::new();
    let mut n_cur = n_tokens as i32;
    // Repeat penalty + Qwen3's recommended non-thinking sampling (temp 0.7,
    // top_p 0.8, top_k 20). Without the penalty the model loops on a phrase.
    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::penalties(opts.penalty_last_n, opts.repeat_penalty, 0.0, 0.0),
        LlamaSampler::top_k(20),
        LlamaSampler::top_p(0.8, 1),
        LlamaSampler::temp(opts.temp),
        LlamaSampler::dist(1234),
    ]);

    loop {
        // Generation is the long-running inner loop, so cancel is checked here.
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }

        let new_token_id = sampler.sample(&ctx, batch.n_tokens() - 1);

        if model.is_eog_token(new_token_id) || n_cur - n_tokens as i32 >= opts.max_new_tokens {
            break;
        }

        output_tokens.push(new_token_id);
        sampler.accept(new_token_id);

        batch.clear();
        batch
            .add(new_token_id, n_cur, &[0], true)
            .map_err(|e| AppError::Summarization(e.to_string()))?;

        n_cur += 1;

        ctx.decode(&mut batch)
            .map_err(|e| AppError::Summarization(e.to_string()))?;
    }

    // Collect raw bytes of every token, then decode the whole buffer at once.
    // Per-token UTF-8 decoding split multi-byte letters (Cyrillic) across token
    // boundaries → � garbage. 64 bytes covers any token; on the rare overflow,
    // retry with the size llama reports.
    let mut bytes = Vec::new();
    for &tok in &output_tokens {
        let piece = match model.token_to_piece_bytes(tok, 64, false, None) {
            Ok(b) => b,
            Err(TokenToStringError::InsufficientBufferSpace(n)) => model
                .token_to_piece_bytes(tok, n.unsigned_abs() as usize, false, None)
                .map_err(|e| AppError::Summarization(e.to_string()))?,
            Err(e) => return Err(AppError::Summarization(e.to_string())),
        };
        bytes.extend_from_slice(&piece);
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();

    // Drop any leftover <think>…</think> block, keep only the real answer.
    let text = match text.split_once("</think>") {
        Some((_, rest)) => rest,
        None => &text,
    };
    Ok(text.trim().to_string())
}
