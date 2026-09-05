// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.first().map(|s| s.as_str()) == Some("transcribe") {
        if let Err(e) = voodooscribe_lib::cli::run_transcribe(&args[1..]) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    } else {
        voodooscribe_lib::run();
    }
}
