// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Date`, `RegExp`, `Map`, `Set` and typed-array values.
//!
//! The text of a value of these types (its time, pattern, flags, entries,
//! kind and length) is read through intrinsics pinned by [`pin`], so reading
//! it runs no script code, whatever a script replaced or defined on the
//! value. Values held inside a `Map`, a `Set` or a typed array are printed
//! like any other value.

use rquickjs::{
    Array, Ctx, Function, Object, Result as JsResult, Value, function::This, object::Property,
};

use super::collections::{write_collection, write_typed_array};
use super::{Mode, put, settled};
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
