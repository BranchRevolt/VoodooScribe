# SPDX-FileCopyrightText: 2026 WarpCoreDev
# SPDX-License-Identifier: GPL-3.0-or-later

"""
One-time script to generate audio fixtures for audio module tests.
Run from repo root: .venv-fixtures/bin/python src-tauri/tests/gen_fixtures.py
Requires: soundfile numpy pydub (lame must be in PATH for MP3)
"""
import struct, wave, os, numpy as np, soundfile as sf

OUT = os.path.join(os.path.dirname(__file__), "fixtures")
os.makedirs(OUT, exist_ok=True)

RATE = 44100
DURATION = 0.5  # seconds — keep fixtures tiny
N = int(RATE * DURATION)
t = np.linspace(0, DURATION, N, endpoint=False)
# 440 Hz sine, stereo, float32 in [-1, 1]
tone_mono = (np.sin(2 * np.pi * 440 * t) * 0.5).astype(np.float32)
tone_stereo = np.column_stack([tone_mono, tone_mono])

# 1. WAV — 16-bit stereo 44100
sf.write(f"{OUT}/tone.wav", tone_stereo, RATE, subtype="PCM_16")
print("tone.wav")

# 2. FLAC — 16-bit stereo 44100
sf.write(f"{OUT}/tone.flac", tone_stereo, RATE, subtype="PCM_16")
print("tone.flac")

# 3. OGG/Vorbis — stereo 44100
sf.write(f"{OUT}/tone.ogg", tone_stereo, RATE, format="OGG", subtype="VORBIS")
print("tone.ogg")

# 4. AIFF — 16-bit stereo 44100 (Apple native, symphonia supports it)
sf.write(f"{OUT}/tone.aiff", tone_stereo, RATE, format="AIFF", subtype="PCM_16")
print("tone.aiff")

# 5. MP3 — mono 44100 via lame directly (pydub removed audioop in Py3.14)
import subprocess
pcm16 = (tone_mono * 32767).astype(np.int16).tobytes()
result = subprocess.run(
    ["lame", "-r", "-s", "44100", "--bitwidth", "16", "-m", "m",
     "--signed", "--little-endian", "-b", "64", "-", f"{OUT}/tone.mp3"],
    input=pcm16, capture_output=True
)
if result.returncode != 0:
    raise RuntimeError(f"lame failed: {result.stderr.decode()}")
print("tone.mp3")

print("Done — 5 fixtures in", OUT)
