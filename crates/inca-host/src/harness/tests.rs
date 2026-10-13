// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

mod disabled;
mod events;
mod focus;
mod input;
mod key_click;
mod mouse;
mod pointer;
mod reveal;

const ENTRY: &str = "/test/entry.js";

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

/// Defines `listen(node, type, key, capture, passive)`, which registers the
/// function `cb[key]` under the callback id the host returns.
const LISTEN: &str = r"
        globalThis.cb = {};
        globalThis.__inca_callbacks__ = {};
        globalThis.listen = (node, type, key, capture = false, passive = false) => {
            if (typeof globalThis.cb[key] !== 'function') {
                throw new Error('cb[' + key + '] is not a function');
            }
            const id = __inca_native__.addEventListener(node, type, capture, passive);
            globalThis.__inca_callbacks__[id] = globalThis.cb[key];
        };
    ";

fn load_listening(cx: &mut TestAppContext, source: &str) -> Harness {
    Harness::load(cx, ENTRY, &format!("{LISTEN}{source}"))
}

fn run_js(h: &mut Harness, code: &str) {
    h.window
        .read_with(&h.cx, |app, _| app.session.engine.eval::<()>(code).unwrap())
        .unwrap();
    h.settle();
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

/// A 200x200 `frame` holding a 100x50 `button` named `box` at its
/// top-left corner, with `box` focused. Callback 0 logs each event as
/// `type@currentTarget/target`, 1 calls `preventDefault`, 2 calls
/// `stopPropagation` and 3 logs the click fields.
const BUTTON: &str = r"
        const n = __inca_native__;
        const frame = n.createNode('div');
        n.setStyle(frame, 'width', 200);
        n.setStyle(frame, 'height', 200);
        n.appendChild(n.rootNodeId(), frame);
        const box = n.createNode('button');
        n.setStyle(box, 'width', 100);
        n.setStyle(box, 'height', 50);
        n.appendChild(frame, box);
        globalThis.frame = frame;
        globalThis.box = box;
        globalThis.log = [];
        const name = (id) => (id === box ? 'box' : id === frame ? 'frame' : id === n.rootNodeId() ? 'root' : 'kid');
        globalThis.cb = {
            0: (e) => { globalThis.log.push(`${e.type}@${name(e.currentTarget)}/${name(e.target)}`); },
            1: (e) => { e.preventDefault(); },
            2: (e) => { e.stopPropagation(); },
            3: (e) => {
                globalThis.log.push([e.detail, e.button, e.buttons, e.clientX, e.clientY, e.shiftKey,
                    e.pointerId, e.pointerType, e.isPrimary, e.width, e.height, e.pressure].join(':'));
            },
        };
        n.focusNode(box);
    ";

/// Registers callback `callback` for each of `types` on `target`.
fn listen(h: &mut Harness, target: &str, types: &str, callback: u32) {
    run_js(
        h,
        &format!(
            r"
            for (const t of '{types}'.split(' '))
            listen({target}, t, {callback});
            "
        ),
    );
}

// frame > mid (scrolls) > [inner, label (text), tall]; `log` records
// `type@currentTarget/target/eventPhase` by node name.
const TARGETS: &str = r"
        const n = __inca_native__;
        const make = (parent, w, h) => {
            const id = n.createNode('div');
            n.setStyle(id, 'width', w);
            n.setStyle(id, 'height', h);
            n.appendChild(parent, id);
            return id;
        };
        const frame = make(n.rootNodeId(), 200, 200);
        const mid = make(frame, 100, 50);
        n.setStyle(mid, 'overflow_y', 'scroll');
        const inner = make(mid, 30, 30);
        const label = n.createNode('text');
        n.setAttribute(label, 'value', 'hi');
        n.appendChild(mid, label);
        const tall = make(mid, 100, 300);
        Object.assign(globalThis, { frame, mid, inner, label, tall });
        globalThis.log = [];
        const names = { [frame]: 'frame', [mid]: 'mid', [inner]: 'inner' };
        const name = (id) => names[id] ?? 'other';
        globalThis.cb = {
            0: (e) => { globalThis.log.push(
                `${e.type}@${name(e.currentTarget)}/${name(e.target)}/${e.eventPhase}`); },
        };
    ";

fn targets_log(h: &mut Harness) -> String {
    let log = log(h);
    run_js(h, "globalThis.log.length = 0;");
    log
}
