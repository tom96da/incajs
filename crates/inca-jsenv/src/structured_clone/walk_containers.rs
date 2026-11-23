// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The containers of the walk: `Array`, ordinary objects, `Map` and `Set`.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::function::This;
use rquickjs::object::{Filter, Property};
use rquickjs::{Array, Atom, Function, Object, Result as JsResult, Value};

use super::walk::{Frame, Walk, has_own};
use crate::structured_clone::globals::{Cause, refuse};
use crate::structured_clone::intrinsics::Kind;

impl<'js> Walk<'js> {
    /// Builds the empty clone of the container `source`, records it and pushes its frame.
    pub(super) fn open(&mut self, kind: Kind, source: &Object<'js>) -> JsResult<Value<'js>> {
        let ctx = self.ctx.clone();
        let target: Object<'js> = match kind {
            Kind::Array => {
                let array = Array::new(ctx.clone())?.into_object();
                array.set("length", source.get::<_, f64>("length")?)?;
                array
            }
            Kind::Ordinary => {
                if self.tk.is_platform(source)? {
                    return Err(refuse(&ctx, Cause::Object));
                }
                Object::new(ctx.clone())?
            }
            Kind::Map => self.tk.construct("Map", ())?,
            _ => self.tk.construct("Set", ())?,
        };
        let clone = target.clone().into_value();
        self.remember(source.as_value(), &clone)?;
        let frame = match kind {
            Kind::Map => Frame::Entries {
                target,
                items: self.entries_of(source)?,
                next: 0,
                key: None,
            },
            Kind::Set => Frame::Values {
                target,
                items: self.values_of(source)?,
                next: 0,
            },
            _ => Frame::Props {
                keys: source
                    .own_keys::<Atom>(Filter::new().string().enum_only())
                    .collect::<JsResult<Vec<_>>>()?,
                source: source.clone(),
                target,
                next: 0,
            },
        };
        self.frames.push(frame);
        Ok(clone)
    }

    /// The entries of the `Map` `source`, collected with the captured `forEach`.
    fn entries_of(&self, source: &Object<'js>) -> JsResult<Vec<(Value<'js>, Value<'js>)>> {
        let items = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&items);
        let collect = Function::new(
            self.ctx.clone(),
            move |value: Value<'js>, key: Value<'js>| {
                sink.borrow_mut().push((key, value));
            },
        )?;
        self.tk
            .call::<_, Value>("mapForEach", (This(source.clone()), collect))?;
        let taken = std::mem::take(&mut *items.borrow_mut());
        Ok(taken)
    }

    /// The values of the `Set` `source`, collected with the captured `forEach`.
    fn values_of(&self, source: &Object<'js>) -> JsResult<Vec<Value<'js>>> {
        let items = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&items);
        let collect = Function::new(self.ctx.clone(), move |value: Value<'js>| {
            sink.borrow_mut().push(value);
        })?;
        self.tk
            .call::<_, Value>("setForEach", (This(source.clone()), collect))?;
        let taken = std::mem::take(&mut *items.borrow_mut());
        Ok(taken)
    }

    /// Copies the next property of an `Array` or an ordinary object.
    pub(super) fn step_props(&mut self) -> JsResult<()> {
        let Some(Frame::Props {
            source,
            target,
            keys,
            next,
        }) = self.frames.last_mut()
        else {
            return Ok(());
        };
        let Some(key) = keys.get(*next).cloned() else {
            self.frames.pop();
            return Ok(());
        };
        *next += 1;
        let (source, target) = (source.clone(), target.clone());
        if !has_own(&source, &key)? {
            return Ok(());
        }
        let value: Value = source.get(key.clone())?;
        let clone = self.visit(value)?;
        target.prop(
            key,
            Property::from(clone).writable().enumerable().configurable(),
        )
    }

    /// Copies the next entry of a `Map`: its key, then its value.
    pub(super) fn step_entries(&mut self) -> JsResult<()> {
        let Some(Frame::Entries {
            target,
            items,
            next,
            key,
        }) = self.frames.last_mut()
        else {
            return Ok(());
        };
        let Some((source_key, source_value)) = items.get(*next).cloned() else {
            self.frames.pop();
            return Ok(());
        };
        let target = target.clone();
        let Some(clone_key) = key.take() else {
            let place = self.frames.len() - 1;
            let clone = self.visit(source_key)?;
            if let Frame::Entries { key, .. } = &mut self.frames[place] {
                *key = Some(clone);
            }
            return Ok(());
        };
        *next += 1;
        let clone_value = self.visit(source_value)?;
        self.tk.call::<_, Value>(
            "mapSet",
            (This(target.into_value()), clone_key, clone_value),
        )?;
        Ok(())
    }

    /// Copies the next value of a `Set`.
    pub(super) fn step_values(&mut self) -> JsResult<()> {
        let Some(Frame::Values {
            target,
            items,
            next,
        }) = self.frames.last_mut()
        else {
            return Ok(());
        };
        let Some(source) = items.get(*next).cloned() else {
            self.frames.pop();
            return Ok(());
        };
        *next += 1;
        let target = target.clone();
        let clone = self.visit(source)?;
        self.tk
            .call::<_, Value>("setAdd", (This(target.into_value()), clone))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::tests::{run, thrown};

    #[test]
    fn a_map_keeps_its_entries_in_order() {
        assert_eq!(
            run("JSON.stringify([...structuredClone(new Map([[1, 'a'], [2, 'b']]))])"),
            r#"[[1,"a"],[2,"b"]]"#
        );
    }

    #[test]
    fn a_map_clones_a_key_that_is_also_a_value_once() {
        assert_eq!(
            run(r"
                const k = {};
                const [[ck, cv]] = structuredClone(new Map([[k, k]]));
                String(ck === cv && ck !== k);
                "),
            "true"
        );
    }

    #[test]
    fn a_map_that_holds_itself_gives_a_map_that_holds_its_clone() {
        assert_eq!(
            run(r"
                const m = new Map();
                m.set(1, m);
                const c = structuredClone(m);
                String(c.get(1) === c && c !== m);
                "),
            "true"
        );
    }

    #[test]
    fn a_set_keeps_its_values_in_order() {
        assert_eq!(
            run("[...structuredClone(new Set([3, 1, 2]))].join()"),
            "3,1,2"
        );
    }

    #[test]
    fn a_set_that_holds_itself_gives_a_set_that_holds_its_clone() {
        assert_eq!(
            run(r"
                const s = new Set();
                s.add(s);
                const c = structuredClone(s);
                String(c.has(c) && !c.has(s));
                "),
            "true"
        );
    }

    #[test]
    fn a_set_and_a_map_in_one_value_share_the_clone_of_an_object() {
        assert_eq!(
            run(r"
                const o = {};
                const r = structuredClone({ m: new Map([[1, o]]), s: new Set([o]) });
                String([...r.s][0] === r.m.get(1));
                "),
            "true"
        );
    }

    #[test]
    fn an_array_keeps_its_holes() {
        assert_eq!(
            run(r"
                const c = structuredClone([1, , 3]);
                String(!(1 in c) && c.length === 3);
                "),
            "true"
        );
    }

    #[test]
    fn an_array_keeps_a_sparse_length() {
        assert_eq!(
            run(r"
                const a = [];
                a.length = 4294967295;
                String(structuredClone(a).length);
                "),
            "4294967295"
        );
    }

    #[test]
    fn an_array_keeps_its_own_enumerable_named_properties() {
        assert_eq!(
            run(r"
                const a = [1];
                a.name = 'x';
                structuredClone(a).name;
                "),
            "x"
        );
    }

    #[test]
    fn an_array_subclass_gives_an_array() {
        assert_eq!(
            run(r"
                class A extends Array {}
                String(Object.getPrototypeOf(structuredClone(A.from([1]))) === Array.prototype);
                "),
            "true"
        );
    }

    #[test]
    fn an_object_keeps_the_key_order_with_integer_keys_first() {
        assert_eq!(
            run("Object.keys(structuredClone({ b: 1, 2: 1, a: 1, 1: 1 })).join()"),
            "1,2,b,a"
        );
    }

    #[test]
    fn an_object_drops_a_symbol_key() {
        assert_eq!(
            run("String(Object.getOwnPropertySymbols(structuredClone({ [Symbol()]: 1 })).length)"),
            "0"
        );
    }

    #[test]
    fn an_object_drops_a_non_enumerable_key() {
        assert_eq!(
            run(r"
                const o = {};
                Object.defineProperty(o, 'hidden', { value: 1 });
                String('hidden' in structuredClone(o));
                "),
            "false"
        );
    }

    #[test]
    fn an_object_drops_an_inherited_key() {
        assert_eq!(
            run("String('inherited' in structuredClone(Object.create({ inherited: 1 })))"),
            "false"
        );
    }

    #[test]
    fn an_accessor_becomes_a_data_property_and_its_getter_runs_once() {
        assert_eq!(
            run(r"
                let reads = 0;
                const c = structuredClone({
                    get x() {
                        reads += 1;
                        return 5;
                    },
                });
                const d = Object.getOwnPropertyDescriptor(c, 'x');
                [d.value, d.get === undefined, reads].join();
                "),
            "5,true,1"
        );
    }

    #[rstest]
    #[case::class_instance("new (class {})()")]
    #[case::null_prototype("Object.create(null)")]
    fn an_object_gives_object_prototype(#[case] source: &str) {
        assert_eq!(
            run(&format!(
                "String(Object.getPrototypeOf(structuredClone({source})) === Object.prototype)"
            )),
            "true"
        );
    }

    #[test]
    fn a_proto_key_from_json_stays_an_own_key() {
        assert_eq!(
            run(r#"
                const c = structuredClone(JSON.parse('{"__proto__": {"a": 1}}'));
                [Object.getPrototypeOf(c) === Object.prototype, Object.keys(c).join()].join();
                "#),
            "true,__proto__"
        );
    }

    #[test]
    fn a_copied_property_is_writable_enumerable_and_configurable() {
        assert_eq!(
            run(r"
                const d = Object.getOwnPropertyDescriptor(structuredClone({ a: 1 }), 'a');
                [d.writable, d.enumerable, d.configurable].join();
                "),
            "true,true,true"
        );
    }

    #[rstest]
    #[case::math("Math")]
    #[case::json("JSON")]
    #[case::reflect("Reflect")]
    fn a_namespace_object_clones_as_a_plain_object(#[case] name: &str) {
        assert_eq!(
            run(&format!(
                "String(Object.getPrototypeOf(structuredClone({name})) === Object.prototype)"
            )),
            "true"
        );
    }

    #[test]
    fn object_prototype_clones_to_a_new_object() {
        assert_eq!(
            run("String(structuredClone(Object.prototype) !== Object.prototype)"),
            "true"
        );
    }

    #[test]
    fn array_prototype_clones_to_a_new_array() {
        assert_eq!(
            run("String(Array.isArray(structuredClone(Array.prototype)))"),
            "true"
        );
    }

    #[test]
    fn a_frozen_source_gives_a_clone_that_is_not_frozen() {
        assert_eq!(
            run("String(Object.isFrozen(structuredClone(Object.freeze({ a: 1 }))))"),
            "false"
        );
    }

    #[test]
    fn a_getter_that_deletes_a_later_key_removes_it_from_the_copy() {
        assert_eq!(
            run(r"
                const o = {
                    get a() {
                        delete this.b;
                        return 1;
                    },
                    b: 2,
                };
                Object.keys(structuredClone(o)).join();
                "),
            "a"
        );
    }

    #[test]
    fn a_getter_that_throws_surfaces_its_value() {
        assert_eq!(
            thrown(
                r"
                structuredClone({
                    get a() {
                        throw new RangeError('boom');
                    },
                })
                "
            ),
            "RangeError: boom"
        );
    }

    #[test]
    fn a_getter_that_adds_a_map_entry_before_the_map_is_reached_is_seen() {
        assert_eq!(
            run(r"
                const m = new Map([[1, 1]]);
                const o = {
                    get a() {
                        m.set(2, 2);
                        return 1;
                    },
                    m,
                };
                String(structuredClone(o).m.size);
                "),
            "2"
        );
    }

    #[test]
    fn a_getter_that_adds_a_set_value_before_the_set_is_reached_is_seen() {
        assert_eq!(
            run(r"
                const s = new Set([1]);
                const o = {
                    get a() {
                        s.add(2);
                        return 1;
                    },
                    s,
                };
                String(structuredClone(o).s.size);
                "),
            "2"
        );
    }

    #[test]
    fn an_entry_added_to_a_map_already_taken_is_not_in_the_clone() {
        assert_eq!(
            run(r"
                const m = new Map([
                    [
                        1,
                        {
                            get x() {
                                m.set(2, 2);
                                return 1;
                            },
                        },
                    ],
                ]);
                String(structuredClone(m).size);
                "),
            "1"
        );
    }

    #[test]
    fn getters_run_depth_first() {
        assert_eq!(
            run(r"
                const log = [];
                const value = {
                    get a() {
                        log.push('a');
                        return {
                            get x() {
                                log.push('x');
                                return 1;
                            },
                        };
                    },
                    get b() {
                        log.push('b');
                        return 1;
                    },
                };
                structuredClone(value);
                log.join();
                "),
            "a,x,b"
        );
    }

    #[test]
    fn the_key_of_a_map_entry_is_visited_before_its_value() {
        assert_eq!(
            run(r"
                const log = [];
                const key = {
                    get p() {
                        log.push('key');
                        return 1;
                    },
                };
                const value = {
                    get q() {
                        log.push('value');
                        return 1;
                    },
                };
                structuredClone(new Map([[key, value]]));
                log.join();
                "),
            "key,value"
        );
    }

    #[test]
    fn an_array_nested_a_hundred_thousand_levels_clones_without_a_crash() {
        assert_eq!(
            run(r"
                let a = [];
                for (let i = 0; i < 100000; i++) {
                    a = [a];
                }
                Array.isArray(structuredClone(a)) ? 'cloned' : 'no';
                "),
            "cloned"
        );
    }

    #[test]
    fn an_own_property_of_a_map_is_not_copied() {
        assert_eq!(
            run(r"
                const m = new Map();
                m.extra = 1;
                String('extra' in structuredClone(m));
                "),
            "false"
        );
    }

    #[test]
    fn a_replaced_global_map_changes_no_map_clone() {
        assert_eq!(
            run(r"
                const source = new Map([[1, 2]]);
                globalThis.Map = function () {
                    throw new Error('used');
                };
                String(structuredClone(source).get(1));
                "),
            "2"
        );
    }

    #[test]
    fn a_replaced_global_set_changes_no_set_clone() {
        assert_eq!(
            run(r"
                const source = new Set([1]);
                globalThis.Set = function () {
                    throw new Error('used');
                };
                String(structuredClone(source).has(1));
                "),
            "true"
        );
    }

    #[test]
    fn a_replaced_map_set_is_not_called_by_a_map_clone() {
        assert_eq!(
            run(r"
                const source = new Map([[1, 2]]);
                let calls = 0;
                Map.prototype.set = function () {
                    calls += 1;
                    return this;
                };
                const c = structuredClone(source);
                `${calls}:${c.size}`;
                "),
            "0:1"
        );
    }

    #[test]
    fn a_replaced_set_add_is_not_called_by_a_set_clone() {
        assert_eq!(
            run(r"
                const source = new Set([1]);
                let calls = 0;
                Set.prototype.add = function () {
                    calls += 1;
                    return this;
                };
                const c = structuredClone(source);
                `${calls}:${c.size}`;
                "),
            "0:1"
        );
    }

    #[test]
    fn a_replaced_map_for_each_changes_no_map_clone() {
        assert_eq!(
            run(r"
                const source = new Map([[1, 2]]);
                Map.prototype.forEach = function () {};
                String(structuredClone(source).size);
                "),
            "1"
        );
    }

    #[test]
    fn a_replaced_set_for_each_changes_no_set_clone() {
        assert_eq!(
            run(r"
                const source = new Set([1]);
                Set.prototype.forEach = function () {};
                String(structuredClone(source).size);
                "),
            "1"
        );
    }

    #[test]
    fn a_replaced_map_get_keeps_the_sharing_of_a_clone() {
        assert_eq!(
            run(r"
                const shared = {};
                const source = { p: shared, q: shared };
                let calls = 0;
                Map.prototype.get = function () {
                    calls += 1;
                };
                const c = structuredClone(source);
                `${c.p === c.q}:${calls}`;
                "),
            "true:0"
        );
    }

    #[rstest]
    #[case::array_index(
        "Object.defineProperty(Array.prototype, '0', { set() { calls += 1; }, configurable: true });",
        "structuredClone([7])[0]"
    )]
    #[case::object_key(
        "Object.defineProperty(Object.prototype, 'k', { set() { calls += 1; }, configurable: true });",
        "structuredClone({ k: 7 }).k"
    )]
    fn an_inherited_setter_is_not_called(#[case] define: &str, #[case] read: &str) {
        assert_eq!(
            run(&format!(
                r"
                let calls = 0;
                {define};
                const value = {read};
                `${{value}}:${{calls}}`;
                "
            )),
            "7:0"
        );
    }
}
