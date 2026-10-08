// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Safe wrappers over `QuickJS` calls.
//!
//! This is the only module of the crate that imports `rquickjs::qjs` or
//! contains `unsafe`. The wrappers take and return owned `rquickjs` types and
//! report failure as [`rquickjs::Error::Exception`] with the exception left
//! pending. They run the script code that the underlying `QuickJS` call runs
//! and no more. They use no type of this crate.
//!
//! A panic that a Rust callback stored during a call stays with the runtime
//! until `rquickjs` next resumes it.

// The workspace denies `unsafe_code`, and this module is its one exception.
#![allow(unsafe_code)]

use std::mem::MaybeUninit;

use rquickjs::{Ctx, Error, IntoJs, Object, Result, Value, object::PropertyFlags, qjs};

/// What `Object.getOwnPropertyDescriptor` describes.
///
/// A data property has its content in `value`, and `getter` and `setter` are
/// `undefined`. An accessor property has `value` `undefined`, and each of
/// `getter` and `setter` is a function or `undefined`.
pub struct PropertyDescriptor<'js> {
    /// The `JS_PROP_*` bits of the property. The descriptor of a proxy's
    /// property can carry `JS_PROP_HAS_*` bits besides the attribute bits,
    /// and [`PropertyDescriptor::is_accessor`] reads the right bit in both
    /// cases.
    pub flags: PropertyFlags,
    pub value: Value<'js>,
    pub getter: Value<'js>,
    pub setter: Value<'js>,
}

impl PropertyDescriptor<'_> {
    /// Returns whether the property is an accessor property.
    #[must_use]
    pub fn is_accessor(&self) -> bool {
        self.flags & qjs::JS_PROP_GETSET.cast_signed() != 0
    }

    /// Returns whether the property is enumerable.
    #[must_use]
    pub fn is_enumerable(&self) -> bool {
        self.flags & qjs::JS_PROP_ENUMERABLE.cast_signed() != 0
    }
}

/// Property access on [`Object`].
pub trait ObjectExt<'js> {
    /// Get the descriptor of an own property, or `None` when the object has no
    /// such property. Neither a getter nor a setter runs.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Exception`] if the key cannot be converted or the
    /// lookup throws, as for a proxy's trap or a module namespace export that
    /// is not initialised yet. The key is any value: `null`, `undefined`, a
    /// boolean and a bigint name their string form.
    fn get_own_property_descriptor<K: IntoJs<'js>>(
        &self,
        key: K,
    ) -> Result<Option<PropertyDescriptor<'js>>>;
}

impl<'js> ObjectExt<'js> for Object<'js> {
    fn get_own_property_descriptor<K: IntoJs<'js>>(
        &self,
        key: K,
    ) -> Result<Option<PropertyDescriptor<'js>>> {
        let ctx = self.ctx();
        let atom = Atom::new(ctx, &key.into_js(ctx)?)?;
        let mut desc = MaybeUninit::<qjs::JSPropertyDescriptor>::uninit();
        // SAFETY: the context, the object and the atom are live. `desc` is a
        // writable descriptor that the call fills on a positive result.
        let found = unsafe {
            qjs::JS_GetOwnProperty(
                ctx.as_raw().as_ptr(),
                desc.as_mut_ptr(),
                self.as_value().as_raw(),
                atom.raw,
            )
        };
        match found {
            ..=-1 => Err(Error::Exception),
            0 => Ok(None),
            _ => {
                // SAFETY: a positive result filled `desc`, and its value,
                // getter and setter are new references that nothing else
                // releases. `Value::from_raw` takes each over, so each is
                // released once, when the returned descriptor drops.
                let (flags, value, getter, setter) = unsafe {
                    let desc = desc.assume_init();
                    (
                        desc.flags,
                        Value::from_raw(ctx.clone(), desc.value),
                        Value::from_raw(ctx.clone(), desc.getter),
                        Value::from_raw(ctx.clone(), desc.setter),
                    )
                };
                Ok(Some(PropertyDescriptor {
                    flags,
                    value,
                    getter,
                    setter,
                }))
            }
        }
    }
}

/// Class checks and proxy access on [`Value`].
///
/// A class check reads the object's class id and runs no script code. A
/// proxy has the class Proxy.
pub trait ValueExt<'js> {
    /// Returns whether the value is a `Date`.
    fn is_date(&self) -> bool;

    /// Returns whether the value is a `RegExp`.
    fn is_reg_exp(&self) -> bool;

    /// Returns whether the value is a `Map`.
    fn is_map(&self) -> bool;

    /// Returns whether the value is a `Set`.
    fn is_set(&self) -> bool;

    /// Get the target of a proxy. No trap runs. A proxy whose target is
    /// callable reports the type Function or Constructor, so
    /// [`Value::as_proxy`] returns `None` for it and this method serves it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Exception`] if the value is not a proxy or the proxy
    /// is revoked.
    fn proxy_target(&self) -> Result<Object<'js>>;
}

impl<'js> ValueExt<'js> for Value<'js> {
    fn is_date(&self) -> bool {
        has_class(self, qjs::JS_IsDate)
    }

    fn is_reg_exp(&self) -> bool {
        has_class(self, qjs::JS_IsRegExp)
    }

    fn is_map(&self) -> bool {
        has_class(self, qjs::JS_IsMap)
    }

    fn is_set(&self) -> bool {
        has_class(self, qjs::JS_IsSet)
    }

    fn proxy_target(&self) -> Result<Object<'js>> {
        let ctx = self.ctx();
        // SAFETY: the context and the value are live. The call returns a new
        // reference to the target, or the exception value, which owns
        // nothing. `Value::from_raw` takes over the reference, so it is
        // released once, when the value drops.
        let target = unsafe {
            Value::from_raw(
                ctx.clone(),
                qjs::JS_GetProxyTarget(ctx.as_raw().as_ptr(), self.as_raw()),
            )
        };
        if target.is_exception() {
            return Err(Error::Exception);
        }
        Object::from_value(target)
    }
}

fn has_class(value: &Value<'_>, check: unsafe extern "C" fn(qjs::JSValue) -> bool) -> bool {
    // SAFETY: each `JS_Is*` class check takes the value without changing its
    // reference count, cannot throw, and only reads the object's class id.
    unsafe { check(value.as_raw()) }
}

/// A `QuickJS` atom, released when dropped.
struct Atom<'a, 'js> {
    ctx: &'a Ctx<'js>,
    raw: qjs::JSAtom,
}

impl<'a, 'js> Atom<'a, 'js> {
    /// The atom of a JS value. A value that is not a string or a symbol
    /// converts to its string form, which can throw.
    fn new(ctx: &'a Ctx<'js>, key: &Value<'js>) -> Result<Self> {
        // SAFETY: the context and the value are live, and the call leaves the
        // value's reference count alone. It returns a new reference to the
        // atom, which `Drop` releases.
        let raw = unsafe { qjs::JS_ValueToAtom(ctx.as_raw().as_ptr(), key.as_raw()) };
        if raw == qjs::JS_ATOM_NULL {
            return Err(Error::Exception);
        }
        Ok(Self { ctx, raw })
    }
}

impl Drop for Atom<'_, '_> {
    fn drop(&mut self) {
        // SAFETY: `raw` is the one reference `new` took, released here once,
        // in the context that created it.
        unsafe { qjs::JS_FreeAtom(self.ctx.as_raw().as_ptr(), self.raw) };
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rquickjs::{Context, Persistent, Runtime};

    use super::*;

    /// Runs `body` with `source` evaluated first, in a fresh realm.
    fn with<R>(source: &str, body: impl FnOnce(&Ctx<'_>) -> R) -> R {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            ctx.eval::<(), _>(source).unwrap();
            body(&ctx)
        })
    }

    /// The descriptor of `key` on the object `expression` evaluates to.
    fn descriptor<'js, K: IntoJs<'js>>(
        ctx: &Ctx<'js>,
        expression: &str,
        key: K,
    ) -> Result<Option<PropertyDescriptor<'js>>> {
        let object: Object<'_> = ctx.eval(expression).unwrap();
        object.get_own_property_descriptor(key)
    }

    /// Which class checks `source`'s value passes: `Date`, `RegExp`, `Map`, `Set`.
    fn classes(ctx: &Ctx<'_>, source: &str) -> [bool; 4] {
        let v: Value<'_> = ctx.eval(source).unwrap();
        [v.is_date(), v.is_reg_exp(), v.is_map(), v.is_set()]
    }

    fn calls(ctx: &Ctx<'_>) -> i32 {
        ctx.eval("calls").unwrap()
    }

    #[test]
    fn a_data_property_has_its_value_and_its_attributes() {
        with("globalThis.calls = 0;", |ctx| {
            let d = descriptor(ctx, "({ a: 5 })", "a").unwrap().unwrap();

            assert_eq!(d.value.as_int(), Some(5));
            assert!(d.getter.is_undefined() && d.setter.is_undefined());
            assert!(!d.is_accessor());
            assert_eq!(d.flags & qjs::JS_PROP_C_W_E.cast_signed(), 7);
        });
    }

    #[test]
    fn attributes_follow_define_property() {
        with("", |ctx| {
            let d = descriptor(ctx, "Object.defineProperty({}, 'a', { value: 1 })", "a")
                .unwrap()
                .unwrap();

            assert_eq!(d.flags & qjs::JS_PROP_C_W_E.cast_signed(), 0);
        });
    }

    #[test]
    fn an_accessor_names_its_functions_and_runs_none() {
        with("globalThis.calls = 0;", |ctx| {
            let source = "Object.defineProperties({}, { \
                g: { get() { calls++; }, enumerable: true }, \
                s: { set(v) { calls++; } }, \
                b: { get() { calls++; }, set(v) { calls++; } }, \
                n: { get: undefined, set: undefined } })";
            let object: Object<'_> = ctx.eval(source).unwrap();
            let read = |key: &str| object.get_own_property_descriptor(key).unwrap().unwrap();
            let (g, s, b, n) = (read("g"), read("s"), read("b"), read("n"));

            for d in [&g, &s, &b, &n] {
                assert!(d.is_accessor());
                assert!(d.value.is_undefined());
            }
            assert!(g.is_enumerable());
            assert!(!s.is_enumerable());
            assert!(g.getter.is_function() && g.setter.is_undefined());
            assert!(s.getter.is_undefined() && s.setter.is_function());
            assert!(b.getter.is_function() && b.setter.is_function());
            assert!(n.getter.is_undefined() && n.setter.is_undefined());
            assert_eq!(calls(ctx), 0);
        });
    }

    #[test]
    fn a_missing_property_is_none_and_an_inherited_one_too() {
        with("globalThis.calls = 0;", |ctx| {
            assert!(descriptor(ctx, "({})", "a").unwrap().is_none());
            let inherited = "Object.create({ get a() { calls++; } })";
            assert!(descriptor(ctx, inherited, "a").unwrap().is_none());
            assert_eq!(calls(ctx), 0);
            assert!(!ctx.has_exception());
        });
    }

    #[test]
    fn keys_of_every_kind_find_their_own_property() {
        with("globalThis.calls = 0;", |ctx| {
            // The two keys differ only when a UTF-8 byte string is read as
            // Latin-1.
            let object = "({ '\u{c3}\u{a9}': 2, get '\u{e9}'() { calls++; } })";
            let first = descriptor(ctx, object, "\u{c3}\u{a9}").unwrap().unwrap();
            let second = descriptor(ctx, object, "\u{e9}").unwrap().unwrap();

            assert_eq!(first.value.as_int(), Some(2));
            assert!(second.is_accessor());

            let numeric = descriptor(ctx, "({ 7: 'x', get 8() { calls++; } })", 7)
                .unwrap()
                .unwrap();
            assert!(!numeric.is_accessor());
            let as_text = descriptor(ctx, "({ 7: 'x', get 8() { calls++; } })", "8")
                .unwrap()
                .unwrap();
            assert!(as_text.is_accessor());

            let symbol: Value<'_> = ctx.eval("globalThis.s = Symbol('s')").unwrap();
            let keyed = descriptor(ctx, "({ get [s]() { calls++; } })", symbol)
                .unwrap()
                .unwrap();
            assert!(keyed.is_accessor());
            assert_eq!(calls(ctx), 0);
        });
    }

    #[test]
    fn any_value_is_a_key_by_its_string_form() {
        with("", |ctx| {
            let object = "({ null: 1, undefined: 2, true: 3, 1: 4 })";
            for (key, expected) in [("null", 1), ("undefined", 2), ("true", 3), ("1n", 4)] {
                let key: Value<'_> = ctx.eval(key).unwrap();
                let d = descriptor(ctx, object, key).unwrap().unwrap();

                assert_eq!(d.value.as_int(), Some(expected));
            }
            assert!(!ctx.has_exception());
        });
    }

    #[test]
    fn array_elements_and_holes_are_properties_or_none() {
        with("", |ctx| {
            assert!(
                descriptor(ctx, "[1, , 3]", 0)
                    .unwrap()
                    .unwrap()
                    .value
                    .is_int()
            );
            assert!(descriptor(ctx, "[1, , 3]", 1).unwrap().is_none());
            assert!(descriptor(ctx, "[1, , 3]", "length").unwrap().is_some());
        });
    }

    #[test]
    fn a_proxy_runs_its_descriptor_trap_and_its_failure_is_an_exception() {
        with("globalThis.calls = 0;", |ctx| {
            let trap = "new Proxy({}, { getOwnPropertyDescriptor() { calls++; throw 1; } })";
            let result = descriptor(ctx, trap, "a");

            assert!(matches!(result, Err(Error::Exception)));
            assert_eq!(calls(ctx), 1);
            drop(ctx.catch());
            assert!(!ctx.has_exception());
        });
    }

    #[test]
    fn an_uninitialised_module_export_is_an_exception() {
        with("", |ctx| {
            drop(rquickjs::Module::evaluate(
                ctx.clone(),
                "m",
                "import * as self from 'm'; globalThis.ns = self; throw 1; export let x = 1;",
            ));
            drop(ctx.catch());
            let result = descriptor(ctx, "ns", "x");

            assert!(matches!(result, Err(Error::Exception)));
            assert!(ctx.has_exception());
            drop(ctx.catch());
            assert!(!ctx.has_exception());
        });
    }

    #[test]
    fn a_key_that_cannot_be_converted_is_an_exception() {
        with("", |ctx| {
            let key: Value<'_> = ctx
                .eval("({ toString() { throw new Error('no'); } })")
                .unwrap();
            let result = descriptor(ctx, "({})", key);

            assert!(matches!(result, Err(Error::Exception)));
            drop(ctx.catch());
            assert!(!ctx.has_exception());
        });
    }

    #[test]
    fn the_class_checks_name_their_class() {
        with("", |ctx| {
            let check = |source: &str| classes(ctx, source);

            assert_eq!(check("new Date(0)"), [true, false, false, false]);
            assert_eq!(check("/x/g"), [false, true, false, false]);
            assert_eq!(check("new Map()"), [false, false, true, false]);
            assert_eq!(check("new Set()"), [false, false, false, true]);
            assert_eq!(check("({})"), [false; 4]);
            assert_eq!(check("[]"), [false; 4]);
            assert_eq!(check("1"), [false; 4]);
            assert_eq!(check("'x'"), [false; 4]);
            assert_eq!(check("undefined"), [false; 4]);
        });
    }

    #[test]
    fn the_class_checks_look_at_the_class_and_not_the_prototype() {
        with("", |ctx| {
            let check = |source: &str| classes(ctx, source);

            assert_eq!(check("Object.create(Map.prototype)"), [false; 4]);
            assert_eq!(check("Object.create(Date.prototype)"), [false; 4]);
            assert_eq!(
                check("new (class extends Map {})()"),
                [false, false, true, false]
            );
            assert_eq!(
                check("const d = new Date(0); Object.setPrototypeOf(d, null); d"),
                [true, false, false, false]
            );
            assert_eq!(check("new Proxy(new Map(), {})"), [false; 4]);
            assert_eq!(check("new Proxy(new Date(0), {})"), [false; 4]);
        });
    }

    #[test]
    fn a_proxy_gives_its_target_without_a_trap() {
        with(
            "globalThis.calls = 0; globalThis.spy = (t) => new Proxy(t, new Proxy({}, { get() { calls++; } }));",
            |ctx| {
                for (source, check) in [
                    ("spy({ a: 1 })", "t.a === 1"),
                    ("spy(function f() {})", "typeof t === 'function'"),
                    ("spy([1, 2])", "Array.isArray(t)"),
                    ("spy(new Map())", "t instanceof Map"),
                    ("spy(class C {})", "typeof t === 'function'"),
                ] {
                    ctx.eval::<(), _>(format!(
                        "globalThis.p = {source}; globalThis.t = undefined;"
                    ))
                    .unwrap();
                    let proxy: Value<'_> = ctx.globals().get("p").unwrap();
                    assert!(proxy.is_proxy(), "{source}");
                    let target = proxy.proxy_target().unwrap();
                    ctx.globals().set("t", target).unwrap();

                    assert!(ctx.eval::<bool, _>(check).unwrap(), "{source}");
                }
                assert_eq!(calls(ctx), 0);
            },
        );
    }

    #[test]
    fn a_proxy_chain_gives_one_target_per_step() {
        with(
            "globalThis.inner = {}; globalThis.outer = new Proxy(new Proxy(inner, {}), {});",
            |ctx| {
                let outer: Value<'_> = ctx.globals().get("outer").unwrap();
                let middle = outer.proxy_target().unwrap().into_value();
                assert!(middle.is_proxy());
                let last = middle.proxy_target().unwrap();

                assert!(!last.as_value().is_proxy());
                ctx.globals().set("last", last).unwrap();
                assert!(ctx.eval::<bool, _>("last === inner").unwrap());
            },
        );
    }

    #[test]
    fn a_revoked_or_missing_proxy_is_an_exception() {
        with(
            "globalThis.r = Proxy.revocable({}, {}); r.revoke();",
            |ctx| {
                let revoked: Value<'_> = ctx.eval("r.proxy").unwrap();
                assert!(matches!(revoked.proxy_target(), Err(Error::Exception)));
                assert!(ctx.has_exception());
                drop(ctx.catch());
                assert!(!ctx.has_exception());

                let plain: Value<'_> = ctx.eval("({})").unwrap();
                assert!(matches!(plain.proxy_target(), Err(Error::Exception)));
                drop(ctx.catch());
                let number: Value<'_> = ctx.eval("1").unwrap();
                assert!(matches!(number.proxy_target(), Err(Error::Exception)));
                drop(ctx.catch());
                assert!(!ctx.has_exception());
            },
        );
    }

    #[test]
    fn the_proxy_target_read_releases_what_it_takes() {
        let (before, after) = around(|ctx| {
            let objs: Object<'_> = ctx.globals().get("objs").unwrap();
            for name in ["proxy", "data"] {
                let v: Value<'_> = objs.get(name).unwrap();
                if v.proxy_target().is_err() {
                    drop(ctx.catch());
                }
            }
        });

        assert_eq!(before, after);
    }

    /// The counters a leak moves.
    fn counters(runtime: &Runtime) -> [i64; 5] {
        runtime.run_gc();
        let usage = runtime.memory_usage();
        [
            usage.atom_count,
            usage.str_count,
            usage.obj_count,
            usage.malloc_count,
            usage.memory_used_count,
        ]
    }

    /// Calls `each` once to warm up, then 10 000 times, and returns the
    /// counters before and after.
    fn around(each: impl Fn(&Ctx<'_>)) -> ([i64; 5], [i64; 5]) {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            ctx.eval::<(), _>(
                "globalThis.objs = { data: { a: 1 }, get: { get a() {} }, set: { set a(v) {} }, \
                 both: { get a() {}, set a(v) {} }, none: Object.defineProperty({}, 'a', { get: undefined }), \
                 date: new Date(0), map: new Map(), proxy: new Proxy({}, { getOwnPropertyDescriptor() { throw 1; } }) };",
            )
            .unwrap();
            each(&ctx);
        });
        let before = counters(&runtime);
        context.with(|ctx| {
            for _ in 0..10_000 {
                each(&ctx);
            }
        });
        (before, counters(&runtime))
    }

    /// Reads `key` of `objs[name]` and drops the result, clearing a
    /// pending exception.
    fn read(ctx: &Ctx<'_>, name: &str, key: &str) {
        let objs: Object<'_> = ctx.globals().get("objs").unwrap();
        let object: Object<'_> = objs.get(name).unwrap();
        if object.get_own_property_descriptor(key).is_err() {
            drop(ctx.catch());
        }
    }

    #[test]
    fn every_outcome_of_the_descriptor_read_releases_what_it_takes() {
        for (name, key) in [
            ("data", "a"),
            ("get", "a"),
            ("set", "a"),
            ("both", "a"),
            ("none", "a"),
            ("data", "missing"),
            ("proxy", "a"),
            ("data", "\u{e9}"),
        ] {
            let (before, after) = around(|ctx| read(ctx, name, key));

            assert_eq!(before, after, "{name} {key}");
        }
    }

    #[test]
    fn a_key_seen_once_does_not_stay_interned() {
        // A repeated key keeps its atom alive anyway, so each call gets a new one.
        let next = std::cell::Cell::new(0);
        let (before, after) = around(|ctx| {
            let n = next.replace(next.get() + 1);
            read(ctx, "data", &format!("key{n}"));
        });

        assert_eq!(before, after);
    }

    #[test]
    fn the_class_checks_release_nothing_they_did_not_take() {
        let (before, after) = around(|ctx| {
            let objs: Object<'_> = ctx.globals().get("objs").unwrap();
            for name in ["date", "map", "data"] {
                let v: Value<'_> = objs.get(name).unwrap();
                let _ = (v.is_date(), v.is_reg_exp(), v.is_map(), v.is_set());
            }
        });

        assert_eq!(before, after);
    }

    #[test]
    fn a_value_that_is_kept_alive_moves_a_counter() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        let before = counters(&runtime);
        let kept: Vec<Persistent<Object<'static>>> = context.with(|ctx| {
            (0..1000)
                .map(|_| Persistent::save(&ctx, ctx.eval::<Object<'_>, _>("({})").unwrap()))
                .collect()
        });
        let during = counters(&runtime);

        assert_ne!(before, during, "the counters would miss a leak");
        context.with(|ctx| {
            for object in kept {
                drop(object.restore(&ctx).unwrap());
            }
        });
        assert_eq!(before, counters(&runtime));
    }
}
