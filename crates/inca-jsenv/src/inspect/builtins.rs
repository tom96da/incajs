// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Date`, `RegExp`, `Map`, `Set` and typed-array values.
//!
//! The text of a value of these types (its time, pattern, flags, entries,
//! kind and length) is read through intrinsics pinned by [`pin`], so reading
//! it runs no script code, whatever a script replaced or defined on the
//! value. Values held inside a `Map`, a `Set` or a typed array are printed
//! like any other value.

use std::cell::RefCell;
use std::fmt::Write;
use std::rc::Rc;

use rquickjs::{
    Array, Ctx, Function, Object, Result as JsResult, Value,
    function::{Rest, This},
    object::Property,
};

use super::{
    MAX_ENTRIES, Mode, constructor_name, put, separate, settled, slot, write_more, write_slot,
    write_value,
};
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
/// The `Symbol.toStringTag` getter of the typed arrays' shared prototype: the kind of a typed
/// array, `undefined` for any other value.
const TYPED_TAG: usize = SET_EACH + 1;
/// The `length` getter of the typed arrays' shared prototype.
const TYPED_LENGTH: usize = TYPED_TAG + 1;

type Pins<'js> = Vec<Function<'js>>;

/// The global holding the pinned functions as a frozen array. [`pin`] defines it read-only
/// and non-configurable and freezes the array.
const PINS: &str = "__inca_intrinsics__";

/// Stores the intrinsics `console` uses to print a `Date`, `RegExp`, `Map`, `Set` or typed
/// array in the global [`PINS`]. The first pinning in a realm wins.
///
/// # Errors
///
/// Returns an error if an intrinsic is missing from the realm or the global cannot be
/// defined.
pub(crate) fn pin(ctx: &Ctx<'_>) -> JsResult<()> {
    let globals = ctx.globals();
    if globals.contains_key(PINS)? {
        return Ok(());
    }
    let pins: Array = ctx.eval(
        r"
        (() => {
            const getter = (key) =>
                Object.getOwnPropertyDescriptor(RegExp.prototype, key).get;
            const typed = (key) =>
                Object.getOwnPropertyDescriptor(Object.getPrototypeOf(Uint8Array.prototype), key)
                    .get;
            return Object.freeze([
                Date.prototype.toISOString,
                getter('source'),
                ...['hasIndices', 'global', 'ignoreCase', 'multiline', 'dotAll',
                    'unicode', 'unicodeSets', 'sticky'].map(getter),
                Map.prototype.forEach,
                Set.prototype.forEach,
                typed(Symbol.toStringTag),
                typed('length'),
            ]);
        })()
        ",
    )?;
    debug_assert_eq!(pins.len(), TYPED_LENGTH + 1);
    globals.prop(PINS, Property::from(pins))
}

enum Kind {
    Date,
    RegExp,
    Map,
    Set,
    /// A typed array of the kind named.
    TypedArray(String),
}

fn kind_of<'js>(value: &Value<'js>, pins: &Pins<'js>) -> Option<Kind> {
    if value.is_date() {
        Some(Kind::Date)
    } else if value.is_reg_exp() {
        Some(Kind::RegExp)
    } else if value.is_map() {
        Some(Kind::Map)
    } else if value.is_set() {
        Some(Kind::Set)
    } else {
        let tag = pins[TYPED_TAG].call::<_, Option<String>>((This(value.clone()),));
        settled(value.ctx(), tag).flatten().map(Kind::TypedArray)
    }
}

/// Appends `object` if it is a `Date`, `RegExp`, `Map`, `Set` or typed array and the realm
/// is pinned; returns whether it did.
pub(super) fn write(out: &mut String, object: &Object<'_>, depth: usize, mode: Mode) -> bool {
    let value = object.as_value();
    let ctx = value.ctx();
    let Ok(pins) = ctx.globals().get::<_, Pins>(PINS) else {
        return false;
    };
    let Some(kind) = kind_of(value, &pins) else {
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
        Kind::TypedArray(name) => {
            write_typed_array(out, object, &name, &pins[TYPED_LENGTH], depth, mode);
        }
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

/// `Name(n) [ ... ]`, the elements of a typed array of the kind `name`. `length_of` is the
/// pinned `length` getter.
fn write_typed_array<'js>(
    out: &mut String,
    object: &Object<'js>,
    name: &str,
    length_of: &Function<'js>,
    depth: usize,
    mode: Mode,
) {
    let own = constructor_name(object);
    if depth >= mode.max_depth {
        let text = match own.as_deref() {
            None => format!("[{name}: null prototype]"),
            Some(own) if own != name => format!("[{own} [{name}]]"),
            Some(_) => format!("[{name}]"),
        };
        put(out, mode, Style::Cyan, &text);
        return;
    }
    let length = settled(
        object.ctx(),
        length_of.call::<_, f64>((This(object.clone()),)),
    )
    .unwrap_or(0.0);
    let _ = match own.as_deref() {
        None => write!(out, "[{name}({length}): null prototype] "),
        Some(own) if own != name => write!(out, "{own}({length}) [{name}] "),
        Some(_) => write!(out, "{name}({length}) "),
    };
    if length < 1.0 {
        out.push_str("[]");
        return;
    }
    out.push_str("[ ");
    let mut written = false;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let count = length as usize;
    for index in 0..count.min(MAX_ENTRIES) {
        separate(out, &mut written);
        write_slot(out, slot(object, &index.to_string()), depth, mode);
    }
    if count > MAX_ENTRIES {
        #[allow(clippy::cast_precision_loss)]
        write_more(out, &mut written, (count - MAX_ENTRIES) as f64, mode);
    }
    out.push_str(" ]");
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
