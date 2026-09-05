// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

/// Returns total VRAM in MB for the primary GPU, or 0 if unknown.
///
/// Methods tried, in order:
///   Linux/Windows NVIDIA  — nvidia-smi
///   Linux AMD/Intel       — /sys/class/drm/card*/device/mem_info_vram_total
///   macOS                 — system_profiler SPDisplaysDataType
///   Windows (fallback)    — wmic path Win32_VideoController get AdapterRAM
pub fn total_vram_mb() -> u32 {
    #[cfg(target_os = "macos")]
    {
        if let Some(v) = macos_vram() { return v; }
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(v) = nvidia_smi_vram() { return v; }
        if let Some(v) = linux_amd_vram()  { return v; }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(v) = nvidia_smi_vram() { return v; }
        if let Some(v) = wmic_vram()       { return v; }
    }
    0
}

/// Best model that fits in `vram_mb`, as the snake_case WhisperSize variant name
/// used by serde.
pub fn recommend_for_vram(vram_mb: u32) -> &'static str {
    // No detectable GPU memory: either there is no discrete card or it could not
    // be queried. Inference then runs on the CPU, where the big models are far too
    // slow to recommend.
    if vram_mb == 0 {
        return recommend_for_cpu(system_ram_mb());
    }
    // Best model that fits, using the VRAM figures from registry.rs. Quality order
    // is tiny < base < small < medium < turbo < large v3, and quantizing costs less
    // accuracy than dropping a size, so at a given budget a Q5 build of the bigger
    // model beats the full build of the smaller one. Medium never wins a bracket:
    // turbo is better and lighter.
    match vram_mb {
        0             => unreachable!("handled above"),
        1..=499       => "tiny",
        500..=999     => "base",
        1_000..=1_199 => "small",
        1_200..=2_199 => "large_v3_turbo_q5",
        2_200..=2_399 => "large_v3_turbo",
        2_400..=4_699 => "large_v3_q5", // full decoder, quantized: beats turbo
        _             => "large_v3",    // 4.7 GB+: nothing here is better
    }
}

/// True when a model-load failure reads like the GPU ran out of memory. ggml,
/// Vulkan and Metal word this differently, so the match is on common fragments
/// rather than one message.
pub fn looks_like_oom(raw: &str) -> bool {
    let raw = raw.to_lowercase();
    [
        "out of memory",
        "outofdevicememory",
        "outofhostmemory",
        "failed to allocate",
        "allocation of size",
        "cannot allocate",
        "unable to allocate",
        "insufficient memory",
    ]
    .iter()
    .any(|needle| raw.contains(needle))
}

/// Turns a raw model-load failure into an actionable message. ggml's bare "failed
/// to load model" says nothing about the cause, which on a small card is almost
/// always VRAM.
pub fn classify_load_failure(required_mb: u32, raw: &str) -> Option<(u32, u32)> {
    let free = free_vram_mb();
    let short_on_memory = free.is_some_and(|f| f < required_mb);
    if looks_like_oom(raw) || short_on_memory {
        Some((required_mb, free.unwrap_or(0)))
    } else {
        None
    }
}

/// Rough VRAM a GGUF/ggml model needs: its file size, which is what is streamed
/// onto the device. Compute buffers come on top and are covered by the headroom
/// constants.
pub fn model_file_size_mb(path: &std::path::Path) -> u32 {
    std::fs::metadata(path)
        .map(|m| (m.len() / 1_048_576) as u32)
        .unwrap_or(0)
}

/// Slack on top of the models' nominal sizes: KV cache, ggml compute buffers and
/// fragmentation. Smaller when the free figure is known, since the desktop's own
/// usage is then already accounted for.
pub const HEADROOM_MEASURED_MB: u32 = 1_500;
pub const HEADROOM_TOTAL_MB: u32 = 3_000;

/// Whether both models can sit in VRAM at once. `available_mb == 0` means nothing
/// could be detected, and the machine is assumed to cope, as in the model
/// recommendation.
pub fn both_fit(available_mb: u32, headroom_mb: u32, whisper_mb: u32, llm_mb: u32) -> bool {
    if available_mb == 0 {
        return true;
    }
    available_mb >= whisper_mb.saturating_add(llm_mb).saturating_add(headroom_mb)
}

/// `both_fit` against the best figure the machine can give: free VRAM where the
/// platform exposes it, otherwise the card's total.
pub fn both_models_fit(whisper_mb: u32, llm_mb: u32) -> bool {
    match free_vram_mb() {
        Some(free) => both_fit(free, HEADROOM_MEASURED_MB, whisper_mb, llm_mb),
        None => both_fit(total_vram_mb(), HEADROOM_TOTAL_MB, whisper_mb, llm_mb),
    }
}

/// VRAM free right now, or `None` when the platform offers no cheap way to ask.
///
/// The desktop and anything else on the same GPU already hold memory, so deciding
/// what fits from the card's total size overestimates what can be loaded.
///
/// Supported: NVIDIA everywhere via nvidia-smi, AMD/Intel on Linux via sysfs.
/// macOS and Windows return `None`, and callers then use the total.
pub fn free_vram_mb() -> Option<u32> {
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        if let Some(v) = nvidia_smi_free() {
            return Some(v);
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Some((total, used)) = linux_drm_total_used() {
            return Some(total.saturating_sub(used));
        }
    }
    None
}

/// Best model for a machine running on the CPU, chosen by system RAM. Biased
/// small: CPU inference is roughly an order of magnitude slower, so a model that
/// merely fits is still too slow to use.
pub fn recommend_for_cpu(ram_mb: u32) -> &'static str {
    match ram_mb {
        0..=3_999 => "tiny",
        4_000..=7_999 => "base",
        _ => "small",
    }
}

/// Total system RAM in MB (0 if it can't be read).
pub fn system_ram_mb() -> u32 {
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_memory();
    (sys.total_memory() / 1_048_576) as u32
}

// ---------------------------------------------------------------------------
// Platform implementations
// ---------------------------------------------------------------------------

/// Parses `nvidia-smi --query-gpu=... --format=csv,noheader,nounits` output, one
/// line of megabytes per GPU. The first (primary) GPU is used.
fn parse_smi_mb(stdout: &str) -> Option<u32> {
    stdout.lines().next().and_then(|l| l.trim().parse::<u32>().ok())
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn nvidia_smi_vram() -> Option<u32> {
    let out = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=memory.total", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;
    parse_smi_mb(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn nvidia_smi_free() -> Option<u32> {
    let out = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=memory.free", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;
    parse_smi_mb(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(target_os = "linux")]
fn linux_amd_vram() -> Option<u32> {
    linux_drm_total_used().map(|(total, _)| total)
}

/// Reads `(total_mb, used_mb)` from AMD/Intel sysfs. Both values come from the
/// same card; mixing cards would produce a nonsense "free" figure.
#[cfg(target_os = "linux")]
fn linux_drm_total_used() -> Option<(u32, u32)> {
    for entry in std::fs::read_dir("/sys/class/drm").ok()?.flatten() {
        let device = entry.path().join("device");
        let Some(total) = read_bytes_as_mb(&device.join("mem_info_vram_total")) else {
            continue;
        };
        if total == 0 {
            continue;
        }
        // `used` is missing on some drivers; treated as "nothing used" rather
        // than skipping an otherwise usable card.
        let used = read_bytes_as_mb(&device.join("mem_info_vram_used")).unwrap_or(0);
        return Some((total, used.min(total)));
    }
    None
}

#[cfg(target_os = "linux")]
fn read_bytes_as_mb(path: &std::path::Path) -> Option<u32> {
    let raw = std::fs::read_to_string(path).ok()?;
    raw.trim().parse::<u64>().ok().map(|b| (b / 1_048_576) as u32)
}

#[cfg(target_os = "macos")]
fn macos_vram() -> Option<u32> {
    // system_profiler returns JSON with VRAM info under SPDisplaysDataType.
    // Key is "spdisplays_vram" with values like "8 GB" or "1536 MB".
    let out = std::process::Command::new("system_profiler")
        .args(["SPDisplaysDataType", "-json"])
        .output()
        .ok()?;
    let json = String::from_utf8_lossy(&out.stdout);
    // Plain text scan, to avoid pulling in a JSON parser.
    for line in json.lines() {
        let line = line.trim();
        if line.starts_with("\"spdisplays_vram\"") {
            // e.g. "spdisplays_vram" : "8 GB"
            let val = line.split(':').nth(1)?.trim().trim_matches('"').trim();
            if let Some(gb) = val.strip_suffix(" GB") {
                return gb.trim().parse::<u32>().ok().map(|g| g * 1024);
            }
            if let Some(mb) = val.strip_suffix(" MB") {
                return mb.trim().parse::<u32>().ok();
            }
        }
    }
    // Apple Silicon: unified memory, so total RAM stands in for VRAM.
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_memory();
    Some((sys.total_memory() / 1_048_576) as u32)
}

#[cfg(target_os = "windows")]
fn wmic_vram() -> Option<u32> {
    let out = std::process::Command::new("wmic")
        .args(["path", "Win32_VideoController", "get", "AdapterRAM"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    s.lines()
        .filter_map(|l| l.trim().parse::<u64>().ok())
        .next()
        .map(|bytes| (bytes / 1_048_576) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_first_gpu_line() {
        assert_eq!(parse_smi_mb("16384\n"), Some(16_384));
        // Multi-GPU: the primary one is used.
        assert_eq!(parse_smi_mb("8192\n24576\n"), Some(8_192));
        assert_eq!(parse_smi_mb("  4096  "), Some(4_096));
        assert_eq!(parse_smi_mb(""), None);
        assert_eq!(parse_smi_mb("N/A"), None);
    }

    #[test]
    fn recognizes_out_of_memory_messages() {
        assert!(looks_like_oom(
            "ggml_vulkan: Device memory allocation of size 1610612736 failed"
        ));
        assert!(looks_like_oom("vk::Device::allocateMemory: ErrorOutOfDeviceMemory"));
        assert!(looks_like_oom("Metal: failed to allocate buffer"));
        // Unrelated failures are not labelled as memory problems.
        assert!(!looks_like_oom("failed to open file: no such file or directory"));
        assert!(!looks_like_oom("invalid model file magic"));
    }

    #[test]
    fn fitting_decision() {
        // 8 GB card: whisper turbo + Qwen + headroom doesn't fit.
        assert!(!both_fit(8_192, HEADROOM_TOTAL_MB, 1_600, 4_000));
        // Same card, but measured free memory and the smaller headroom: fits.
        assert!(both_fit(8_192, HEADROOM_MEASURED_MB, 1_600, 4_000));
        // 16 GB: fits either way.
        assert!(both_fit(16_384, HEADROOM_TOTAL_MB, 1_600, 4_000));
        // 4 GB: never.
        assert!(!both_fit(4_096, HEADROOM_MEASURED_MB, 1_600, 4_000));
        // Undetectable VRAM → assume it copes.
        assert!(both_fit(0, HEADROOM_TOTAL_MB, 1_600, 4_000));
        // A busy GPU with little left evicts.
        assert!(!both_fit(2_000, HEADROOM_MEASURED_MB, 1_600, 4_000));
    }

    #[test]
    fn recommendation_scales_with_vram() {
        assert_eq!(recommend_for_vram(400), "tiny");
        assert_eq!(recommend_for_vram(700), "base");
        assert_eq!(recommend_for_vram(1_100), "small");
        assert_eq!(recommend_for_vram(1_400), "large_v3_turbo_q5");
        assert_eq!(recommend_for_vram(2_300), "large_v3_turbo");
        assert_eq!(recommend_for_vram(4_096), "large_v3_q5");
        // A roomy card gets the full model, not turbo.
        assert_eq!(recommend_for_vram(8_192), "large_v3");
    }

    #[test]
    fn cpu_only_machines_get_a_modest_model() {
        // No GPU means CPU inference; never send those users to a 1.6 GB model.
        assert_eq!(recommend_for_cpu(2_048), "tiny");
        assert_eq!(recommend_for_cpu(4_096), "base");
        assert_eq!(recommend_for_cpu(6_144), "base");
        assert_eq!(recommend_for_cpu(8_192), "small");
        assert_eq!(recommend_for_cpu(32_768), "small");
        assert_eq!(recommend_for_cpu(0), "tiny");
    }
}
