// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The walk over a value: the visit of one value and the loop over the frames of the containers
//! it opens. A visit returns the clone at once and pushes the frame that fills it, so the native
//! depth stays constant whatever the depth of the value.

use rquickjs::function::This;
use rquickjs::object::Property;
use rquickjs::{Atom, Ctx, Object, Result as JsResult, Value};

use super::buffer::finish_transfer;
use super::globals::{Cause, refuse};
use super::intrinsics::{Kind, Toolkit};
use crate::quickjs::ObjectExt;

mod containers;

/// The work that is left on a container or an error.
pub(super) enum Frame<'js> {
    /// The own enumerable string keys of an `Array` or an ordinary object.
    Props {
        source: Object<'js>,
        target: Object<'js>,
        keys: Vec<Atom<'js>>,
        next: usize,
    },
    /// The entries of a `Map`. `key` holds the clone of the entry's key once it is visited.
    Entries {
        target: Object<'js>,
        items: Vec<(Value<'js>, Value<'js>)>,
        next: usize,
        key: Option<Value<'js>>,
    },
    /// The values of a `Set`.
    Values {
        target: Object<'js>,
        items: Vec<Value<'js>>,
        next: usize,
    },
    /// The `cause` of a cloned error.
    Cause {
        target: Object<'js>,
        cause: Value<'js>,
    },
}

/// The state of one call.
pub(super) struct Walk<'js> {
    pub(super) ctx: Ctx<'js>,
    pub(super) tk: Toolkit<'js>,
    /// A `Map` from a source object to its clone.
    memory: Object<'js>,
    pub(super) frames: Vec<Frame<'js>>,
    pub(super) transfers: Vec<Value<'js>>,
}

/// The clone of `value`, with the buffers of `transfers` moved to it. `transfers` is checked.
///
/// # Errors
///
/// Returns a `DataCloneError` for a value that cannot be cloned, and the error a getter throws.
pub(crate) fn clone_value<'js>(
    ctx: &Ctx<'js>,
    value: &Value<'js>,
    transfers: &[Value<'js>],
) -> JsResult<Value<'js>> {
    if value.is_symbol() {
        return Err(refuse(ctx, Cause::Symbol));
    }
    if !value.is_object() && transfers.is_empty() {
        return Ok(value.clone());
    }
    let tk = Toolkit::load(ctx)?;
    let memory = tk.construct("Map", ())?;
    let mut walk = Walk {
        ctx: ctx.clone(),
        tk,
        memory,
        frames: Vec::new(),
        transfers: transfers.to_vec(),
    };
    let root = walk.visit(value.clone())?;
    walk.run()?;
    finish_transfer(&walk)?;
    Ok(root)
}

impl<'js> Walk<'js> {
    /// The clone of `value`. A container's clone is empty until the loop runs its frame.
    pub(super) fn visit(&mut self, value: Value<'js>) -> JsResult<Value<'js>> {
        let Some(object) = value.as_object().cloned() else {
            return if value.is_symbol() {
                Err(refuse(&self.ctx, Cause::Symbol))
            } else {
                Ok(value)
            };
        };
        let kind = self.tk.kind_of(&object);
        if let Some(known) = self.recorded(&value)? {
            return Ok(known);
        }
        if kind == Kind::Buffer && self.transfers.contains(&value) {
            let copy = self.copy_listed(&value)?;
            self.remember(&value, &copy)?;
            return Ok(copy);
        }
        match kind {
            Kind::Proxy => Err(refuse(&self.ctx, Cause::Proxy)),
            Kind::Function => Err(refuse(&self.ctx, Cause::Function)),
            Kind::Refused => Err(refuse(&self.ctx, Cause::Object)),
            Kind::Shared => Err(refuse(&self.ctx, Cause::Shared)),
            Kind::Boxed | Kind::Date | Kind::RegExp => {
                let copy = self.clone_leaf(kind, &value)?;
                self.remember(&value, &copy)?;
                Ok(copy)
            }
            Kind::DomException => {
                let copy = self.clone_dom_exception(&value)?;
                self.remember(&value, &copy)?;
                Ok(copy)
            }
            Kind::Rect => {
                let copy = self.clone_rect(&value)?;
                self.remember(&value, &copy)?;
                Ok(copy)
            }
            Kind::Error => self.clone_error(&object),
            Kind::Buffer => {
                let copy = self.clone_buffer(&value)?;
                self.remember(&value, &copy)?;
                Ok(copy)
            }
            Kind::View => {
                let copy = self.clone_view(&value)?;
                self.remember(&value, &copy)?;
                Ok(copy)
            }
            Kind::Array | Kind::Ordinary | Kind::Map | Kind::Set => self.open(kind, &object),
        }
    }

    /// Records `copy` as the clone of `source`.
    pub(super) fn remember(&self, source: &Value<'js>, copy: &Value<'js>) -> JsResult<()> {
        self.tk
            .call::<_, Value>(
                "mapSet",
                (This(self.memory.clone()), source.clone(), copy.clone()),
            )
            .map(drop)
    }

    /// The clone recorded for `source`, if any.
    pub(super) fn recorded(&self, source: &Value<'js>) -> JsResult<Option<Value<'js>>> {
        let known: Value = self
            .tk
            .call("mapGet", (This(self.memory.clone()), source.clone()))?;
        Ok((!known.is_undefined()).then_some(known))
    }

    /// Runs the frames until none is left.
    fn run(&mut self) -> JsResult<()> {
        loop {
            let step = match self.frames.last() {
                None => return Ok(()),
                Some(Frame::Props { .. }) => Self::step_props,
                Some(Frame::Entries { .. }) => Self::step_entries,
                Some(Frame::Values { .. }) => Self::step_values,
                Some(Frame::Cause { .. }) => Self::step_cause,
            };
            step(self)?;
        }
    }

    /// Visits the cause of an error and defines it on the clone as a non-enumerable property.
    fn step_cause(&mut self) -> JsResult<()> {
        let Some(Frame::Cause { target, cause }) = self.frames.pop() else {
            return Ok(());
        };
        let clone = self.visit(cause)?;
        target.prop("cause", Property::from(clone).writable().configurable())
    }
}

/// Whether `source` still has `key` as an own property.
pub(super) fn has_own<'js>(source: &Object<'js>, key: &Atom<'js>) -> JsResult<bool> {
    Ok(source
        .get_own_property_descriptor(key.to_value()?)?
        .is_some())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::Engine;
    use crate::tests::{around, run, thrown};

    #[test]
    fn a_primitive_returns_the_same_value() {
        assert_eq!(run("String(structuredClone(1.5))"), "1.5");
    }

    #[test]
    fn a_symbol_value_is_refused() {
        assert_eq!(
            thrown("structuredClone(Symbol())"),
            "DataCloneError: a symbol cannot be cloned. remove it from the value"
        );
    }

    #[test]
    fn a_symbol_inside_an_object_is_refused() {
        assert_eq!(
            thrown("structuredClone({ a: Symbol() })"),
            "DataCloneError: a symbol cannot be cloned. remove it from the value"
        );
    }

    #[test]
    fn an_object_that_holds_itself_gives_a_clone_that_holds_itself() {
        assert_eq!(
            run(r"
                const o = {};
                o.self = o;
                const c = structuredClone(o);
                String(c.self === c && c !== o);
                "),
            "true"
        );
    }

    #[test]
    fn a_reference_held_twice_is_one_clone() {
        assert_eq!(
            run(r"
                const s = {};
                const c = structuredClone({ p: s, q: s });
                String(c.p === c.q && c.p !== s);
                "),
            "true"
        );
    }

    #[test]
    fn an_error_whose_cause_is_itself_gives_a_clone_that_is_its_own_cause() {
        assert_eq!(
            run(r"
                const e = new Error('x');
                e.cause = e;
                const r = structuredClone(e);
                String(r.cause === r);
                "),
            "true"
        );
    }

    #[test]
    fn a_chain_of_a_hundred_thousand_causes_clones_without_a_crash() {
        assert_eq!(
            run(r"
                let e = new Error('0');
                for (let i = 0; i < 100000; i++) {
                    const next = new Error('n');
                    next.cause = e;
                    e = next;
                }
                structuredClone(e) instanceof Error ? 'cloned' : 'no';
                "),
            "cloned"
        );
    }

    #[test]
    fn an_error_shared_as_a_cause_and_an_element_is_one_clone() {
        assert_eq!(
            run(r"
                const e = new Error('x');
                const p = new Error('p');
                p.cause = e;
                const r = structuredClone([e, p]);
                String(r[1].cause === r[0]);
                "),
            "true"
        );
    }

    #[test]
    fn a_function_is_refused() {
        assert_eq!(
            thrown("structuredClone(function () {})"),
            "DataCloneError: a function cannot be cloned. remove it from the value"
        );
    }

    #[rstest]
    #[case::object("structuredClone(new Proxy({}, {}))")]
    #[case::revoked(
        r"
        const p = Proxy.revocable({}, {});
        p.revoke();
        structuredClone(p.proxy)
        "
    )]
    fn a_proxy_is_refused(#[case] source: &str) {
        assert_eq!(
            thrown(source),
            "DataCloneError: a proxy cannot be cloned. clone its target"
        );
    }

    #[test]
    fn an_arguments_object_is_refused() {
        assert_eq!(
            thrown(
                r"
                function f() {
                    return structuredClone(arguments);
                }
                f()
                "
            ),
            "DataCloneError: the object cannot be cloned. pass plain data"
        );
    }

    #[rstest]
    #[case::weak_map("new WeakMap()")]
    #[case::symbol_object("Object(Symbol())")]
    #[case::url("new URL('http://a/')")]
    #[case::mark("performance.mark('x')")]
    fn an_object_of_another_class_is_refused(#[case] value: &str) {
        assert_eq!(
            thrown(&format!("structuredClone({value})")),
            "DataCloneError: the object cannot be cloned. pass plain data"
        );
    }

    #[test]
    fn cloning_a_graph_ten_thousand_times_changes_no_counter() {
        let engine = Engine::new().unwrap();
        let (before, after) = around(&engine, |ctx| {
            ctx.eval::<(), _>(
                r"
                (() => {
                    const o = { a: [1, { b: 2 }], m: new Map([[1, 2]]), s: new Set([1]) };
                    o.self = o;
                    structuredClone(o);
                })();
                ",
            )
            .unwrap();
        });

        assert_eq!(before, after);
    }

    #[test]
    fn refusing_a_value_ten_thousand_times_changes_no_counter() {
        let engine = Engine::new().unwrap();
        let (before, after) = around(&engine, |ctx| {
            ctx.eval::<(), _>(
                r"
                try {
                    structuredClone({ a: [1, { f() {} }] });
                } catch (e) {}
                ",
            )
            .unwrap();
        });

        assert_eq!(before, after);
    }
}
