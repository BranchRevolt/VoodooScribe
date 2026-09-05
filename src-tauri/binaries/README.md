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

| File | Platform | Source | License |
|------|----------|--------|---------|
| `ffmpeg-x86_64-unknown-linux-gnu`   | Linux x64     | `BtbN/FFmpeg-Builds`, `ffmpeg-master-latest-linux64-lgpl.tar.xz`   | LGPL |
| `ffmpeg-aarch64-unknown-linux-gnu`  | Linux ARM64   | `BtbN/FFmpeg-Builds`, `ffmpeg-master-latest-linuxarm64-lgpl.tar.xz` | LGPL |
| `ffmpeg-x86_64-pc-windows-msvc.exe` | Windows x64   | `BtbN/FFmpeg-Builds`, `ffmpeg-master-latest-win64-lgpl.zip`       | LGPL |
| `ffmpeg-aarch64-pc-windows-msvc.exe`| Windows ARM64 | `BtbN/FFmpeg-Builds`, `ffmpeg-master-latest-winarm64-lgpl.zip`    | LGPL |
| `ffmpeg-aarch64-apple-darwin`       | macOS ARM64   | osxexperts.net, `ffmpeg<version>arm.zip`                          | GPL  |
| `ffmpeg-x86_64-apple-darwin`        | macOS x64     | osxexperts.net, `ffmpeg<version>intel.zip`                        | GPL  |

BtbN publishes under a rolling `latest` tag, and osxexperts puts the FFmpeg
version in the file name and drops the previous file when a new one appears.
Neither address identifies one immutable build, so `fetch-ffmpeg.sh` reads the
current macOS file name off the index page rather than hard-coding it — and the
exact files a release was built from have to be written down when that release
is made. See below.

## Licensing notes

VoodooScribe itself is **GPL-3.0-or-later**, so both FFmpeg build flavors are
compatible and no relicensing question arises.

- Linux/Windows builds are **LGPL** (no `--enable-gpl`), which is enough because
  the app only decodes audio.
- macOS builds from osxexperts are **GPL-2.0-or-later**, which may be used under
  GPLv3. Since this project is GPL anyway, shipping them is fine; swapping in an
  LGPL macOS build is optional, not required.
- **Any binary release must ship the corresponding FFmpeg source, or a written
  offer to provide it** — this obligation comes from the LGPL/GPL of these
  binaries and applies on every platform. Because both upstreams move their
  files, the offer is only meaningful if the exact build is recorded at release
  time; that is what the section below is for.
- The builds are full (with video). To shrink the installer you can swap in an
  audio-only `--disable-everything --enable-decoder=... --enable-demuxer=...`
  custom build; functionality is unchanged.

## Builds shipped in each release

Filled in when a release is cut, from the `Fetch FFmpeg sidecar` step of the
build that produced the artifacts. Without it the source offer above cannot be
honored, because neither upstream keeps old files at a stable address.

### 1.0.0

| Platform | Archive |
|---|---|
| Linux x64   | `ffmpeg-master-latest-linux64-lgpl.tar.xz` (BtbN, rolling `latest` tag) |
| Windows x64 | `ffmpeg-master-latest-win64-lgpl.zip` (BtbN, rolling `latest` tag) |
| macOS ARM64 | `ffmpeg711arm.zip` |
| macOS x64   | `ffmpeg80intel.zip` |

The two macOS builds are different FFmpeg versions: the ARM package was built
before osxexperts renamed its files, the Intel one after. Both decode the same
formats, so this affects the source offer only, not behavior.
