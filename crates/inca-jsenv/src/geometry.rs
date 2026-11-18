// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The rect classes of the Geometry Interfaces standard: `DOMRectReadOnly`, `DOMRect` and the
//! alias `SVGRect`.
//!
//! An instance holds the four numbers and no own property. The edges are computed at each read.

use std::cell::Cell;

use rquickjs::convert::Coerced;
use rquickjs::function::{IntoJsFunc, ParamRequirement, Params, Rest};
use rquickjs::object::Property;
// The import is for the derive on `Rect`.
#[allow(clippy::disallowed_types)]
use rquickjs::JsLifetime;
use rquickjs::{Class, Ctx, Exception, FromJs, Object, Result as JsResult, Value};

use crate::class::{
    NEEDS_NEW, define_class, define_global, define_sized_method, define_this_accessor,
    define_this_getter, define_this_method, given, new_target_prototype, not_a, options_of,
    plain_class, receiver, receiver_named,
};
use crate::vault::{required_prototype, store_prototype};

/// The vault keys of the prototypes.
const READ_ONLY: &str = "DOMRectReadOnly";
const WRITABLE: &str = "DOMRect";

/// A `DOMRectReadOnly` or a `DOMRect` instance.
// `Rect` holds numbers and no JS value, so the derive is sound.
#[allow(clippy::disallowed_macros)]
#[derive(JsLifetime)]
pub(crate) struct Rect {
    x: Cell<f64>,
    y: Cell<f64>,
    width: Cell<f64>,
    height: Cell<f64>,
    read_only: bool,
}

plain_class!(Rect, "DOMRect");

impl Rect {
    fn new(read_only: bool, [x, y, width, height]: [f64; 4]) -> Self {
        Self {
            x: Cell::new(x),
            y: Cell::new(y),
            width: Cell::new(width),
            height: Cell::new(height),
            read_only,
        }
    }

    /// The minimum of `a` and `b`, `NaN` when either is.
    fn least(a: f64, b: f64) -> f64 {
        if a.is_nan() || b.is_nan() {
            f64::NAN
        } else {
            a.min(b)
        }
    }

    /// The maximum of `a` and `b`, `NaN` when either is.
    fn greatest(a: f64, b: f64) -> f64 {
        if a.is_nan() || b.is_nan() {
            f64::NAN
        } else {
            a.max(b)
        }
    }

    fn top(&self) -> f64 {
        Self::least(self.y.get(), self.y.get() + self.height.get())
    }

    fn right(&self) -> f64 {
        Self::greatest(self.x.get(), self.x.get() + self.width.get())
    }

    fn bottom(&self) -> f64 {
        Self::greatest(self.y.get(), self.y.get() + self.height.get())
    }

    fn left(&self) -> f64 {
        Self::least(self.x.get(), self.x.get() + self.width.get())
    }

    /// The eight values in the order of `toJSON`.
    fn values(&self) -> [(&'static str, f64); 8] {
        [
            ("x", self.x.get()),
            ("y", self.y.get()),
            ("width", self.width.get()),
            ("height", self.height.get()),
            ("top", self.top()),
            ("right", self.right()),
            ("bottom", self.bottom()),
            ("left", self.left()),
        ]
    }

    /// The prototype key of the interface of this rect.
    fn interface(&self) -> &'static str {
        Self::name(self.read_only)
    }

    /// The interface name, which is also the vault key, of the rects `read_only` names.
    fn name(read_only: bool) -> &'static str {
        if read_only { READ_ONLY } else { WRITABLE }
    }
}

/// `value` converted with `ToNumber`, `0` for an absent or `undefined` one.
fn number<'js>(ctx: &Ctx<'js>, value: Option<&Value<'js>>) -> JsResult<f64> {
    match value.filter(|value| !value.is_undefined()) {
        Some(value) => Ok(Coerced::<f64>::from_js(ctx, value.clone())?.0),
        None => Ok(0.0),
    }
}

/// The body of a constructor.
struct Construct {
    read_only: bool,
}

impl<'js> IntoJsFunc<'js, ()> for Construct {
    fn param_requirements() -> ParamRequirement {
        ParamRequirement::any()
    }

    fn call<'a>(&self, params: Params<'a, 'js>) -> JsResult<Value<'js>> {
        let ctx = params.ctx();
        if !params.is_constructor() {
            return Err(Exception::throw_type(ctx, NEEDS_NEW));
        }
        let mut values = [0.0; 4];
        for (index, slot) in values.iter_mut().enumerate() {
            *slot = number(ctx, params.arg(index).as_ref())?;
        }
        let key = Rect::name(self.read_only);
        let proto = new_target_prototype(&params, required_prototype(ctx, key)?)?;
        let rect = Rect::new(self.read_only, values);
        Ok(Class::instance_proto(rect, proto)?.into_value())
    }
}

/// A new `DOMRect` with the given values, for a caller outside a script.
///
/// # Errors
///
/// Returns an error if the realm has no rect classes.
pub fn dom_rect<'js>(
    ctx: &Ctx<'js>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> JsResult<Object<'js>> {
    let proto = required_prototype(ctx, WRITABLE)?;
    let rect = Rect::new(false, [x, y, width, height]);
    Ok(Class::instance_proto(rect, proto)?.into_inner())
}

/// A new rect of the interface of the rect `value` is, with its four values.
///
/// # Errors
///
/// Returns a `TypeError` if `value` is no rect.
pub(crate) fn copy<'js>(ctx: &Ctx<'js>, value: &Value<'js>) -> JsResult<Value<'js>> {
    let source = receiver::<Rect>(ctx, value)?;
    let source = source.borrow();
    let proto = required_prototype(ctx, source.interface())?;
    let rect = Rect::new(
        source.read_only,
        [
            source.x.get(),
            source.y.get(),
            source.width.get(),
            source.height.get(),
        ],
    );
    Ok(Class::instance_proto(rect, proto)?.into_value())
}

/// Whether `value` is a rect.
pub(crate) fn is_rect(value: &Value<'_>) -> bool {
    Class::<Rect>::from_value(value).is_ok()
}

/// The eight values and the interface name of `value` when it is a rect, `None` for any other
/// value. The values are read from the state of the instance, so no script code runs.
pub(crate) fn printed(value: &Value<'_>) -> Option<(&'static str, [(&'static str, f64); 8])> {
    let class = Class::<Rect>::from_value(value).ok()?;
    let rect = class.borrow();
    Some((rect.interface(), rect.values()))
}

/// The value of a `DOMRectInit` member, `0` when the member is `undefined`.
fn member<'js>(ctx: &Ctx<'js>, init: Option<&Object<'js>>, key: &str) -> JsResult<f64> {
    let value = init.map(|init| init.get::<_, Value>(key)).transpose()?;
    number(ctx, value.as_ref())
}

/// Defines `fromRect` of `class`, which builds an instance of the interface `read_only` names.
fn define_from_rect<'js>(
    ctx: &Ctx<'js>,
    class: &rquickjs::Function<'js>,
    read_only: bool,
) -> JsResult<()> {
    define_sized_method(
        ctx,
        class,
        "fromRect",
        0,
        move |ctx: Ctx<'js>, Rest(args): Rest<Value<'js>>| {
            let init = options_of(&ctx, given(&args, 0).cloned())?;
            // The members are read in the lexicographic order of the dictionary.
            let height = member(&ctx, init.as_ref(), "height")?;
            let width = member(&ctx, init.as_ref(), "width")?;
            let x = member(&ctx, init.as_ref(), "x")?;
            let y = member(&ctx, init.as_ref(), "y")?;
            let key = Rect::name(read_only);
            let rect = Rect::new(read_only, [x, y, width, height]);
            Ok::<_, rquickjs::Error>(
                Class::instance_proto(rect, required_prototype(&ctx, key)?)?.into_value(),
            )
        },
    )
    .map(drop)
}

/// Defines the globals `DOMRectReadOnly`, `DOMRect` and `SVGRect` and keeps the two prototypes in
/// the vault of `ctx`'s realm.
///
/// # Errors
///
/// Returns an error if the realm has no vault or a global cannot be defined.
pub(crate) fn install(ctx: &Ctx<'_>) -> JsResult<()> {
    let read_only_proto = Object::new(ctx.clone())?;
    let proto = Object::new(ctx.clone())?;
    proto.set_prototype(Some(&read_only_proto))?;
    store_prototype(ctx, READ_ONLY, &read_only_proto)?;
    store_prototype(ctx, WRITABLE, &proto)?;

    let read_only_class = define_class(
        ctx,
        &read_only_proto,
        READ_ONLY,
        0,
        Construct { read_only: true },
    )?;
    let class = define_class(ctx, &proto, WRITABLE, 0, Construct { read_only: false })?;
    class.set_prototype(Some(&read_only_class))?;

    define_read_only_members(ctx, &read_only_proto)?;
    define_writable_members(ctx, &proto)?;
    define_from_rect(ctx, &read_only_class, true)?;
    define_from_rect(ctx, &class, false)?;

    define_global(ctx, READ_ONLY, read_only_class)?;
    define_global(ctx, "SVGRect", class.clone())?;
    define_global(ctx, WRITABLE, class)
}

/// The getters and `toJSON` of `DOMRectReadOnly.prototype`.
fn define_read_only_members<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    type Read = fn(&Rect) -> f64;
    let getters: [(&str, Read); 8] = [
        ("x", |rect| rect.x.get()),
        ("y", |rect| rect.y.get()),
        ("width", |rect| rect.width.get()),
        ("height", |rect| rect.height.get()),
        ("top", Rect::top),
        ("right", Rect::right),
        ("bottom", Rect::bottom),
        ("left", Rect::left),
    ];
    for (key, read) in getters {
        define_this_getter(ctx, proto, key, move |ctx, this| {
            Ok(read(
                &receiver_named::<Rect>(ctx, &this, READ_ONLY)?.borrow(),
            ))
        })?;
    }
    define_this_method(ctx, proto, "toJSON", 0, |ctx, this, _| {
        let json = Object::new(ctx.clone())?;
        for (key, value) in receiver_named::<Rect>(ctx, &this, READ_ONLY)?
            .borrow()
            .values()
        {
            // `set` would run a setter inherited from Object.prototype.
            json.prop(
                key,
                Property::from(value).writable().enumerable().configurable(),
            )?;
        }
        Ok(json)
    })?;
    Ok(())
}

/// The `DOMRect` instance `this` is, or a `TypeError` for any other receiver.
fn writable<'js>(ctx: &Ctx<'js>, this: &Value<'js>) -> JsResult<Class<'js, Rect>> {
    let rect = receiver_named::<Rect>(ctx, this, WRITABLE)?;
    if rect.borrow().read_only {
        return Err(not_a(ctx, WRITABLE));
    }
    Ok(rect)
}

/// The accessors of `DOMRect.prototype`.
fn define_writable_members<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    type Field = fn(&Rect) -> &Cell<f64>;
    let fields: [(&str, Field); 4] = [
        ("x", |rect| &rect.x),
        ("y", |rect| &rect.y),
        ("width", |rect| &rect.width),
        ("height", |rect| &rect.height),
    ];
    for (key, field) in fields {
        define_this_accessor(
            ctx,
            proto,
            key,
            move |ctx, this| Ok(field(&writable(ctx, &this)?.borrow()).get()),
            move |ctx, this, value: Value<'js>| {
                let rect = writable(ctx, &this)?;
                let value = Coerced::<f64>::from_js(ctx, value)?;
                field(&rect.borrow()).set(value.0);
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::tests::{run, thrown};

    #[test]
    fn the_prototypes_and_constructors_are_linked() {
        assert_eq!(
            run(r"
                const links = [
                    Object.getPrototypeOf(DOMRect.prototype) === DOMRectReadOnly.prototype,
                    Object.getPrototypeOf(DOMRect) === DOMRectReadOnly,
                    Object.getPrototypeOf(DOMRectReadOnly.prototype) === Object.prototype,
                    SVGRect === DOMRect,
                ];
                links.join();
                "),
            "true,true,true,true"
        );
    }

    #[test]
    fn an_instance_has_no_own_key() {
        assert_eq!(
            run(r"
                const r = new DOMRect(1, 2, 3, 4);
                JSON.stringify([Object.getOwnPropertyNames(r), Object.keys(r), { ...r }]);
                "),
            "[[],[],{}]"
        );
    }

    #[test]
    fn the_prototypes_hold_the_members_in_order() {
        assert_eq!(
            run(r"
                const names = (p) => Object.getOwnPropertyNames(p).join();
                const flags = [DOMRectReadOnly.prototype, DOMRect.prototype].flatMap((p) =>
                    Object.getOwnPropertyNames(p)
                        .filter((k) => k !== 'constructor')
                        .map((k) => {
                            const d = Object.getOwnPropertyDescriptor(p, k);
                            return d.enumerable && d.configurable;
                        }),
                );
                `${names(DOMRectReadOnly.prototype)}|${names(DOMRect.prototype)}|${flags.every(Boolean)}`;
                "),
            "constructor,x,y,width,height,top,right,bottom,left,toJSON|constructor,x,y,width,height|true"
        );
    }

    /// The expectation lists `left,right,top,bottom`.
    #[rstest]
    #[case::positive("new DOMRect(10, 20, 30, 40)", "10,40,20,60")]
    #[case::negative_width("new DOMRect(0, 0, -10, 5)", "-10,0,0,5")]
    #[case::negative_height("new DOMRect(0, 0, 5, -10)", "0,5,-10,0")]
    #[case::nan_width("new DOMRect(0, 0, NaN, 5)", "NaN,NaN,0,5")]
    #[case::nan_height("new DOMRect(0, 0, 5, NaN)", "0,5,NaN,NaN")]
    fn the_edges_are_computed_from_the_values(#[case] source: &str, #[case] expected: &str) {
        assert_eq!(
            run(&format!(
                r"
                const r = {source};
                [r.left, r.right, r.top, r.bottom].join();
                "
            )),
            expected
        );
    }

    #[test]
    fn the_edges_follow_writes() {
        assert_eq!(
            run(r"
                const r = new DOMRect(10, 0, 30, 0);
                r.x = 5;
                const first = [r.left, r.right].join();
                r.width = -20;
                `${first}|${[r.left, r.right].join()}`;
                "),
            "5,35|-15,5"
        );
    }

    #[test]
    fn to_json_gives_a_plain_object_in_the_standard_order() {
        assert_eq!(
            run(r"
                const r = new DOMRect(1, 2, 3, 4);
                const json = r.toJSON();
                [
                    Object.getPrototypeOf(json) === Object.prototype,
                    Object.keys(json).join(),
                    JSON.stringify(r) === JSON.stringify(json),
                    JSON.stringify(r),
                ].join('|');
                "),
            "true|x,y,width,height,top,right,bottom,left|true|\
             {\"x\":1,\"y\":2,\"width\":3,\"height\":4,\"top\":2,\"right\":4,\"bottom\":6,\"left\":1}"
        );
    }

    #[test]
    fn the_constructor_converts_with_to_number() {
        assert_eq!(
            run(r"
                const r = new DOMRectReadOnly('3', undefined, null, true);
                [r.x, r.y, r.width, r.height].join();
                "),
            "3,0,0,1"
        );
    }

    #[rstest]
    #[case::writable("DOMRect()")]
    #[case::read_only("DOMRectReadOnly()")]
    fn a_call_without_new_throws(#[case] call: &str) {
        assert_eq!(
            thrown(call),
            "TypeError: the class constructor needs new. call it with new"
        );
    }

    #[test]
    fn a_subclass_gives_an_instance_of_the_subclass() {
        assert_eq!(
            run(r"
                class R extends DOMRect {}
                const r = new R(1, 2, 3, 4);
                `${r instanceof R}${Object.getPrototypeOf(r) === R.prototype}${r.right}`;
                "),
            "truetrue4"
        );
    }

    #[rstest]
    #[case::undefined("undefined", "0,0,0,0")]
    #[case::null("null", "0,0,0,0")]
    #[case::partial("{ x: 1 }", "1,0,0,0")]
    #[case::all("{ x: 1, y: 2, width: 3, height: 4 }", "1,2,3,4")]
    fn from_rect_reads_a_dictionary(#[case] init: &str, #[case] expected: &str) {
        assert_eq!(
            run(&format!(
                r"
                const r = DOMRect.fromRect({init});
                [r.x, r.y, r.width, r.height].join();
                "
            )),
            expected
        );
    }

    #[test]
    fn from_rect_reads_the_members_in_lexicographic_order() {
        assert_eq!(
            run(r"
                const log = [];
                const init = {};
                for (const k of ['x', 'y', 'width', 'height']) {
                    Object.defineProperty(init, k, {
                        get() {
                            log.push(k);
                            return 1;
                        },
                    });
                }
                DOMRect.fromRect(init);
                log.join();
                "),
            "height,width,x,y"
        );
    }

    #[test]
    fn from_rect_of_a_number_throws() {
        assert_eq!(
            thrown("DOMRect.fromRect(5)"),
            "TypeError: the options are not an object. pass an object or leave them out"
        );
    }

    #[test]
    fn from_rect_returns_its_own_interface() {
        assert_eq!(
            run(r"
                const a = DOMRect.fromRect({});
                const b = DOMRectReadOnly.fromRect({});
                `${Object.getPrototypeOf(a) === DOMRect.prototype}${Object.getPrototypeOf(b) === DOMRectReadOnly.prototype}`;
                "),
            "truetrue"
        );
    }

    #[test]
    fn a_setter_on_a_read_only_rect_throws() {
        assert_eq!(
            thrown(
                "Object.getOwnPropertyDescriptor(DOMRect.prototype, 'x').set.call(new DOMRectReadOnly(), 1)"
            ),
            "TypeError: the receiver is not a DOMRect. call the method on an instance"
        );
    }

    #[rstest]
    #[case::read_only("new DOMRectReadOnly()")]
    #[case::plain_object("{}")]
    fn a_getter_on_the_writable_prototype_throws(#[case] receiver: &str) {
        assert_eq!(
            thrown(&format!(
                "Object.getOwnPropertyDescriptor(DOMRect.prototype, 'x').get.call({receiver})"
            )),
            "TypeError: the receiver is not a DOMRect. call the method on an instance"
        );
    }

    #[test]
    fn a_getter_reads_either_interface() {
        assert_eq!(
            run(r"
                const get = Object.getOwnPropertyDescriptor(DOMRectReadOnly.prototype, 'left').get;
                `${get.call(new DOMRect(7, 0, 1, 0))}${get.call(new DOMRectReadOnly(8, 0, 1, 0))}`;
                "),
            "78"
        );
    }

    #[test]
    fn a_getter_on_a_plain_object_throws() {
        assert_eq!(
            thrown(
                "Object.getOwnPropertyDescriptor(DOMRectReadOnly.prototype, 'left').get.call({})"
            ),
            "TypeError: the receiver is not a DOMRectReadOnly. call the method on an instance"
        );
    }

    #[test]
    fn a_setter_stores_to_number() {
        assert_eq!(
            run(r"
                const r = new DOMRect();
                r.x = '7';
                r.height = true;
                `${r.x}${r.height}`;
                "),
            "71"
        );
    }

    #[test]
    fn a_setter_checks_the_receiver_before_the_value() {
        assert_eq!(
            thrown(
                "Object.getOwnPropertyDescriptor(DOMRect.prototype, 'x').set.call({}, Symbol())"
            ),
            "TypeError: the receiver is not a DOMRect. call the method on an instance"
        );
    }

    #[test]
    fn a_setter_throws_for_a_symbol() {
        assert_eq!(
            run(r"
                const r = new DOMRect(3);
                let kind = 'none';
                try {
                    r.x = Symbol();
                } catch (e) {
                    kind = e.constructor.name;
                }
                `${kind}${r.x}`;
                "),
            "TypeError3"
        );
    }

    #[test]
    fn a_rect_has_the_interface_name_as_its_tag() {
        assert_eq!(
            run("Object.prototype.toString.call(new DOMRectReadOnly())"),
            "[object DOMRectReadOnly]"
        );
    }

    #[test]
    fn to_json_ignores_a_setter_on_object_prototype() {
        assert_eq!(
            run(r"
                Object.defineProperty(Object.prototype, 'x', {
                    set() {},
                    configurable: true,
                });
                let json;
                try {
                    json = new DOMRectReadOnly(1, 2, 3, 4).toJSON();
                } finally {
                    delete Object.prototype.x;
                }
                const d = Object.getOwnPropertyDescriptor(json, 'x');
                `${json.x}${d.writable}${d.enumerable}${d.configurable}`;
                "),
            "1truetruetrue"
        );
    }

    #[test]
    fn dom_rect_builds_a_writable_rect() {
        let engine = crate::Engine::new().unwrap();
        let got: String = engine.with(|ctx| {
            let rect = super::dom_rect(&ctx, 1.0, 2.0, 3.0, 4.0).unwrap();
            ctx.globals().set("r", rect).unwrap();
            ctx.eval::<String, _>(
                "`${Object.getPrototypeOf(r) === DOMRect.prototype}${[r.x, r.y, r.width, r.height]}`",
            )
            .unwrap()
        });

        assert_eq!(got, "true1,2,3,4");
    }
}
