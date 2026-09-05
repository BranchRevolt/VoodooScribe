// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WhisperSize {
    Tiny,
    Base,
    Small,
    Medium,
    LargeV3TurboQ5,
    LargeV3Turbo,
    LargeV3Q5,
    LargeV3,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LlmSize {
    Qwen3_4B,
    Qwen3_8B,
    Qwen3_14B,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Whisper(WhisperSize),
    Llama(LlmSize),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub kind: ModelKind,
    pub name: String,
    pub filename: String,
    pub url: String,
    pub size_bytes: u64,
    /// GPU VRAM needed at inference time (MB), from whisper.cpp benchmarks.
    pub vram_required_mb: u32,
    /// SHA-256 of the finished file, checked before it is put in place. Taken
    /// from HuggingFace's `lfs.oid`, which is that digest. `None` skips
    /// verification and no entry should need to.
    pub sha256: Option<String>,
}

pub fn all_whisper_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            kind: ModelKind::Whisper(WhisperSize::Tiny),
            name: "Whisper Tiny".into(),
            filename: "ggml-tiny.bin".into(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin".into(),
            size_bytes: 77_691_713,
            vram_required_mb: 390,
            sha256: Some("be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21".into()),
        },
        ModelInfo {
            kind: ModelKind::Whisper(WhisperSize::Base),
            name: "Whisper Base".into(),
            filename: "ggml-base.bin".into(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin".into(),
            size_bytes: 147_951_465,
            vram_required_mb: 500,
            sha256: Some("60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe".into()),
        },
        ModelInfo {
            kind: ModelKind::Whisper(WhisperSize::Small),
            name: "Whisper Small".into(),
            filename: "ggml-small.bin".into(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin".into(),
            size_bytes: 487_601_967,
            vram_required_mb: 1_000,
            sha256: Some("1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b".into()),
        },
        ModelInfo {
            kind: ModelKind::Whisper(WhisperSize::Medium),
            name: "Whisper Medium".into(),
            filename: "ggml-medium.bin".into(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin".into(),
            size_bytes: 1_533_763_059,
            vram_required_mb: 2_600,
            sha256: Some("6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208".into()),
        },
        // Turbo is a distilled decoder: four layers instead of thirty-two. Several
        // times faster than the full v3 and close to it on clean speech, but less
        // accurate on hard audio and non-English languages, which is why the full
        // v3 is listed as well.
        ModelInfo {
            kind: ModelKind::Whisper(WhisperSize::LargeV3TurboQ5),
            name: "Whisper Large v3 Turbo (Q5)".into(),
            filename: "ggml-large-v3-turbo-q5_0.bin".into(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin".into(),
            size_bytes: 574_041_195,
            vram_required_mb: 1_200,
            sha256: Some("394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2".into()),
        },
        ModelInfo {
            kind: ModelKind::Whisper(WhisperSize::LargeV3Turbo),
            name: "Whisper Large v3 Turbo".into(),
            filename: "ggml-large-v3-turbo.bin".into(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin".into(),
            size_bytes: 1_624_555_275,
            // Weights are 1.6 GB; inference needs the KV cache and compute buffers
            // on top. This figure is what the ladder in `vram.rs` compares against.
            vram_required_mb: 2_200,
            sha256: Some("1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69".into()),
        },
        ModelInfo {
            kind: ModelKind::Whisper(WhisperSize::LargeV3Q5),
            name: "Whisper Large v3 (Q5)".into(),
            filename: "ggml-large-v3-q5_0.bin".into(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-q5_0.bin".into(),
            size_bytes: 1_081_140_203,
            vram_required_mb: 2_400,
            sha256: Some("d75795ecff3f83b5faa89d1900604ad8c780abd5739fae406de19f23ecd98ad1".into()),
        },
        ModelInfo {
            kind: ModelKind::Whisper(WhisperSize::LargeV3),
            name: "Whisper Large v3".into(),
            filename: "ggml-large-v3.bin".into(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin".into(),
            size_bytes: 3_095_033_483,
            vram_required_mb: 4_700,
            sha256: Some("64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2".into()),
        },
    ]
}

/// Ordered smallest first, which is also the order the UI lists them in.
pub fn all_llm_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            kind: ModelKind::Llama(LlmSize::Qwen3_4B),
            name: "Qwen3-4B Q4_K_M".into(),
            filename: "Qwen3-4B-Q4_K_M.gguf".into(),
            url: "https://huggingface.co/Qwen/Qwen3-4B-GGUF/resolve/main/Qwen3-4B-Q4_K_M.gguf".into(),
            size_bytes: 2_497_280_256,
            vram_required_mb: 4_000,
            sha256: Some("7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5".into()),
        },
        // Worth its extra 2.5 GB on inflected languages: grammatical agreement and
        // case endings are where the 4B quant is weakest.
        ModelInfo {
            kind: ModelKind::Llama(LlmSize::Qwen3_8B),
            name: "Qwen3-8B Q4_K_M".into(),
            filename: "Qwen3-8B-Q4_K_M.gguf".into(),
            url: "https://huggingface.co/Qwen/Qwen3-8B-GGUF/resolve/main/Qwen3-8B-Q4_K_M.gguf".into(),
            size_bytes: 5_027_783_488,
            vram_required_mb: 6_500,
            sha256: Some("d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785".into()),
        },
        // Every entry must be a Qwen3: `llama::generate` hard-codes ChatML framing
        // and Qwen's `/no_think` switch, so another family would answer with
        // garbage rather than fail loudly.
        ModelInfo {
            kind: ModelKind::Llama(LlmSize::Qwen3_14B),
            name: "Qwen3-14B Q4_K_M".into(),
            filename: "Qwen3-14B-Q4_K_M.gguf".into(),
            url: "https://huggingface.co/Qwen/Qwen3-14B-GGUF/resolve/main/Qwen3-14B-Q4_K_M.gguf".into(),
            size_bytes: 9_001_752_960,
            vram_required_mb: 11_000,
            sha256: Some("500a8806e85ee9c83f3ae08420295592451379b4f8cf2d0f41c15dffeb6b81f0".into()),
        },
    ]
}

/// The LLM assumed when none is installed yet; also what onboarding offers.
pub fn default_llm_model() -> ModelInfo {
    all_llm_models().swap_remove(0)
}

/// Registry entry for a kind, whisper and LLM alike.
pub fn find(kind: &ModelKind) -> Option<ModelInfo> {
    match kind {
        ModelKind::Whisper(_) => all_whisper_models(),
        ModelKind::Llama(_) => all_llm_models(),
    }
    .into_iter()
    .find(|m| &m.kind == kind)
}

/// Registry entry for a file already on disk, whatever kind it is.
pub fn find_by_filename(filename: &str) -> Option<ModelInfo> {
    all_whisper_models()
        .into_iter()
        .chain(all_llm_models())
        .find(|m| m.filename == filename)
}


#[cfg(test)]
mod tests {
    use super::*;

    /// The wire names are part of the IPC contract: the TS `ModelKind` union is
    /// written by hand against these strings.
    #[test]
    fn model_kinds_serialize_to_the_names_the_frontend_expects() {
        let whisper = serde_json::to_string(&ModelKind::Whisper(WhisperSize::LargeV3Turbo)).unwrap();
        assert_eq!(whisper, r#"{"whisper":"large_v3_turbo"}"#);

        let small = serde_json::to_string(&ModelKind::Llama(LlmSize::Qwen3_4B)).unwrap();
        assert_eq!(small, r#"{"llama":"qwen3_4_b"}"#);

        let big = serde_json::to_string(&ModelKind::Llama(LlmSize::Qwen3_8B)).unwrap();
        assert_eq!(big, r#"{"llama":"qwen3_8_b"}"#);

        let q5 = serde_json::to_string(&ModelKind::Whisper(WhisperSize::LargeV3Q5)).unwrap();
        assert_eq!(q5, r#"{"whisper":"large_v3_q5"}"#);
    }

    #[test]
    fn every_registry_entry_is_findable_by_its_own_kind_and_filename() {
        for m in all_whisper_models().into_iter().chain(all_llm_models()) {
            assert_eq!(find(&m.kind).map(|f| f.filename.clone()), Some(m.filename.clone()));
            assert_eq!(find_by_filename(&m.filename).map(|f| f.kind), Some(m.kind));
        }
    }

    /// Value of `key: <literal>` on a `src/lib/models.ts` row, quotes stripped
    /// and the digit grouping of a numeric literal removed.
    fn ts_field<'a>(row: &'a str, key: &str) -> Option<&'a str> {
        let rest = row.split_once(&format!("{key}: "))?.1;
        let value = rest.split([',', '}']).next()?.trim();
        Some(value.trim_matches('"'))
    }

    /// The frontend keeps its own copy of the catalog (`src/lib/models.ts`) for
    /// display. The part both sides claim to know — which files exist, how big they
    /// are and how much VRAM they need — must not drift: a wrong size shows up only
    /// as a progress bar running past 100 %, so it is pinned here instead.
    #[test]
    fn the_frontend_catalog_matches_this_registry() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/models.ts");
        let Ok(source) = std::fs::read_to_string(path) else {
            // Rust-only checkout (a crate tarball): nothing to compare against.
            return;
        };

        let rows: Vec<&str> = source
            .lines()
            .filter(|l| l.contains("filename: \""))
            .collect();
        assert_eq!(
            rows.len(),
            all_whisper_models().len() + all_llm_models().len(),
            "models.ts lists {} models, the registry has {}",
            rows.len(),
            all_whisper_models().len() + all_llm_models().len()
        );

        for m in all_whisper_models().into_iter().chain(all_llm_models()) {
            let row = rows
                .iter()
                .find(|r| ts_field(r, "filename") == Some(m.filename.as_str()))
                .unwrap_or_else(|| panic!("{} is missing from models.ts", m.filename));

            let id = serde_json::to_value(&m.kind).unwrap();
            let id = id.as_object().unwrap().values().next().unwrap().as_str().unwrap().to_string();
            assert_eq!(ts_field(row, "id"), Some(id.as_str()), "id of {}", m.filename);

            let size: u64 = ts_field(row, "sizeBytes")
                .unwrap()
                .replace('_', "")
                .parse()
                .unwrap();
            assert_eq!(size, m.size_bytes, "sizeBytes of {}", m.filename);

            let vram: u32 = ts_field(row, "vramMb").unwrap().trim().parse().unwrap();
            assert_eq!(vram, m.vram_required_mb, "vramMb of {}", m.filename);
        }
    }
}
