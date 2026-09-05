# Building VoodooScribe

VoodooScribe is a Tauri 2 application: a Rust backend that statically links whisper.cpp and
llama.cpp, and a React + TypeScript frontend. Building it means building both, plus the GPU
backends, so the first build is slow (20–40 minutes is normal) and needs several GB of disk.

Development happens on Linux. Windows and macOS are built by CI but have never been run by hand — if
you build there, please report what broke.

## Prerequisites

### All platforms

| Tool | Version | Why |
|---|---|---|
| Rust | 1.82 or newer | The crate sets `rust-version = "1.82"` |
| Node.js | 18 or newer (20+ recommended) | Vite 6 |
| CMake | 3.20 or newer | whisper.cpp and llama.cpp are built from source |
| A C/C++ toolchain | — | Same reason |
| Python 3 | 3.8+ | Only for `scripts/generate-icons.py`, not for building |

### Linux

Tauri needs WebKitGTK and friends; the GPU backends need the Vulkan headers at build time and the
Vulkan loader at run time.

Debian / Ubuntu:

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
     libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev patchelf \
     cmake vulkan-tools libvulkan-dev glslc
```

Arch / CachyOS:

```bash
sudo pacman -S webkit2gtk-4.1 base-devel curl wget file openssl \
     libayatana-appindicator librsvg patchelf cmake \
     vulkan-headers vulkan-icd-loader shaderc
```

At run time you additionally need a Vulkan driver for your GPU (`vulkan-radeon`, `nvidia-utils`,
`vulkan-intel`, …). Verify with `vulkaninfo --summary`.

The canonical, always-current list of Tauri's own system dependencies lives at
<https://tauri.app/start/prerequisites/>.

### Windows

- Visual Studio Build Tools with the C++ workload
- WebView2 runtime (preinstalled on Windows 11)
- CMake
- The [Vulkan SDK](https://vulkan.lunarg.com/) — provides both the headers and `glslc`

> **Untested.** `src-tauri/.cargo/config.toml` passes `-Wl,--allow-multiple-definition` on Linux
> targets to resolve duplicate ggml symbols (whisper-rs-sys and llama-cpp-sys-2 each bundle their
> own copy). The MSVC linker needs a different flag — likely `/FORCE:MULTIPLE` — and nobody has
> confirmed this yet. Expect to fight the linker on the first Windows build.

### macOS

- Xcode Command Line Tools (`xcode-select --install`)
- CMake

Metal is part of the system, so there is no GPU SDK to install. macOS is the one platform where the
duplicate-symbol problem resolves itself, because dyld picks one definition.

## Building

```bash
git clone <repository-url>
cd VoodooScribe

npm install                             # frontend dependencies
./src-tauri/binaries/fetch-ffmpeg.sh    # FFmpeg sidecar for your platform

npm run tauri dev                       # development build with hot reload
npm run tauri build                     # release build + installers
```

Installers land in `src-tauri/target/release/bundle/`.

For a release build covering every platform, fetch all sidecars instead:

```bash
./src-tauri/binaries/fetch-ffmpeg.sh --all
```

The sidecar binaries are git-ignored — they are tens of MB each. See
[`src-tauri/binaries/README.md`](../src-tauri/binaries/README.md) for their sources and licences;
note that any binary release must ship the corresponding FFmpeg source or a written offer for it.

## Running the tests

```bash
cd src-tauri
cargo test
```

The default run is fully offline: 44 tests covering export formatting, audio handling, the
hallucination filter, VRAM arithmetic, chunking and catalog consistency.

Tests that need real models are environment-gated and skip when the variables are unset:

| Variable | Enables |
|---|---|
| `VOODOOSCRIBE_TEST_WHISPER_MODEL` | ASR smoke test and the whisper context-reuse regression test |
| `VOODOOSCRIBE_TEST_AUDIO` | Audio fixture for the ASR test |
| `VOODOOSCRIBE_TEST_EXPECT` / `_LANG` | Optional expected text and language for that test |
| `VOODOOSCRIBE_TEST_LLM_MODEL` | Summarize/polish tests against a real GGUF |

Frontend type checking:

```bash
npx tsc --noEmit
```

The build is expected to be **warning-free**. Please keep it that way.

## Linux development tip: the window icon

Under Wayland a window cannot set its own icon — the compositor matches the window's `app_id`
against an installed `.desktop` file. Packaged builds ship one; `tauri dev` does not, so the dev
window shows a placeholder icon until you install an entry:

```bash
./scripts/install-linux-desktop-entry.sh          # --uninstall to remove
```

## Regenerating the app icon

`assets/logo.png` is the master artwork. Everything under `src-tauri/icons/` is generated from it and
should never be edited by hand:

```bash
python3 scripts/generate-icons.py
```

The script is deterministic — running it without changing the logo reproduces the committed icons
byte for byte.

## Build troubleshooting

**Duplicate ggml symbols at link time.** Both `whisper-rs-sys` and `llama-cpp-sys-2` bundle ggml.
On Linux this is handled by `--allow-multiple-definition` in `src-tauri/.cargo/config.toml`. The two
copies are ABI-compatible, so taking the first definition is safe.

**`target/` grew to tens of GB.** Statically linked whisper.cpp and llama.cpp with full debug info
are enormous. The dev profile already limits this (`debug = "line-tables-only"`, dependencies built
without debug info). `cargo clean` reclaims the rest.

**The build fails with paths that no longer exist after you moved the project.** Cargo caches build
script output, including absolute paths, and does not rerun scripts whose fingerprint is still valid.
Either `cargo clean`, or rewrite the stale prefix in place:

```bash
cd src-tauri/target
grep -rlIZ "old/path/segment" . | xargs -0 -r sed -i 's|old/path/segment|new/path/segment|g'
```

**Rust changes do not appear after a hot reload.** The Vite side hot-reloads; the Rust side does not.
Stop and restart `npm run tauri dev` fully, or you will keep talking to the old backend process.
