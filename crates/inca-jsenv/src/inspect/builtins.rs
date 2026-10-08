// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Date`, `RegExp`, `Map` and `Set` values.
//!
//! The text of a value of these types (its time, pattern, flags and entries)
//! is read through intrinsics pinned by [`pin`], so reading it runs no script
//! code, whatever a script replaced or defined on the value. Values held
//! inside a `Map` or `Set` are printed like any other value.

use std::cell::RefCell;
use std::fmt::Write;
use std::rc::Rc;

use rquickjs::{
    Ctx, Error, Function, Result as JsResult, Value,
    function::{Rest, This},
};

use super::{MAX_ENTRIES, Mode, put, settled, write_more, write_value};
use crate::paint::Style;
use crate::quickjs::ValueExt;

/// Indexes into the pinned functions.
const ISO: usize = 0;
const SOURCE: usize = 1;
const FLAGS: usize = 2;

/// The `RegExp.prototype` flag getters at `FLAGS..MAP_EACH`, in this order.
const FLAG_LETTERS: &str = "dgimsuvy";
const MAP_EACH: usize = FLAGS + FLAG_LETTERS.len();
const SET_EACH: usize = MAP_EACH + 1;

type Pins<'js> = Vec<Function<'js>>;

fn capture<'js>(ctx: &Ctx<'js>) -> JsResult<Pins<'js>> {
    let pins: Pins<'js> = ctx.eval(
        "(() => {
            const getter = (key) =>
                Object.getOwnPropertyDescriptor(RegExp.prototype, key).get;
            return [
                Date.prototype.toISOString,
                getter('source'),
                ...['hasIndices', 'global', 'ignoreCase', 'multiline', 'dotAll',
                    'unicode', 'unicodeSets', 'sticky'].map(getter),
                Map.prototype.forEach,
                Set.prototype.forEach,
            ];
        })()",
    )?;
    debug_assert_eq!(pins.len(), SET_EACH + 1);
    Ok(pins)
}

/// Reads the intrinsics `console` prints a `Date`, `RegExp`, `Map` or `Set`
/// with. The first pinning in a runtime wins, and a second context in the
/// same runtime reuses it, so the first context's realm stays alive until the
/// runtime is dropped.
///
/// # Errors
///
/// Returns an error if an intrinsic is missing from the realm or cannot be
/// stored.
pub(crate) fn pin(ctx: &Ctx<'_>) -> JsResult<()> {
    if ctx.userdata::<Pins>().is_some() {
        return Ok(());
    }
    let pins = capture(ctx)?;
    ctx.store_userdata(pins).map(drop).map_err(|_| {
        Error::new_from_js_message(
            "intrinsics",
            "userdata",
            "the runtime's userdata is borrowed",
        )
    })
}

enum Kind {
    Date,
    RegExp,
    Map,
    Set,
}

fn kind_of(value: &Value<'_>) -> Option<Kind> {
    if value.is_date() {
        Some(Kind::Date)
    } else if value.is_reg_exp() {
        Some(Kind::RegExp)
    } else if value.is_map() {
        Some(Kind::Map)
    } else if value.is_set() {
        Some(Kind::Set)
    } else {
        None
    }
}

/// Appends `value` if it is a `Date`, `RegExp`, `Map` or `Set` and the realm
/// is pinned; returns whether it did.
pub(super) fn write(out: &mut String, value: &Value<'_>, depth: usize, mode: Mode) -> bool {
    let Some(kind) = kind_of(value) else {
        return false;
    };
    let ctx = value.ctx();
    let Some(pins) = ctx.userdata::<Pins>().map(|pins| pins.clone()) else {
        return false;
    };
    match kind {
        Kind::Date => {
            let text = settled(ctx, pins[ISO].call::<_, String>((This(value.clone()),)))
                .unwrap_or_else(|| "Invalid Date".to_owned());
            put(out, mode, Style::Magenta, &text);
        }
        Kind::RegExp => {
            let text = regexp_text(value, &pins).unwrap_or_else(|| "[RegExp]".to_owned());
            put(out, mode, Style::Red, &text);
        }
        Kind::Map => write_collection(out, value, &pins[MAP_EACH], "Map", depth, mode),
        Kind::Set => write_collection(out, value, &pins[SET_EACH], "Set", depth, mode),
    }
    true
}

/// `/source/flags`, or `None` after clearing the exception a read threw.
fn regexp_text<'js>(value: &Value<'js>, pins: &Pins<'js>) -> Option<String> {
    let ctx = value.ctx();
    let source = settled(ctx, pins[SOURCE].call::<_, String>((This(value.clone()),)))?;
    let mut flags = String::new();
    for (getter, letter) in pins[FLAGS..MAP_EACH].iter().zip(FLAG_LETTERS.chars()) {
        if settled(ctx, getter.call::<_, bool>((This(value.clone()),)))? {
            flags.push(letter);
        }
    }
    Some(format!("/{source}/{flags}"))
}

/// `Name(n) { ... }`, a map's entries as `key => value`.
fn write_collection<'js>(
    out: &mut String,
    value: &Value<'js>,
    each: &Function<'js>,
    name: &str,
    depth: usize,
    mode: Mode,
) {
    if depth >= mode.max_depth {
        put(out, mode, Style::Cyan, &format!("[{name}]"));
        return;
    }
    let items = items(value, each);
    if items.is_empty() {
        let _ = write!(out, "{name}(0) {{}}");
        return;
    }

    let _ = write!(out, "{name}({}) {{ ", items.len());
    for (index, (item, key)) in items.iter().take(MAX_ENTRIES).enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        if name == "Map" {
            write_value(out, key, depth + 1, mode);
            out.push_str(" => ");
        }
        write_value(out, item, depth + 1, mode);
    }
    if items.len() > MAX_ENTRIES {
        // Entries precede it, so it starts with a separator.
        let mut written = true;
        #[allow(clippy::cast_precision_loss)]
        write_more(out, &mut written, (items.len() - MAX_ENTRIES) as f64, mode);
    }
    out.push_str(" }");
}

/// The `(value, key)` pairs `each` (a `forEach`) visits, as far as it got
/// before any exception, which is cleared.
fn items<'js>(value: &Value<'js>, each: &Function<'js>) -> Vec<(Value<'js>, Value<'js>)> {
    let ctx = value.ctx();
    let found = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&found);
    let visit = settled(
        ctx,
        Function::new(ctx.clone(), move |args: Rest<Value<'js>>| {
            let mut args = args.0.into_iter();
            if let (Some(item), Some(key)) = (args.next(), args.next()) {
                sink.borrow_mut().push((item, key));
            }
        }),
    );
    if let Some(visit) = visit {
        settled(ctx, each.call::<_, ()>((This(value.clone()), visit)));
    }
    found.take()
}
