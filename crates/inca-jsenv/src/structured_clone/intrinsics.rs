// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the walk reads from the realm: the intrinsics captured before any script ran, the class
//! ids of the ordinary kinds, the kind of a value and the maps of platform objects.

use rquickjs::function::{Constructor, This};
use rquickjs::prelude::IntoArgs;
use rquickjs::{Array, Ctx, Exception, FromJs, Function, Object, Result as JsResult, Value};

use crate::buffer_source::{shared_class, store};
use crate::quickjs::{
    TYPED_ARRAY_NAMES, ValueExt, class_id, is_array_buffer, is_data_view, typed_array_name,
};
use crate::vault::vault;

/// The vault key of the captured intrinsics.
const KEY: &str = "structured-clone";

/// The names of the error constructors a clone may take.
pub(super) const ERRORS: [&str; 7] = [
    "Error",
    "EvalError",
    "RangeError",
    "ReferenceError",
    "SyntaxError",
    "TypeError",
    "URIError",
];

/// Reads the intrinsics before any script runs.
const INTRINSICS_JS: &str = r"
    (errorNames, typedNames) => {
        const get = (object, key) => Object.getOwnPropertyDescriptor(object, key).get;
        const typed = Object.getPrototypeOf(Uint8Array.prototype);
        const pick = (names) => {
            const out = { __proto__: null };
            for (const name of names) {
                out[name] = globalThis[name];
            }
            return out;
        };
        const flags = {
            __proto__: null,
            d: get(RegExp.prototype, 'hasIndices'),
            g: get(RegExp.prototype, 'global'),
            i: get(RegExp.prototype, 'ignoreCase'),
            m: get(RegExp.prototype, 'multiline'),
            s: get(RegExp.prototype, 'dotAll'),
            u: get(RegExp.prototype, 'unicode'),
            v: get(RegExp.prototype, 'unicodeSets'),
            y: get(RegExp.prototype, 'sticky'),
        };
        return {
            __proto__: null,
            Object,
            objectPrototype: Object.prototype,
            Map,
            Set,
            Date,
            RegExp,
            ArrayBuffer,
            DataView,
            DOMException,
            errors: pick(errorNames),
            typed: pick(typedNames),
            mapForEach: Map.prototype.forEach,
            mapSet: Map.prototype.set,
            mapGet: Map.prototype.get,
            setForEach: Set.prototype.forEach,
            setAdd: Set.prototype.add,
            numberValueOf: Number.prototype.valueOf,
            stringValueOf: String.prototype.valueOf,
            booleanValueOf: Boolean.prototype.valueOf,
            bigintValueOf: BigInt.prototype.valueOf,
            dateGetTime: Date.prototype.getTime,
            regexpSource: get(RegExp.prototype, 'source'),
            regexpFlags: flags,
            domName: get(DOMException.prototype, 'name'),
            domMessage: get(DOMException.prototype, 'message'),
            errorStack: get(Error.prototype, 'stack'),
            resize: ArrayBuffer.prototype.resize,
            byteLength: get(ArrayBuffer.prototype, 'byteLength'),
            maxByteLength: get(ArrayBuffer.prototype, 'maxByteLength'),
            typedLength: get(typed, 'length'),
            typedOffset: get(typed, 'byteOffset'),
            weakHas: WeakMap.prototype.has,
            records: [],
            samples: {
                __proto__: null,
                ordinary: {},
                number: new Number(0),
                string: new String(''),
                boolean: new Boolean(false),
                bigint: Object(0n),
                dom: new DOMException(),
                rect: new DOMRect(),
                weakMap: new WeakMap(),
            },
        };
    }
    ";

/// An array of `names`.
fn names<'js>(ctx: &Ctx<'js>, names: &[&str]) -> JsResult<Array<'js>> {
    let array = Array::new(ctx.clone())?;
    for (index, name) in names.iter().enumerate() {
        array.set(index, *name)?;
    }
    Ok(array)
}

/// The class ids of the kinds the walk tells apart by class.
#[derive(Clone, Copy)]
pub(super) struct Ids {
    ordinary: u32,
    number: u32,
    string: u32,
    boolean: u32,
    bigint: u32,
    dom: u32,
    rect: u32,
    weak_map: u32,
    shared: u32,
}

/// The kinds of value the walk tells apart.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    Proxy,
    Function,
    Boxed,
    Date,
    RegExp,
    Buffer,
    Shared,
    View,
    Map,
    Set,
    Error,
    DomException,
    Rect,
    Array,
    Ordinary,
    Refused,
}

/// The intrinsics object of a realm and the class ids.
pub(super) struct Toolkit<'js> {
    object: Object<'js>,
    ids: Ids,
}

/// Captures the intrinsics of `ctx`'s realm and keeps them in the vault.
///
/// # Errors
///
/// Returns an error if the realm lacks an intrinsic or has no vault.
pub(crate) fn install(ctx: &Ctx<'_>) -> JsResult<()> {
    let build: Function = ctx.eval(INTRINSICS_JS)?;
    let object: Object = build.call((names(ctx, &ERRORS)?, names(ctx, &TYPED_ARRAY_NAMES)?))?;
    let samples: Object = object.get("samples")?;
    let id = |key: &str| -> JsResult<u32> { Ok(class_id(&samples.get::<_, Value>(key)?)) };
    let ids = Object::new(ctx.clone())?;
    for key in [
        "ordinary", "number", "string", "boolean", "bigint", "dom", "rect", "weakMap",
    ] {
        ids.set(key, id(key)?)?;
    }
    object.set("ids", ids)?;
    object.remove("samples")?;
    vault(ctx)?.set(KEY, object)
}

impl<'js> Toolkit<'js> {
    /// The toolkit of `ctx`'s realm.
    ///
    /// # Errors
    ///
    /// Returns a `TypeError` if the realm has no stored state.
    pub(super) fn load(ctx: &Ctx<'js>) -> JsResult<Self> {
        let object: Object = vault(ctx)?.get::<_, Option<Object>>(KEY)?.ok_or_else(|| {
            Exception::throw_type(ctx, "no structuredClone state is stored. install it first")
        })?;
        let ids: Object = object.get("ids")?;
        let id = |key: &str| ids.get::<_, u32>(key);
        let ids = Ids {
            ordinary: id("ordinary")?,
            number: id("number")?,
            string: id("string")?,
            boolean: id("boolean")?,
            bigint: id("bigint")?,
            dom: id("dom")?,
            rect: id("rect")?,
            weak_map: id("weakMap")?,
            shared: shared_class(&store(ctx)?)?,
        };
        Ok(Self { object, ids })
    }

    /// The captured value `key`.
    pub(super) fn get<T: FromJs<'js>>(&self, key: &str) -> JsResult<T> {
        self.object.get(key)
    }

    /// The captured value `name` of the group `group`.
    pub(super) fn member<T: FromJs<'js>>(&self, group: &str, name: &str) -> JsResult<T> {
        self.object.get::<_, Object>(group)?.get(name)
    }

    /// Calls the captured function `key` with `args`, which start with the `This` of the call.
    pub(super) fn call<A: IntoArgs<'js>, R: FromJs<'js>>(&self, key: &str, args: A) -> JsResult<R> {
        self.get::<Function>(key)?.call(args)
    }

    /// Calls the captured getter `key` of the group `group` on `this`.
    pub(super) fn call_member<R: FromJs<'js>>(
        &self,
        group: &str,
        name: &str,
        this: &Value<'js>,
    ) -> JsResult<R> {
        self.member::<Function>(group, name)?
            .call((This(this.clone()),))
    }

    /// Builds an object with the captured constructor `key`.
    pub(super) fn construct<A: IntoArgs<'js>>(&self, key: &str, args: A) -> JsResult<Object<'js>> {
        self.get::<Constructor>(key)?.construct(args)
    }

    /// Builds an object with the constructor `name` of the group `group`.
    pub(super) fn construct_member<A: IntoArgs<'js>>(
        &self,
        group: &str,
        name: &str,
        args: A,
    ) -> JsResult<Object<'js>> {
        self.member::<Constructor>(group, name)?.construct(args)
    }

    /// The kind of the object `object`. The first test that holds decides: a proxy, a function, an
    /// array, an error, then the class of the object.
    pub(super) fn kind_of(&self, object: &Object<'js>) -> Kind {
        let ids = &self.ids;
        let value = object.as_value();
        if value.is_proxy() {
            return Kind::Proxy;
        }
        if value.is_function() {
            return Kind::Function;
        }
        if value.is_array() {
            return Kind::Array;
        }
        if value.is_error() {
            return Kind::Error;
        }
        let class = class_id(value);
        if class == ids.dom {
            return Kind::DomException;
        }
        // The classes whose instances hold plain Rust data share one class id.
        if class == ids.rect && crate::geometry::is_rect(value) {
            return Kind::Rect;
        }
        if [ids.number, ids.string, ids.boolean, ids.bigint].contains(&class) {
            return Kind::Boxed;
        }
        if value.is_date() {
            return Kind::Date;
        }
        if value.is_reg_exp() {
            return Kind::RegExp;
        }
        if value.is_map() {
            return Kind::Map;
        }
        if value.is_set() {
            return Kind::Set;
        }
        if is_array_buffer(value) {
            return Kind::Buffer;
        }
        if class == ids.shared {
            return Kind::Shared;
        }
        if typed_array_name(value).is_some() || is_data_view(value) {
            return Kind::View;
        }
        if class == ids.ordinary {
            return Kind::Ordinary;
        }
        Kind::Refused
    }

    /// The boxed kind's `valueOf` key.
    pub(super) fn value_of_key(&self, value: &Value<'js>) -> &'static str {
        let class = class_id(value);
        if class == self.ids.number {
            "numberValueOf"
        } else if class == self.ids.string {
            "stringValueOf"
        } else if class == self.ids.boolean {
            "booleanValueOf"
        } else {
            "bigintValueOf"
        }
    }

    /// Whether `object` is a key of a registered map. An object whose prototype is
    /// `Object.prototype` or `null` is never asked.
    pub(super) fn is_platform(&self, object: &Object<'js>) -> JsResult<bool> {
        let proto = object.get_prototype();
        let plain: Object = self.get("objectPrototype")?;
        if proto.is_none_or(|proto| proto == plain) {
            return Ok(false);
        }
        let records: Array = self.get("records")?;
        for map in records.iter::<Value>() {
            let found: bool = self.call("weakHas", (This(map?), object.clone()))?;
            if found {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Registers `map`, a `WeakMap`, for `structuredClone`. An ordinary object whose prototype is
/// neither `Object.prototype` nor `null` is refused when it is a key of the map, and any other
/// object is cloned as usual. A map stays registered for the life of the realm.
///
/// # Errors
///
/// Returns a `TypeError` if `map` is no `WeakMap` or if the realm has no stored state.
pub fn register_uncloneable<'js>(ctx: &Ctx<'js>, map: &Value<'js>) -> JsResult<()> {
    let toolkit = Toolkit::load(ctx)?;
    if class_id(map) != toolkit.ids.weak_map {
        return Err(Exception::throw_type(
            ctx,
            "the value is not a WeakMap. pass a WeakMap",
        ));
    }
    let records: Array = toolkit.get("records")?;
    records.set(records.len(), map.clone())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rquickjs::{Context, Runtime, Value};
    use rstest::rstest;

    use super::*;
    use crate::Engine;
    use crate::vault::create_vault;

    /// An engine whose script `setup` ran and whose `WeakMap`s `maps` are registered.
    fn registered(setup: &str, maps: &[&str]) -> Engine {
        let engine = Engine::new().unwrap();
        engine.eval::<()>(setup).unwrap();
        engine.with(|ctx| {
            for name in maps {
                let map: Value = ctx.globals().get(*name).unwrap();
                register_uncloneable(&ctx, &map).unwrap();
            }
        });
        engine
    }

    /// What `structuredClone(expression)` gives: `ok` and the prototype test, or the error text.
    fn cloned(engine: &Engine, expression: &str) -> String {
        engine
            .eval(&format!(
                r"
                try {{
                    const r = structuredClone({expression});
                    `ok ${{Object.getPrototypeOf(r) === Object.prototype}}`;
                }} catch (e) {{
                    `${{e.name}}: ${{e.message}}`;
                }}
                "
            ))
            .unwrap()
    }

    const REFUSED: &str = "DataCloneError: the object cannot be cloned. pass plain data";
    const SETUP: &str = r"
        class P {}
        globalThis.a = new P();
        globalThis.b = new P();
        globalThis.m1 = new WeakMap([[a, 1]]);
        globalThis.m2 = new WeakMap([[b, 1]]);
        ";

    #[test]
    fn an_object_that_is_a_key_of_a_registered_map_is_refused() {
        assert_eq!(cloned(&registered(SETUP, &["m1"]), "a"), REFUSED);
    }

    #[test]
    fn every_registered_map_is_asked() {
        assert_eq!(cloned(&registered(SETUP, &["m1", "m2"]), "b"), REFUSED);
    }

    #[test]
    fn a_map_registered_after_a_clone_refuses_its_objects() {
        let engine = registered(SETUP, &["m1"]);
        assert_eq!(cloned(&engine, "{}"), "ok true");
        engine.with(|ctx| {
            let map: Value = ctx.globals().get("m2").unwrap();
            register_uncloneable(&ctx, &map).unwrap();
        });

        assert_eq!(cloned(&engine, "b"), REFUSED);
    }

    #[rstest]
    #[case::no_record("new P()")]
    #[case::created_from_a_registered_object("Object.create(a)")]
    #[case::same_prototype_as_a_registered_object("Object.create(Object.getPrototypeOf(a))")]
    fn an_object_with_no_record_is_cloned_as_a_plain_object(#[case] expression: &str) {
        let engine = registered(SETUP, &["m1"]);

        assert_eq!(cloned(&engine, expression), "ok true");
    }

    #[test]
    fn a_replaced_has_is_not_called() {
        let engine = registered(
            r"
            class P {}
            globalThis.a = new P();
            globalThis.m1 = new WeakMap([[a, 1]]);
            globalThis.calls = 0;
            WeakMap.prototype.has = function () {
                calls += 1;
                return false;
            };
            ",
            &["m1"],
        );

        assert_eq!(
            engine
                .eval::<String>(
                    r"
                    let text;
                    try {
                        structuredClone(a);
                    } catch (e) {
                        text = e.message;
                    }
                    `${text}|${calls}`;
                    "
                )
                .unwrap(),
            "the object cannot be cloned. pass plain data|0"
        );
    }

    #[rstest]
    #[case::object("({})")]
    #[case::map("new Map()")]
    #[case::number("5")]
    fn registering_a_value_that_is_no_weak_map_throws(#[case] source: &str) {
        let engine = Engine::new().unwrap();
        engine.with(|ctx| {
            let value: Value = ctx.eval(source).unwrap();

            assert!(register_uncloneable(&ctx, &value).is_err());
            assert!(ctx.catch().is_error());
        });
    }

    #[test]
    fn registering_without_stored_state_throws_a_type_error() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            create_vault(&ctx).unwrap();
            let map: Value = ctx.eval("new WeakMap()").unwrap();
            register_uncloneable(&ctx, &map).unwrap_err();
            let error: Object = ctx.catch().into_object().unwrap();

            assert_eq!(
                (
                    error.get::<_, String>("name").unwrap(),
                    error.get::<_, String>("message").unwrap()
                ),
                (
                    "TypeError".to_string(),
                    "no structuredClone state is stored. install it first".to_string()
                )
            );
        });
    }
}
