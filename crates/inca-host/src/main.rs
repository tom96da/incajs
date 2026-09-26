// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Parses argv and hands off to `app::run_bundle` — see `app.rs` for what
//! the binary actually does.

mod app;
mod dev;
mod menu;
mod protocol;

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    env_logger::init();

    let args: Vec<String> = env::args().skip(1).collect();
    let (dev, entry_path) = match args.as_slice() {
        [] => {
            let Some(path) = app::bundle_beside_exe() else {
                eprintln!(
                    "usage: inca-host [--dev] [<path-to-bundle.js>]\n\
                     no bundle.js found beside the executable"
                );
                return ExitCode::FAILURE;
            };
            (false, path.to_string_lossy().into_owned())
        }
        [path] => (false, path.clone()),
        [flag, path] if flag == "--dev" => (true, path.clone()),
        _ => {
            eprintln!("usage: inca-host [--dev] [<path-to-bundle.js>]");
            return ExitCode::FAILURE;
        }
    };

    app::run_bundle(&entry_path, dev)
}
