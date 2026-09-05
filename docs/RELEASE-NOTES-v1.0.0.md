# Release notes — VoodooScribe v1.0.0

> This file is the draft body for the GitHub Release. Copy everything below the line into the
> release description, fill in the checksums, and delete this note. Links below are written
> relative to the repository root, which is how GitHub resolves them in a release body.
>
> **Before publishing, see the [release checklist](#release-checklist-not-part-of-the-release-body)
> at the bottom — it is not part of the release body.**

---

## VoodooScribe v1.0.0 — first public release

**Transcribe audio and video entirely on your own machine.** No cloud, no account, no API key.
Your recordings never leave your computer.

VoodooScribe runs Whisper on your GPU to turn recordings into timecoded, searchable text, and can
summarize or clean up the result with a local language model. It works with the network cable
unplugged once the models are downloaded.

### Highlights

🎙️ **Local Whisper transcription** — eight model builds, GPU-accelerated via Vulkan (Linux/Windows) or
Metal (macOS). The app recommends a model that fits your free VRAM and explains itself if one will not.

🌍 **Mixed-language recordings actually work.** The language is re-detected for each speech window,
so a Russian conversation full of English technical terms comes out correct instead of mangled.

🤫 **No hallucinated text over silence.** Voice-activity detection is built in and on by default,
backed by tuned decoding parameters and an output filter — so you do not get "Thanks for watching!"
in the middle of a pause.

📝 **Summaries and readable transcripts, also local.** A Qwen3 model produces a plain-language
summary, or cleans up the raw transcript line by line — punctuation and capitals, optionally grammar
— leaving every segment and timecode exactly where it was.

📤 **Export anywhere** — `txt`, `srt`, `vtt`, `md`, `json`, `docx` for transcripts; `md`, `txt`,
`docx` for summaries.

📚 **Batch queue**, resumable model downloads, drag-and-drop, 30+ formats, and an interface in
seven languages.

### Install

| Platform | File |
|---|---|
| Linux — Debian, Ubuntu | `VoodooScribe_1.0.0_amd64.deb` |
| Linux — Fedora, openSUSE | `VoodooScribe-1.0.0-1.x86_64.rpm` |
| Linux — any distribution | `VoodooScribe_1.0.0_amd64.AppImage` |
| Windows | `VoodooScribe_1.0.0_x64-setup.exe` |
| Windows — for deployment | `VoodooScribe_1.0.0_x64_en-US.msi` |
| macOS — Apple silicon | `VoodooScribe_1.0.0_aarch64.dmg` |
| macOS — Intel | `VoodooScribe_1.0.0_x64.dmg` |

Both Windows files install the same application. Take the `.exe` unless you are pushing the app out
over group policy, which needs the `.msi`. On macOS, pick the file matching your processor: Apple
silicon is everything from the M1 onwards.

Linux packages are built against glibc 2.35 and webkit2gtk-4.1, so they need Ubuntu 22.04, Debian 12,
Fedora 36 or a comparable release, or newer.

For usable speed you need a working GPU driver with Vulkan (Linux/Windows) or Metal (macOS). On Linux
that is the `vulkan-icd-loader` package plus your vendor driver; check with `vulkaninfo --summary`.
Without a GPU the app still runs on CPU and recommends a smaller model.

On first launch the app walks you through downloading a Whisper model (start with **Large v3 Turbo**
if you have 2.2 GB of free VRAM — it is better *and* faster than Medium).

### Known limitations

- **Developed and tested on Linux.** The Windows and macOS packages are built by CI from the same
  source, but nobody has launched them yet. The macOS build is unsigned, so Gatekeeper will warn on
  first launch. Reports from those platforms are very welcome.
- **No speaker diarization** — the transcript does not say who is talking. This was a deliberate
  decision; the reasoning is in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#deliberate-non-goals).
- Summaries come from a 4B parameter model. It is a reading aid, not a record of what was said.

### Verifying your download

```
SHA-256 checksums:

671bcceb7daf328241eb523c7ec942c93dee1b7ea5823cff1c7a8ec04d5e89b1  VoodooScribe_1.0.0_amd64.deb
4bc4a4f59d45509470a0ddb1f3e159c0810627e06b37d367d3db1cb0d94832fa  VoodooScribe-1.0.0-1.x86_64.rpm
0dcc36c275c85b7826ccc3c1e1347fc11061de926c705bd3759c060a5c79f228  VoodooScribe_1.0.0_amd64.AppImage
0f109b96552406e7cbda732fdeec2575b631eb1d13b2c17560ea39143de39b65  VoodooScribe_1.0.0_x64-setup.exe
0d91f38211bb0c503de65f57b502d5e60e8680bd9f136b30472bfecbb32ae4aa  VoodooScribe_1.0.0_x64_en-US.msi
5fe3f1e72c5d8de0ad9a6715e08a2d26df5223f181e47f5107f264a43b13c5ae  VoodooScribe_1.0.0_aarch64.dmg
6a963566d17d2b908491a08891764d6e73d30afa20b8ee2baec3c6b36d0bad0d  VoodooScribe_1.0.0_x64.dmg
```

### License

VoodooScribe is free software under the [GNU General Public License v3.0 or later](LICENSE).
Copyright (C) 2026 WarpCoreDev.

These builds bundle FFmpeg (LGPL-2.1-or-later; GPL-2.0-or-later on macOS) and link whisper.cpp and
llama.cpp (MIT). Full attributions are in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
The corresponding FFmpeg source for these builds is available at the URLs recorded in
[`src-tauri/binaries/README.md`](src-tauri/binaries/README.md); on request we will provide it
directly.

**Full changelog:** [CHANGELOG.md](CHANGELOG.md)

---

## Release checklist (not part of the release body)

Work through this before tagging.

**Version numbers** — the version lives in three places and they must agree:

- [ ] `package.json` → `"version"`
- [ ] `src-tauri/Cargo.toml` → `version` (then run a build so `Cargo.lock` updates)
- [ ] `src-tauri/tauri.conf.json` → `"version"`

**Build** — done by `.github/workflows/release.yml`, which builds all four targets and
attaches the packages to a draft release

- [ ] Record the exact FFmpeg build URLs and versions used, for the source-offer obligation
- [ ] Verify the app launches from an installed package, not just from `tauri dev`
- [ ] Delete the `*.app.tar.gz` files from the draft: they are updater artifacts, and the
      updater is not configured

**Verify**

- [ ] `cd src-tauri && cargo test` — clean
- [ ] `npx tsc --noEmit` — clean
- [ ] `cargo build` — no warnings
- [ ] Transcribe one real file end to end on each platform you are shipping
- [ ] Transcribe a **second** file in the same session (this has caught regressions before)

**Legal**

- [ ] FFmpeg source or a written offer accompanies the release
- [ ] `LICENSE`, `NOTICE` and `THIRD-PARTY-NOTICES.md` are present in the source tree
- [ ] Model licenses in `THIRD-PARTY-NOTICES.md` still match what the registry downloads

**Publish**

- [ ] Check that the date on `## [1.0.0]` in `CHANGELOG.md` matches the day the release goes out
- [ ] Run the Release workflow from the Actions tab with the tag as its input
- [ ] Paste the notes above into the draft and check the checksums against the uploaded files
- [ ] Set the draft as the latest release and publish it
