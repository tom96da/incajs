// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

mod input;
mod pointer;

const TEST_ENTRY_PATH: &str = "/test/entry.js";

fn eval_f64(h: &Harness, code: &str) -> f64 {
    h.window
        .read_with(&h.cx, |app, _| {
            app.session.engine.eval::<f64>(code).unwrap()
        })
        .unwrap()
}

fn id_where(h: &mut Harness, f: impl Fn(&Node) -> bool) -> NodeId {
    h.snapshot().find(&f).unwrap().id
}

/// Defines `listen(node, type, key)`, which registers the function `cb[key]` under the callback
/// id the host returns.
const LISTEN: &str = r"
        globalThis.cb = {};
        globalThis.listen = (node, type, key) => {
            if (typeof globalThis.cb[key] !== 'function') {
                throw new Error('cb[' + key + '] is not a function');
            }
            const id = __inca_native__.addEventListener(node, type, false, false);
            globalThis.__inca_callbacks__[id] = globalThis.cb[key];
        };
    ";

fn load_listening(cx: &mut TestAppContext, source: &str) -> Harness {
    Harness::load(cx, TEST_ENTRY_PATH, &format!("{LISTEN}{source}"))
}

fn log(h: &Harness) -> String {
    h.window
        .read_with(&h.cx, |app, _| {
            app.session
                .engine
                .eval::<String>("globalThis.log.join(',')")
                .unwrap()
        })
        .unwrap()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn node(h: &Harness, name: &str) -> NodeId {
    eval_f64(h, &format!("globalThis.{name}")) as NodeId
}
