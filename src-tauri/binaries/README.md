# FFmpeg sidecar binaries

These are the bundled `ffmpeg` decoders used as a universal fallback when
symphonia can't handle a file's codec/container (Opus voice notes, AC-3, DTS,
AMR, AVI, …). They are **git-ignored** because they're tens of MB each.

## Setup

```bash
./fetch-ffmpeg.sh         # host platform only (dev)
./fetch-ffmpeg.sh --all   # every release target (CI / release builds)
```

Each binary is named with its Rust target triple so Tauri picks it up via
`externalBin: ["binaries/ffmpeg"]` in `tauri.conf.json`:

| File | Platform | Source / license |
|------|----------|------------------|
| `ffmpeg-x86_64-unknown-linux-gnu`   | Linux x64   | BtbN, **LGPL** static |
| `ffmpeg-aarch64-unknown-linux-gnu`  | Linux ARM64 | BtbN, **LGPL** static |
| `ffmpeg-x86_64-pc-windows-msvc.exe` | Windows x64 | BtbN, **LGPL** static |
| `ffmpeg-aarch64-pc-windows-msvc.exe`| Windows ARM64 | BtbN, **LGPL** static |
| `ffmpeg-aarch64-apple-darwin`       | macOS ARM64 | osxexperts, GPL static |
| `ffmpeg-x86_64-apple-darwin`        | macOS x64   | osxexperts, GPL static |

## Licensing notes

VoodooScribe itself is **GPL-3.0-or-later**, so both FFmpeg build flavours are
compatible and no relicensing question arises.

- Linux/Windows builds are **LGPL** (no `--enable-gpl`) — we only do audio decoding.
- macOS builds from osxexperts are **GPL-2.0-or-later**, which may be used under
  GPLv3. Since this project is GPL anyway, shipping them is fine; swapping in an
  LGPL macOS build is optional, not required.
- **Any binary release must ship the corresponding FFmpeg source, or a written
  offer to provide it** — this obligation comes from the LGPL/GPL of these
  binaries and applies on every platform. Record the exact build URL and version
  used by CI so the matching source can be produced.
- The builds are full (with video). To shrink the installer you can swap in an
  audio-only `--disable-everything --enable-decoder=... --enable-demuxer=...`
  custom build; functionality is unchanged.
