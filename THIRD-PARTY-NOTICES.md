# Third-party notices

VoodooScribe is licensed under the **GNU General Public License v3.0 or later**
(see `LICENSE` and `NOTICE`). It redistributes, links against, or downloads the
third-party components listed below, each under its own licence.

All bundled dependencies are under permissive or GPL-compatible licences. There
is no licence conflict: GPLv3 was chosen specifically because a large part of
the Rust dependency tree is Apache-2.0, which is compatible with GPL **v3** but
not with GPL v2.

This file is an attribution summary for convenience, not legal advice.

## Statically linked into the application binary

| Component | Licence | Source |
|---|---|---|
| **whisper.cpp** + **ggml** (speech recognition) | MIT | <https://github.com/ggerganov/whisper.cpp> — linked via `whisper-rs` (Unlicense OR MIT) and `whisper-rs-sys` |
| **llama.cpp** + **ggml** (summarisation / punctuation) | MIT | <https://github.com/ggml-org/llama.cpp> — linked via `llama-cpp-2` (MIT OR Apache-2.0) |
| **Tauri** (application framework, WRY/TAO) | Apache-2.0 OR MIT | <https://github.com/tauri-apps/tauri> |
| **Symphonia** (audio demuxing and decoding) | MPL-2.0 | <https://github.com/pdeljanov/Symphonia> |
| **rubato** (sample-rate conversion) | MIT | <https://github.com/HEnquist/rubato> |
| **docx-rs** (DOCX export) | MIT | <https://github.com/bokuweb/docx-rs> |
| **reqwest**, **rustls**, **ring**, **tokio**, **serde** and the rest of the Rust dependency tree (568 crates) | MIT / Apache-2.0 / BSD / ISC / Zlib / Unicode-3.0 / CDLA-Permissive-2.0 / MPL-2.0 | see `cargo metadata` or `Cargo.lock` |
| **React**, **Vite**, **Tailwind CSS**, **i18next**, **react-i18next**, **Zustand**, **react-dropzone** and the rest of the npm tree (243 packages) | MIT / ISC / Apache-2.0 / 0BSD / BSD-3-Clause | see `package-lock.json` |

**Note on Symphonia and the other MPL-2.0 crates** (`symphonia-*`, `cssparser`,
`selectors`, `dtoa-short`, `option-ext`): MPL-2.0 is a per-file copyleft. Their
files are used unmodified, and MPL-2.0 §3.3 explicitly permits distributing a
Larger Work under the GPL, so redistribution inside this program is compliant.
If you modify those files, the modifications must remain under MPL-2.0.

## Embedded data file

**Silero VAD** — `src-tauri/resources/models/ggml-silero-v5.1.2.bin` (~865 KB),
embedded into the executable via `include_bytes!` and used for voice-activity
detection.

- Model: <https://github.com/snakers4/silero-vad> — MIT
- ggml conversion: <https://huggingface.co/ggml-org/whisper-vad>

## Bundled sidecar executable

**FFmpeg** — shipped as a separate `ffmpeg` sidecar process, used as a universal
decoding fallback for containers and codecs Symphonia cannot handle. It is
executed as a separate program, not linked into the application.

| Target | Build | Licence |
|---|---|---|
| Linux x64 / ARM64, Windows x64 / ARM64 | [BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds) | LGPL-2.1-or-later |
| macOS x64 / ARM64 | [osxexperts.net](https://www.osxexperts.net/) | GPL-2.0-or-later |

Both are compatible with this program's GPL-3.0-or-later licence (GPL-2.0-**or-later**
may be used under GPLv3). **Binary releases must be accompanied by the
corresponding FFmpeg source**, or by a written offer to provide it, as required
by the LGPL and GPL. Sidecars are fetched by `src-tauri/binaries/fetch-ffmpeg.sh`
and are not stored in this repository.

FFmpeg is a trademark of Fabrice Bellard, originator of the FFmpeg project.

## Downloaded by the user at runtime (not redistributed)

These models are fetched from Hugging Face on demand into the application data
directory. They are **not** part of this repository or of any binary release.

| Model | Licence | Source |
|---|---|---|
| Whisper `ggml-*.bin` (tiny / base / small / medium / large-v3-turbo) | MIT | OpenAI Whisper, ggml conversions at <https://huggingface.co/ggerganov/whisper.cpp> |
| `Qwen3-4B-Q4_K_M.gguf` | Apache-2.0 | <https://huggingface.co/Qwen/Qwen3-4B-GGUF> |

## Build-time only

Vulkan headers and the `glslc` shader compiler (Apache-2.0) are required to
build the GPU backends; they are not redistributed. Some build-time-only npm
packages carry other licences (for example `caniuse-lite`, CC-BY-4.0); none of
them end up in the shipped application.
