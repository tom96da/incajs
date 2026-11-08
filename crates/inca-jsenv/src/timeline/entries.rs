// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The performance timeline: the classes `PerformanceEntry`, `PerformanceMark` and
//! `PerformanceMeasure`, and the members of `Performance` that read and clear entries.
//!
//! The timeline is a JS array in the vault that holds the entry objects, so an entry keeps its
//! identity. Every element of the array is defined as an own data property, so a script that adds
//! an indexed setter to `Array.prototype` or `Object.prototype` changes nothing here. The `detail`
//! of an entry sits in a `WeakMap` in the vault. Rust calls the `get` and `set` functions it
//! captured at install, so a script that replaces `WeakMap.prototype` changes nothing either.

use std::cmp::Ordering;

#[allow(clippy::disallowed_types)]
use rquickjs::JsLifetime;
use rquickjs::convert::Coerced;
use rquickjs::function::{Constructor, Rest, This};
use rquickjs::object::Property;
use rquickjs::{Array, Class, Ctx, FromJs, Function, IntoJs, Object, Result as JsResult, Value};

use crate::class::{
    define_class, define_global, define_method, define_this_getter, define_this_method, given,
    illegal_constructor, plain_class, receiver, require,
};
use crate::time::PerformanceInstance;
use crate::vault::vault;

/// The vault key of the object that holds the prototypes of the mark and the measure.
const CLASSES: &str = "performance-classes";

/// The vault key of the array of entries in creation order.
const TIMELINE: &str = "timeline";

/// The vault keys of the `WeakMap` of details and of its `get` and `set` functions.
const DETAILS: &str = "details";
const DETAILS_GET: &str = "details-get";
const DETAILS_SET: &str = "details-set";

/// The kind of an entry.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Kind {
    Mark,
    Measure,
}

impl Kind {
    fn entry_type(self) -> &'static str {
        match self {
            Self::Mark => "mark",
            Self::Measure => "measure",
        }
    }
}

/// The values of an entry, fixed when it is made.
pub(super) struct EntryData {
    pub(super) name: String,
    pub(super) kind: Kind,
    pub(super) start_time: f64,
    pub(super) duration: f64,
}

/// A `PerformanceMark` or a `PerformanceMeasure`. The prototype tells which.
#[allow(clippy::disallowed_macros)]
#[derive(JsLifetime)]
pub(super) struct EntryInstance(pub(super) EntryData);

plain_class!(EntryInstance, "PerformanceEntry");

/// `value` converted with `ToString`.
pub(super) fn text<'js>(ctx: &Ctx<'js>, value: &Value<'js>) -> JsResult<String> {
    Ok(Coerced::<String>::from_js(ctx, value.clone())?.0)
}

/// The prototype of the entries of `kind`.
pub(super) fn prototype<'js>(ctx: &Ctx<'js>, kind: Kind) -> JsResult<Object<'js>> {
    let classes: Object = vault(ctx)?.get(CLASSES)?;
    classes.get(kind.entry_type())
}

/// A new entry with the prototype `proto`, whose detail is `detail`.
pub(super) fn entry<'js>(
    ctx: &Ctx<'js>,
    data: EntryData,
    detail: Value<'js>,
    proto: Object<'js>,
) -> JsResult<Class<'js, EntryInstance>> {
    let entry = Class::instance_proto(EntryInstance(data), proto)?;
    let vault = vault(ctx)?;
    let set: Function = vault.get(DETAILS_SET)?;
    set.call::<_, Value>((
        This(vault.get::<_, Object>(DETAILS)?),
        entry.clone(),
        detail,
    ))?;
    Ok(entry)
}

/// The detail of `entry`.
fn detail<'js>(ctx: &Ctx<'js>, entry: Class<'js, EntryInstance>) -> JsResult<Value<'js>> {
    let vault = vault(ctx)?;
    let get: Function = vault.get(DETAILS_GET)?;
    get.call((This(vault.get::<_, Object>(DETAILS)?), entry))
}

/// Defines `value` as the next element of `list`, which no setter of a prototype can intercept.
fn push<'js>(list: &Array<'js>, value: Value<'js>) -> JsResult<()> {
    list.as_object().prop(
        u32::try_from(list.len()).map_err(|_| rquickjs::Error::Unknown)?,
        Property::from(value).writable().enumerable().configurable(),
    )
}

/// Appends `entry` to the timeline.
pub(super) fn append<'js>(ctx: &Ctx<'js>, entry: Value<'js>) -> JsResult<()> {
    let timeline: Array = vault(ctx)?.get(TIMELINE)?;
    push(&timeline, entry)
}

/// The entries of the timeline in creation order.
pub(super) fn timeline_entries<'js>(ctx: &Ctx<'js>) -> JsResult<Vec<Class<'js, EntryInstance>>> {
    let timeline: Array = vault(ctx)?.get(TIMELINE)?;
    (0..timeline.len())
        .map(|index| {
            let value: Value = timeline.get(index)?;
            Class::from_value(&value)
        })
        .collect()
}

/// The entries that `keep` accepts, sorted by start time. Entries of one start time stay in
/// creation order.
fn entries<'js>(ctx: &Ctx<'js>, keep: impl Fn(&EntryData) -> bool) -> JsResult<Array<'js>> {
    let mut found: Vec<_> = timeline_entries(ctx)?
        .into_iter()
        .filter(|class| keep(&class.borrow().0))
        .collect();
    found.sort_by(|a, b| {
        let (a, b) = (a.borrow().0.start_time, b.borrow().0.start_time);
        a.partial_cmp(&b).unwrap_or(Ordering::Equal)
    });
    let list = Array::new(ctx.clone())?;
    for class in found {
        push(&list, class.into_value())?;
    }
    Ok(list)
}

/// `performance.getEntriesByName()`.
fn entries_by_name<'js>(ctx: &Ctx<'js>, args: &[Value<'js>]) -> JsResult<Array<'js>> {
    require(ctx, "getEntriesByName", args, 1)?;
    let name = text(ctx, &args[0])?;
    let entry_type = given(args, 1).map(|value| text(ctx, value)).transpose()?;
    entries(ctx, |data| {
        data.name == name
            && entry_type
                .as_ref()
                .is_none_or(|entry_type| entry_type == data.kind.entry_type())
    })
}

/// `performance.getEntriesByType()`.
fn entries_by_type<'js>(ctx: &Ctx<'js>, args: &[Value<'js>]) -> JsResult<Array<'js>> {
    require(ctx, "getEntriesByType", args, 1)?;
    let entry_type = text(ctx, &args[0])?;
    entries(ctx, |data| data.kind.entry_type() == entry_type)
}

/// Removes the entries of `kind` named by the first argument, or all of them without one.
fn clear<'js>(ctx: &Ctx<'js>, kind: Kind, args: &[Value<'js>]) -> JsResult<()> {
    let name = given(args, 0).map(|value| text(ctx, value)).transpose()?;
    let kept = Array::new(ctx.clone())?;
    for class in timeline_entries(ctx)? {
        let data = &class.borrow().0;
        let removed = data.kind == kind && name.as_ref().is_none_or(|name| *name == data.name);
        if !removed {
            push(&kept, class.clone().into_value())?;
        }
    }
    vault(ctx)?.set(TIMELINE, kept)
}

/// Defines `run` as the method `name` of `proto`, which checks that `this` is a `Performance`.
fn method<'js, R: IntoJs<'js> + 'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    name: &str,
    length: usize,
    run: impl Fn(&Ctx<'js>, &[Value<'js>]) -> JsResult<R> + 'js,
) -> JsResult<()> {
    let function = Function::new(
        ctx.clone(),
        move |ctx: Ctx<'js>, This(this): This<Value<'js>>, Rest(args): Rest<Value<'js>>| {
            receiver::<PerformanceInstance>(&ctx, &this)?;
            run(&ctx, &args)
        },
    )?
    .with_length(length)?;
    define_method(proto, name, function)
}

/// Defines the getter `key` of `proto`, which reads the entry that `this` is.
fn entry_getter<'js, R: IntoJs<'js> + 'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    key: &str,
    read: impl Fn(&EntryData) -> R + 'js,
) -> JsResult<()> {
    define_this_getter(ctx, proto, key, move |ctx, this| {
        Ok(read(&receiver::<EntryInstance>(ctx, &this)?.borrow().0))
    })
}

/// Defines the classes of the entries as globals, and the members of `performance` that make,
/// read and clear entries on `performance_proto`, the prototype of `Performance`.
///
/// # Errors
///
/// Returns an error if the realm has no vault or a global cannot be defined.
pub(crate) fn install<'js>(ctx: &Ctx<'js>, performance_proto: &Object<'js>) -> JsResult<()> {
    let vault = vault(ctx)?;
    let weak_map: Constructor = ctx.globals().get("WeakMap")?;
    let details: Object = weak_map.construct(())?;
    vault.set(DETAILS_GET, details.get::<_, Function>("get")?)?;
    vault.set(DETAILS_SET, details.get::<_, Function>("set")?)?;
    vault.set(DETAILS, details)?;
    vault.set(TIMELINE, Array::new(ctx.clone())?)?;

    let entry_proto = Object::new(ctx.clone())?;
    let entry_class = define_class(
        ctx,
        &entry_proto,
        "PerformanceEntry",
        0,
        illegal_constructor(),
    )?;
    entry_getter(ctx, &entry_proto, "name", |data| data.name.clone())?;
    entry_getter(ctx, &entry_proto, "entryType", |data| {
        data.kind.entry_type()
    })?;
    entry_getter(ctx, &entry_proto, "startTime", |data| data.start_time)?;
    entry_getter(ctx, &entry_proto, "duration", |data| data.duration)?;
    define_this_method(ctx, &entry_proto, "toJSON", |ctx, this| {
        let entry = receiver::<EntryInstance>(ctx, &this)?;
        let data = &entry.borrow().0;
        let json = Object::new(ctx.clone())?;
        json.set("name", data.name.as_str())?;
        json.set("entryType", data.kind.entry_type())?;
        json.set("startTime", data.start_time)?;
        json.set("duration", data.duration)?;
        Ok(json)
    })?;

    let mark_proto = Object::new(ctx.clone())?;
    let mark_class = define_class(
        ctx,
        &mark_proto,
        "PerformanceMark",
        1,
        super::record::MarkConstructor,
    )?;
    mark_class.set_prototype(Some(&entry_class))?;
    mark_proto.set_prototype(Some(&entry_proto))?;

    let measure_proto = Object::new(ctx.clone())?;
    let measure_class = define_class(
        ctx,
        &measure_proto,
        "PerformanceMeasure",
        0,
        illegal_constructor(),
    )?;
    measure_class.set_prototype(Some(&entry_class))?;
    measure_proto.set_prototype(Some(&entry_proto))?;

    for proto in [&mark_proto, &measure_proto] {
        define_this_getter(ctx, proto, "detail", |ctx, this| {
            detail(ctx, receiver::<EntryInstance>(ctx, &this)?)
        })?;
    }
    let classes = Object::new(ctx.clone())?;
    classes.set_prototype(None)?;
    classes.set(Kind::Mark.entry_type(), mark_proto)?;
    classes.set(Kind::Measure.entry_type(), measure_proto)?;
    vault.set(CLASSES, classes)?;
    define_global(ctx, "PerformanceEntry", entry_class)?;
    define_global(ctx, "PerformanceMark", mark_class)?;
    define_global(ctx, "PerformanceMeasure", measure_class)?;

    method(ctx, performance_proto, "mark", 1, super::record::mark)?;
    method(ctx, performance_proto, "measure", 1, super::record::measure)?;
    method(ctx, performance_proto, "getEntries", 0, |ctx, _| {
        entries(ctx, |_| true)
    })?;
    method(
        ctx,
        performance_proto,
        "getEntriesByName",
        1,
        entries_by_name,
    )?;
    method(
        ctx,
        performance_proto,
        "getEntriesByType",
        1,
        entries_by_type,
    )?;
    method(ctx, performance_proto, "clearMarks", 0, |ctx, args| {
        clear(ctx, Kind::Mark, args)
    })?;
    method(ctx, performance_proto, "clearMeasures", 0, |ctx, args| {
        clear(ctx, Kind::Measure, args)
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::tests::{around, engine_on_test_clock, run_at, thrown_at};

    const FOREIGN: &str =
        "TypeError: the receiver is not a Performance. call the method on an instance";

    #[rstest]
    #[case::performance("Performance")]
    #[case::entry("PerformanceEntry")]
    #[case::measure("PerformanceMeasure")]
    fn illegal_constructor_throws(#[case] class: &str) {
        assert_eq!(
            thrown_at(&format!("new {class}()")),
            "TypeError: illegal constructor"
        );
    }

    #[rstest]
    #[case::entry("PerformanceEntry", "0")]
    #[case::mark("PerformanceMark", "1")]
    #[case::performance("Performance", "0")]
    #[case::measure("PerformanceMeasure", "0")]
    fn class_constructor_has_the_length(#[case] class: &str, #[case] length: &str) {
        assert_eq!(run_at(&format!("String({class}.length)")), length);
    }

    #[rstest]
    #[case::prototype(
        "Object.getPrototypeOf(PerformanceMark.prototype) === PerformanceEntry.prototype"
    )]
    #[case::constructor("Object.getPrototypeOf(PerformanceMark) === PerformanceEntry")]
    fn mark_inherits_the_entry(#[case] check: &str) {
        assert_eq!(run_at(&format!("String({check})")), "true");
    }

    #[rstest]
    #[case::mark("performance.mark.call({}, 'a')", FOREIGN)]
    #[case::name(
        "Object.getOwnPropertyDescriptor(PerformanceEntry.prototype, 'name').get.call({})",
        "TypeError: the receiver is not a PerformanceEntry. call the method on an instance"
    )]
    fn member_on_a_foreign_receiver_throws(#[case] call: &str, #[case] message: &str) {
        assert_eq!(thrown_at(call), message);
    }

    #[test]
    fn marks_at_zero_and_minus_zero_keep_creation_order() {
        let got = run_at(
            r"
            performance.mark('A', { startTime: 0 });
            performance.mark('B', { startTime: -0 });
            performance.getEntries().map((e) => e.name).join();
            ",
        );

        assert_eq!(got, "A,B");
    }

    #[test]
    fn measure_returns_the_listed_entry() {
        let got = run_at(
            r"
            const m = performance.measure('m', { start: 1 });
            String(performance.getEntries()[0] === m);
            ",
        );

        assert_eq!(got, "true");
    }

    #[test]
    fn an_infinite_start_time_sorts_last() {
        let got = run_at(
            r"
            performance.measure('m', { end: 1e308, duration: -1e308 });
            performance.mark('a', { startTime: 5 });
            performance.getEntries().map((e) => e.name).join();
            ",
        );

        assert_eq!(got, "a,m");
    }

    #[test]
    fn get_entries_sorts_by_start_time() {
        let got = run_at(
            r"
            performance.mark('b', { startTime: 20 });
            performance.mark('a', { startTime: 10 });
            performance.getEntries().map((e) => e.name).join();
            ",
        );

        assert_eq!(got, "a,b");
    }

    #[test]
    fn get_entries_keeps_creation_order_for_one_start_time() {
        let got = run_at(
            r"
            performance.mark('c', { startTime: 5 });
            performance.mark('a', { startTime: 5 });
            performance.mark('b', { startTime: 5 });
            performance.getEntries().map((e) => e.name).join();
            ",
        );

        assert_eq!(got, "c,a,b");
    }

    #[rstest]
    #[case::name("getEntriesByName('x')", "mark,measure")]
    #[case::name_and_type("getEntriesByName('x', 'measure')", "measure")]
    #[case::other_type("getEntriesByName('x', 'resource')", "")]
    fn get_entries_by_name_filters(#[case] call: &str, #[case] want: &str) {
        let got = run_at(&format!(
            r"
            performance.mark('x', {{ startTime: 1 }});
            performance.measure('x', {{ start: 2 }});
            performance.mark('y');
            performance.{call}.map((e) => e.entryType).join();
            "
        ));

        assert_eq!(got, want);
    }

    #[rstest]
    #[case::mark("mark", "x,y")]
    #[case::measure("measure", "z")]
    #[case::unknown("navigation", "")]
    fn get_entries_by_type_filters(#[case] entry_type: &str, #[case] want: &str) {
        let got = run_at(&format!(
            r"
            performance.mark('x', {{ startTime: 1 }});
            performance.mark('y', {{ startTime: 2 }});
            performance.measure('z', {{ start: 3 }});
            performance.getEntriesByType('{entry_type}').map((e) => e.name).join();
            "
        ));

        assert_eq!(got, want);
    }

    #[rstest]
    #[case::by_name(
        "getEntriesByName",
        "TypeError: too few arguments for getEntriesByName. pass at least 1"
    )]
    #[case::by_type(
        "getEntriesByType",
        "TypeError: too few arguments for getEntriesByType. pass at least 1"
    )]
    fn get_entries_by_requires_an_argument(#[case] method: &str, #[case] message: &str) {
        assert_eq!(thrown_at(&format!("performance.{method}()")), message);
    }

    /// The entries left after `statements`, as `type:name` joined by commas.
    fn left_after(statements: &str) -> String {
        run_at(&format!(
            r"
            performance.mark('x');
            performance.mark('y');
            performance.measure('x');
            performance.mark('null');
            performance.measure('w');
            {statements}
            performance.getEntries().map((e) => `${{e.entryType}}:${{e.name}}`).join();
            "
        ))
    }

    #[test]
    fn clear_marks_by_name_keeps_the_other_entries() {
        assert_eq!(
            left_after("performance.clearMarks('x');"),
            "mark:y,measure:x,mark:null,measure:w"
        );
    }

    #[test]
    fn clear_marks_removes_every_mark() {
        assert_eq!(
            left_after("performance.clearMarks();"),
            "measure:x,measure:w"
        );
    }

    #[test]
    fn clear_measures_by_name_keeps_the_marks() {
        assert_eq!(
            left_after("performance.clearMeasures('x');"),
            "mark:x,mark:y,mark:null,measure:w"
        );
    }

    #[rstest]
    #[case::undefined("performance.clearMarks(undefined);", "measure:x,measure:w")]
    #[case::null("performance.clearMarks(null);", "mark:x,mark:y,measure:x,measure:w")]
    fn clear_marks_converts_its_argument_to_a_name(#[case] call: &str, #[case] want: &str) {
        assert_eq!(left_after(call), want);
    }

    #[test]
    fn the_calls_vue_makes_leave_the_timeline_empty() {
        let got = run_at(
            r"
            performance.mark('start');
            performance.mark('end');
            performance.measure('m', 'start', 'end');
            performance.clearMeasures('m');
            performance.clearMarks('start');
            performance.clearMarks('end');
            String(performance.getEntries().length);
            ",
        );

        assert_eq!(got, "0");
    }

    #[rstest]
    #[case::mark(
        "performance.mark('a', { startTime: 3, detail: { x: 1 } })",
        r#"{"name":"a","entryType":"mark","startTime":3,"duration":0}"#
    )]
    #[case::measure(
        "performance.measure('m', { start: 1, end: 4, detail: 1 })",
        r#"{"name":"m","entryType":"measure","startTime":1,"duration":3}"#
    )]
    fn entry_to_json_omits_the_detail(#[case] make: &str, #[case] want: &str) {
        assert_eq!(run_at(&format!("JSON.stringify({make})")), want);
    }

    #[test]
    fn clearing_entries_frees_their_memory() {
        let (engine, _) = engine_on_test_clock();
        let (before, after) = around(&engine, |ctx| {
            ctx.eval::<(), _>(
                r"
                performance.mark('a', { detail: {} });
                performance.measure('m', 'a');
                performance.clearMarks('a');
                performance.clearMeasures('m');
                ",
            )
            .unwrap();
        });

        assert_eq!(before, after);
    }

    #[test]
    fn mark_stores_and_lists_an_entry_after_weakmap_and_array_methods_are_replaced() {
        let got = run_at(
            r"
            const fail = () => {
                throw new Error('replaced');
            };
            WeakMap.prototype.get = fail;
            WeakMap.prototype.set = fail;
            Array.prototype.push = fail;
            const m = performance.mark('a', { detail: 1 });
            `${m.name} ${m.detail} ${performance.getEntries().length}`;
            ",
        );

        assert_eq!(got, "a 1 1");
    }

    #[test]
    fn get_entries_returns_a_new_array_on_each_call() {
        let got = run_at(
            r"
            performance.mark('a');
            String(performance.getEntries() !== performance.getEntries());
            ",
        );

        assert_eq!(got, "true");
    }

    #[test]
    fn clear_measures_without_an_argument_removes_every_measure() {
        assert_eq!(
            left_after("performance.clearMeasures();"),
            "mark:x,mark:y,mark:null"
        );
    }

    #[test]
    fn entries_survive_an_indexed_setter_on_a_prototype() {
        let got = run_at(
            r"
            Object.defineProperty(Array.prototype, 0, { set() {}, configurable: true });
            Object.defineProperty(Object.prototype, 1, { set() {}, configurable: true });
            performance.mark('a');
            performance.mark('b');
            performance.mark('c');
            performance.clearMarks('a');
            performance.getEntries().map((e) => e.name).join();
            ",
        );

        assert_eq!(got, "b,c");
    }
}
