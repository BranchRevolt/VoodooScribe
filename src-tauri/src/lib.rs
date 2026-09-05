// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

pub mod audio;
pub mod cli;
pub mod commands;
pub mod error;
pub mod models;
pub mod monitor_fix;
pub mod state;
pub mod summarize;
pub mod transcribe;
pub mod vram;

use commands::{
    export,
    models as cmd_models,
    summarize as cmd_summarize,
    transcribe as cmd_transcribe,
};
use state::AppState;

pub fn run() {
    // On Linux the webview's DMABUF renderer competes with local inference
    // (whisper/llama on Vulkan) for the same device: during summarize bursts the
    // webview content freezes and recovers between them, even though the heavy work
    // is off the main thread. Taking the webview off the DMABUF path keeps it
    // responsive under GPU load. A user-set value is respected; WebView2 and
    // WKWebView ignore the variable. The stronger option is
    // WEBKIT_DISABLE_COMPOSITING_MODE=1.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "voodooscribe=info".into()),
        )
        .init();

    // Redirects whisper.cpp / ggml stderr (per-token debug, model load dumps) into
    // whisper-rs's log macros, which are no-ops without the log/tracing backend
    // features.
    whisper_rs::install_logging_hooks();

    let app_state = AppState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .setup(|app| {
            use tauri::Manager;
            let handle = app.handle().clone();
            handle.state::<AppState>().auto_discover(&handle);
            // Linux/WebKitGTK only: relayout after the window changes monitors.
            monitor_fix::install(&handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            cmd_transcribe::cmd_transcribe,
            cmd_transcribe::cmd_cancel_transcribe,
            cmd_summarize::cmd_summarize,
            cmd_summarize::cmd_polish_transcript,
            cmd_summarize::cmd_cancel_summarize,
            cmd_models::cmd_models_status,
            cmd_models::cmd_list_whisper_models,
            cmd_models::cmd_list_llm_models,
            cmd_models::cmd_download_status,
            cmd_models::cmd_download_model,
            cmd_models::cmd_cancel_download,
            cmd_models::cmd_cancel_and_delete,
            cmd_models::cmd_delete_model,
            cmd_models::cmd_select_model,
            cmd_models::cmd_get_models_dir,
            cmd_models::cmd_set_models_dir,
            cmd_models::cmd_set_model_path,
            export::cmd_export_transcript,
            export::cmd_export_summary,
        ])
        .run(tauri::generate_context!())
        .expect("error running Tauri app");
}
