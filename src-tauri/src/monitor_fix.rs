// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

//! Linux/WebKitGTK: forces a relayout after the window moves to another monitor.
//!
//! WebKitGTK keeps rendering against the previous monitor's geometry when the
//! window is dragged to a screen with a different scale factor; the content stays
//! wrong until a relayout happens. This installs one on every monitor change.
//!
//! The relayout goes through GTK directly rather than `WebviewWindow::set_size`.
//! Tauri fills `inner_size()` from the `configure-event`, which under client-side
//! decorations includes the shadow and titlebar, while `gtk_window_resize` takes
//! the size without them; feeding one into the other adds the frame on every call
//! (+90x138 logical pixels per monitor change). `gtk_window.size()` and
//! `gtk_window.resize()` share one coordinate space, so the round trip is a no-op.
//!
//! Monitor scale is deliberately not tracked. At a fractional scale the desktop
//! has fewer logical pixels (1536x864 for a 1080p screen at 1.25x), so a window
//! keeping its size covers a larger share of it; compensating for that during a
//! move blanks the webview. Only the startup fit-to-screen is applied.
//! Debug builds log measurements to `window-debug.log`.
//!
//! No-op on Windows/macOS: WebView2 and WKWebView handle the monitor switch
//! themselves.

#[cfg(target_os = "linux")]
mod imp {
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::{Duration, Instant};

    use gtk::glib;
    use gtk::prelude::GtkWindowExt;
    use tauri::{AppHandle, Manager, WebviewWindow, WindowEvent};

    /// Trace file for window geometry. Set in debug builds only.
    static TRACE_PATH: OnceLock<PathBuf> = OnceLock::new();
    static TRACE_LINES: AtomicUsize = AtomicUsize::new(0);
    /// Caps the trace file size over a long session.
    const TRACE_MAX_LINES: usize = 2_000;

    /// How long GTK gets to apply the one-pixel step before it is taken back.
    const NUDGE_STEP: Duration = Duration::from_millis(60);
    /// Quiet time after the step before another relayout may be triggered.
    const NUDGE_SETTLE: Duration = Duration::from_millis(120);

    fn trace(line: &str) {
        let Some(path) = TRACE_PATH.get() else { return };
        if TRACE_LINES.fetch_add(1, Ordering::Relaxed) >= TRACE_MAX_LINES {
            return;
        }
        tracing::debug!("window {line}");
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(path) {
            let _ = writeln!(f, "{line}");
        }
    }

    /// Logs the geometry the window and its monitor report.
    ///
    /// Note that tao scales every size by the monitor's factor, so a 1100-wide
    /// window reads as 2200 here and a 1.25x 1080p screen as 3072 wide.
    fn log_geometry(window: &WebviewWindow, event: &str) {
        if TRACE_PATH.get().is_none() {
            return;
        }
        let monitor = window.current_monitor().ok().flatten();
        trace(&format!(
            "{event}: inner={:?} win_scale={:?} max={:?} monitor={:?} mon_size={:?} mon_scale={:?}",
            window.inner_size().ok().map(|s| (s.width, s.height)),
            window.scale_factor().ok(),
            window.is_maximized().ok(),
            monitor.as_ref().and_then(|m| m.name().cloned()),
            monitor.as_ref().map(|m| (m.size().width, m.size().height)),
            monitor.as_ref().map(|m| m.scale_factor()),
        ));
    }

    /// Identity of the screen the window currently sits on. The scale factor is
    /// part of the key: two monitors may differ only in DPI, and a display can be
    /// rescaled in place.
    fn monitor_key(window: &WebviewWindow) -> Option<String> {
        let monitor = window.current_monitor().ok().flatten()?;
        let name = monitor.name().cloned().unwrap_or_default();
        // Scale is a float; format it instead of comparing bit patterns.
        Some(format!("{name}@{:.4}", monitor.scale_factor()))
    }

    /// Forces WebKitGTK to recompute its layout against the new monitor by
    /// resizing one pixel and back.
    ///
    /// Re-applying the zoom would do the same but makes WebKitGTK re-render from
    /// scratch, which blanks the content for a moment.
    fn force_relayout(window: &WebviewWindow, busy: Arc<AtomicBool>) {
        // The resize re-enters this handler through Moved/Resized; the flag breaks
        // the loop.
        if busy.swap(true, Ordering::SeqCst) {
            return;
        }
        if window.is_maximized().unwrap_or(false) || window.is_fullscreen().unwrap_or(false) {
            busy.store(false, Ordering::SeqCst);
            return;
        }

        let handle = window.clone();
        let done = busy.clone();
        // GTK objects belong to the main thread; the closure runs there.
        let sent = window.run_on_main_thread(move || {
            let Ok(gtk_window) = handle.gtk_window() else {
                done.store(false, Ordering::SeqCst);
                return;
            };
            let (width, height) = gtk_window.size();
            trace(&format!("relayout: gtk=({width}, {height})"));
            gtk_window.resize(width, height + 1);
            glib::timeout_add_local_once(NUDGE_STEP, move || {
                gtk_window.resize(width, height);
                // Let the resize events settle before listening for real moves again.
                glib::timeout_add_local_once(NUDGE_SETTLE, move || {
                    done.store(false, Ordering::SeqCst);
                });
            });
        });
        if sent.is_err() {
            busy.store(false, Ordering::SeqCst);
        }
    }

    /// Shrinks the window if it opens larger than its screen can show.
    ///
    /// The configured default (1100x740) fits a 1080p screen at 1x, but a monitor
    /// at 1.25x has only 1536x864 logical pixels, so the window would arrive
    /// taller than the usable area. Runs once at startup, before the webview has
    /// painted.
    fn fit_to_work_area(window: &WebviewWindow) {
        let Some(monitor) = window.current_monitor().ok().flatten() else {
            return;
        };
        let scale = monitor.scale_factor();
        if scale <= 0.0 {
            return;
        }
        // Tauri reports the work area in physical pixels; GTK sizes windows in
        // logical ones.
        let area = monitor.work_area().size;
        let max_w = (f64::from(area.width) / scale) as i32;
        let max_h = (f64::from(area.height) / scale) as i32;

        let handle = window.clone();
        let _ = window.run_on_main_thread(move || {
            let Ok(gtk_window) = handle.gtk_window() else {
                return;
            };
            let (width, height) = gtk_window.size();
            let (fit_w, fit_h) = (width.min(max_w), height.min(max_h));
            trace(&format!(
                "fit: gtk=({width}, {height}) work=({max_w}, {max_h}) -> ({fit_w}, {fit_h})"
            ));
            if (fit_w, fit_h) != (width, height) {
                gtk_window.resize(fit_w, fit_h);
            }
        });
    }

    pub fn install(app: &AppHandle) {
        let Some(window) = app.get_webview_window("main") else {
            return;
        };

        // Debug builds only, truncated per run.
        if cfg!(debug_assertions) {
            if let Ok(dir) = app.path().app_data_dir() {
                let path = dir.join("window-debug.log");
                let _ = std::fs::create_dir_all(&dir);
                let _ = std::fs::write(&path, "");
                let _ = TRACE_PATH.set(path);
            }
        }
        log_geometry(&window, "start");
        fit_to_work_area(&window);

        let watched = window.clone();
        let last_key = Mutex::new(monitor_key(&watched));
        let busy = Arc::new(AtomicBool::new(false));
        // `Moved` fires continuously while dragging, so throttle the monitor
        // lookup.
        let last_check = Mutex::new(Instant::now() - Duration::from_secs(1));

        window.on_window_event(move |event| {
            match event {
                WindowEvent::Moved(_) => log_geometry(&watched, "moved"),
                // Under Wayland a move to another output surfaces as a
                // reconfigure: logged, not acted on.
                WindowEvent::Resized(_) => {
                    log_geometry(&watched, "resized");
                    return;
                }
                WindowEvent::ScaleFactorChanged { .. } => log_geometry(&watched, "rescaled"),
                _ => return,
            }

            {
                let mut last = match last_check.lock() {
                    Ok(l) => l,
                    Err(_) => return,
                };
                if last.elapsed() < Duration::from_millis(100) {
                    return;
                }
                *last = Instant::now();
            }

            let key = monitor_key(&watched);
            // A half-dragged window can report either screen; `busy` absorbs the
            // flapping until the change settles.
            let changed = match last_key.lock() {
                Ok(mut last) => {
                    let changed = key.is_some() && *last != key;
                    if changed {
                        *last = key;
                    }
                    changed
                }
                Err(_) => false,
            };

            if changed {
                force_relayout(&watched, busy.clone());
            }
        });
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    pub fn install(_app: &tauri::AppHandle) {}
}

pub use imp::install;
