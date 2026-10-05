// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Resolves the entry path and flags from argv, then runs the app.

use std::env;
use std::process::ExitCode;

const USAGE: &str = "usage: inca-host [--dev | --print-config] [<path-to-bundle.js>]";

/// The `bundle.js` beside the executable, or the failure to report.
fn bundle_beside_exe() -> Result<String, ExitCode> {
    inca_host::bundle_beside_exe()
        .map(|path| path.to_string_lossy().into_owned())
        .ok_or_else(|| {
            eprintln!("{USAGE}\nno bundle.js found beside the executable");
            ExitCode::FAILURE
        })
}

fn main() -> ExitCode {
    env_logger::init();

    let args: Vec<String> = env::args().skip(1).collect();
    let (dev, entry_path) = match args.as_slice() {
        [] => match bundle_beside_exe() {
            Ok(path) => (false, path),
            Err(code) => return code,
        },
        [flag] if flag == "--print-config" => match bundle_beside_exe() {
            Ok(path) => return inca_host::print_config(&path),
            Err(code) => return code,
        },
        [flag, path] if flag == "--print-config" => return inca_host::print_config(path),
        [path] => (false, path.clone()),
        [flag, path] if flag == "--dev" => (true, path.clone()),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    inca_host::run_bundle(&entry_path, dev)
}
