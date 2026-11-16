// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The buffers and views of a clone, and the transfer list: the checks of the list, the copies of
//! the buffers it names and the move at the end of the call.

use rquickjs::function::This;
use rquickjs::{ArrayBuffer, Ctx, Object, Result as JsResult, Value};

use super::globals::{Cause, refuse};
use super::intrinsics::Toolkit;
use super::walk::Walk;
use crate::buffer_source::{is_detached, is_resizable, read, store};
use crate::quickjs::{
    copy_range, is_array_buffer, is_immutable_array_buffer, typed_array_name, typed_array_range,
    write_typed_array,
};

/// Checks the transfer list: every value is an `ArrayBuffer`, and no buffer comes twice.
///
/// # Errors
///
/// Returns a `DataCloneError` for a value that is no `ArrayBuffer` and for a repeated buffer.
pub(super) fn check_transfer_list<'js>(
    ctx: &Ctx<'js>,
    items: Vec<Value<'js>>,
) -> JsResult<Vec<Value<'js>>> {
    let mut checked: Vec<Value<'js>> = Vec::new();
    for item in items {
        if !is_array_buffer(&item) {
            return Err(refuse(ctx, Cause::TransferItem));
        }
        if checked.contains(&item) {
            return Err(refuse(ctx, Cause::TransferTwice));
        }
        checked.push(item);
    }
    Ok(checked)
}

/// Moves the listed buffers: each one is detached after the clone is rewritten from its bytes.
///
/// # Errors
///
/// Returns a `DataCloneError` for a buffer that is detached by now and a `TypeError` for an
/// immutable one. The buffers before it stay detached.
pub(super) fn finish_transfer(walk: &Walk<'_>) -> JsResult<()> {
    let ctx = &walk.ctx;
    for listed in &walk.transfers {
        let Some(mut source) = ArrayBuffer::from_value(listed.clone()) else {
            drop(ctx.catch());
            return Err(refuse(ctx, Cause::TransferDetached));
        };
        if is_immutable_array_buffer(listed) {
            return Err(refuse(ctx, Cause::TransferImmutable));
        }
        if let Some(clone) = walk.recorded(listed)? {
            rewrite(walk, listed, &clone)?;
        }
        source.detach();
    }
    Ok(())
}

/// Writes the bytes the source has now over the clone, resizing a resizable clone first.
fn rewrite<'js>(walk: &Walk<'js>, source: &Value<'js>, clone: &Value<'js>) -> JsResult<()> {
    let tk = &walk.tk;
    let bytes = copy_range(source, 0, usize::MAX)?;
    if is_resizable(&store(&walk.ctx)?, source)? {
        tk.call::<_, Value>("resize", (This(clone.clone()), bytes.len()))?;
    }
    let view = tk.construct_member("typed", "Uint8Array", (clone.clone(),))?;
    write_typed_array(view.as_value(), &bytes)
}

impl<'js> Walk<'js> {
    /// The copy of a buffer that the transfer list names: the empty buffer for a detached one.
    pub(super) fn copy_listed(&self, buffer: &Value<'js>) -> JsResult<Value<'js>> {
        if is_detached(&store(&self.ctx)?, buffer)? {
            return Ok(self.tk.construct("ArrayBuffer", (0,))?.into_value());
        }
        copy_buffer(&self.ctx, &self.tk, buffer)
    }

    /// The clone of a buffer that the transfer list does not name.
    pub(super) fn clone_buffer(&self, buffer: &Value<'js>) -> JsResult<Value<'js>> {
        if is_detached(&store(&self.ctx)?, buffer)? {
            return Err(refuse(&self.ctx, Cause::Detached));
        }
        copy_buffer(&self.ctx, &self.tk, buffer)
    }

    /// The clone of a typed array or a `DataView`, over the clone of its buffer.
    pub(super) fn clone_view(&mut self, view: &Value<'js>) -> JsResult<Value<'js>> {
        let ctx = self.ctx.clone();
        let store = store(&ctx)?;
        let name = typed_array_name(view);
        let data_view = name.is_none();
        let buffer: Value = read(
            &store,
            if data_view {
                "viewBuffer"
            } else {
                "typedBuffer"
            },
            view,
        )?;
        if is_detached(&store, &buffer)? {
            return Err(refuse(&ctx, Cause::Detached));
        }
        let out_of_bounds = || {
            drop(ctx.catch());
            refuse(&ctx, Cause::ViewOutOfBounds)
        };
        let (offset, length): (f64, f64) = if data_view {
            let length = read::<f64>(&store, "viewLength", view).map_err(|_| out_of_bounds())?;
            (read(&store, "viewOffset", view)?, length)
        } else {
            typed_array_range(view).map_err(|_| out_of_bounds())?;
            (
                self.tk.call("typedOffset", (This(view.clone()),))?,
                self.tk.call("typedLength", (This(view.clone()),))?,
            )
        };
        let clone = self.visit(buffer.clone())?;
        if self.transfers.contains(&buffer) {
            rewrite(self, &buffer, &clone)?;
        }
        let view = match name {
            Some(name) => self
                .tk
                .construct_member("typed", name, (clone, offset, length))?,
            None => self.tk.construct("DataView", (clone, offset, length))?,
        };
        Ok(view.into_value())
    }
}

/// A new buffer with the bytes of `buffer`. A resizable buffer gives a resizable one with the same
/// `maxByteLength`.
fn copy_buffer<'js>(
    ctx: &Ctx<'js>,
    tk: &Toolkit<'js>,
    buffer: &Value<'js>,
) -> JsResult<Value<'js>> {
    let bytes = copy_range(buffer, 0, usize::MAX)?;
    if !is_resizable(&store(ctx)?, buffer)? {
        return Ok(ArrayBuffer::new_copy(ctx.clone(), &bytes)?.into_value());
    }
    let max: f64 = tk.call("maxByteLength", (This(buffer.clone()),))?;
    let options = Object::new(ctx.clone())?;
    options.set("maxByteLength", max)?;
    let clone = tk.construct("ArrayBuffer", (bytes.len(), options))?;
    let view = tk.construct_member("typed", "Uint8Array", (clone.clone(),))?;
    write_typed_array(view.as_value(), &bytes)?;
    Ok(clone.into_value())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::tests::{run, thrown};

    const DETACHED: &str = "DataCloneError: the buffer is detached. clone a buffer that holds data";
    const OUT_OF_BOUNDS: &str =
        "DataCloneError: the view is out of bounds. clone a view inside its buffer";
    const SHARED: &str =
        "DataCloneError: a shared buffer cannot be cloned. copy its bytes into an ArrayBuffer";
    const ITEM: &str = concat!(
        "DataCloneError: the transfer list holds a value that cannot be transferred. ",
        "list ArrayBuffer objects only"
    );
    const TWICE: &str =
        "DataCloneError: the transfer list holds one buffer twice. list each buffer once";
    const DETACHED_ITEM: &str =
        "DataCloneError: a buffer in the transfer list is detached. list buffers that hold data";

    #[test]
    fn an_array_buffer_clone_has_the_bytes_and_owns_them() {
        assert_eq!(
            run(r"
                const source = new Uint8Array([1, 2, 3]).buffer;
                const c = structuredClone(source);
                new Uint8Array(c)[0] = 9;
                [new Uint8Array(source).join(), new Uint8Array(c).join()].join('|');
                "),
            "1,2,3|9,2,3"
        );
    }

    #[test]
    fn an_empty_array_buffer_clones() {
        assert_eq!(
            run("String(structuredClone(new ArrayBuffer(0)).byteLength)"),
            "0"
        );
    }

    #[test]
    fn a_resizable_buffer_clones_resizable_with_the_same_maximum() {
        assert_eq!(
            run(r"
                const c = structuredClone(new ArrayBuffer(4, { maxByteLength: 8 }));
                [c.resizable, c.maxByteLength, c.byteLength].join();
                "),
            "true,8,4"
        );
    }

    #[test]
    fn a_detached_buffer_is_refused() {
        assert_eq!(
            thrown(
                r"
                const b = new ArrayBuffer(4);
                b.transfer();
                structuredClone(b)
                "
            ),
            DETACHED
        );
    }

    #[rstest]
    #[case::uint8_clamped("Uint8ClampedArray")]
    #[case::int8("Int8Array")]
    #[case::uint8("Uint8Array")]
    #[case::int16("Int16Array")]
    #[case::uint16("Uint16Array")]
    #[case::int32("Int32Array")]
    #[case::uint32("Uint32Array")]
    #[case::big_int64("BigInt64Array")]
    #[case::big_uint64("BigUint64Array")]
    #[case::float16("Float16Array")]
    #[case::float32("Float32Array")]
    #[case::float64("Float64Array")]
    fn a_typed_array_keeps_its_constructor_length_offset_and_values(#[case] class: &str) {
        assert_eq!(
            run(&format!(
                r"
                const buffer = new ArrayBuffer(64);
                new Uint8Array(buffer).set([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]);
                const view = new {class}(buffer, 8, 2);
                const c = structuredClone(view);
                const same = Array.from(c).join() === Array.from(view).join();
                [c.constructor === {class}, c.length, c.byteOffset, same, c.buffer !== buffer].join();
                "
            )),
            "true,2,8,true,true"
        );
    }

    #[test]
    fn two_views_of_one_buffer_share_one_cloned_buffer() {
        assert_eq!(
            run(r"
                const b = new ArrayBuffer(8);
                const r = structuredClone([new Uint8Array(b), new Uint16Array(b)]);
                String(r[0].buffer === r[1].buffer && r[0].buffer !== b);
                "),
            "true"
        );
    }

    #[test]
    fn a_data_view_keeps_its_offset_and_length() {
        assert_eq!(
            run(r"
                const c = structuredClone(new DataView(new ArrayBuffer(16), 4, 6));
                [c.byteOffset, c.byteLength].join();
                "),
            "4,6"
        );
    }

    #[test]
    fn a_length_tracking_typed_array_becomes_fixed_over_a_resizable_clone() {
        assert_eq!(
            run(r"
                const c = structuredClone(new Uint8Array(new ArrayBuffer(4, { maxByteLength: 8 })));
                c.buffer.resize(8);
                [c.buffer.resizable, c.length].join();
                "),
            "true,4"
        );
    }

    #[test]
    fn a_length_tracking_data_view_becomes_fixed_over_a_resizable_clone() {
        assert_eq!(
            run(r"
                const c = structuredClone(new DataView(new ArrayBuffer(4, { maxByteLength: 8 })));
                c.buffer.resize(8);
                [c.buffer.resizable, c.byteLength].join();
                "),
            "true,4"
        );
    }

    #[rstest]
    #[case::typed_array("new Uint8Array(b, 0, 2)", "c.length")]
    #[case::data_view("new DataView(b, 0, 2)", "c.byteLength")]
    fn a_fixed_view_over_a_resizable_buffer_keeps_its_length(
        #[case] view: &str,
        #[case] length: &str,
    ) {
        assert_eq!(
            run(&format!(
                r"
                const b = new ArrayBuffer(4, {{ maxByteLength: 8 }});
                const c = structuredClone({view});
                String({length});
                "
            )),
            "2"
        );
    }

    #[rstest]
    #[case::typed_array("new Uint8Array(b, 0, 2)")]
    #[case::data_view("new DataView(b, 0, 2)")]
    fn the_buffer_of_a_cloned_fixed_view_keeps_the_maximum_length(#[case] view: &str) {
        assert_eq!(
            run(&format!(
                r"
                const b = new ArrayBuffer(4, {{ maxByteLength: 8 }});
                const c = structuredClone({view});
                String(c.buffer.maxByteLength);
                "
            )),
            "8"
        );
    }

    #[rstest]
    #[case::typed_array("new Uint8Array(b, 4, 2)")]
    #[case::data_view("new DataView(b, 4, 2)")]
    fn a_view_out_of_bounds_is_refused(#[case] view: &str) {
        assert_eq!(
            thrown(&format!(
                r"
                const b = new ArrayBuffer(8, {{ maxByteLength: 8 }});
                const v = {view};
                b.resize(2);
                structuredClone(v);
                "
            )),
            OUT_OF_BOUNDS
        );
    }

    #[test]
    fn a_view_over_a_detached_buffer_is_refused_as_detached() {
        assert_eq!(
            thrown(
                r"
                const b = new ArrayBuffer(4);
                const u = new Uint8Array(b);
                b.transfer();
                structuredClone(u);
                "
            ),
            DETACHED
        );
    }

    #[test]
    fn a_shared_array_buffer_is_refused() {
        assert_eq!(thrown("structuredClone(new SharedArrayBuffer(4))"), SHARED);
    }

    #[test]
    fn a_view_over_a_shared_array_buffer_is_refused() {
        assert_eq!(
            thrown("structuredClone(new Uint8Array(new SharedArrayBuffer(4)))"),
            SHARED
        );
    }

    #[test]
    fn an_immutable_buffer_clones_as_a_mutable_buffer() {
        assert_eq!(
            run(r"
                const b = new ArrayBuffer(2);
                new Uint8Array(b).set([1, 2]);
                const source = b.transferToImmutable();
                const c = new Uint8Array(structuredClone(source));
                c[0] = 9;
                [c.join(), new Uint8Array(source).join()].join('|');
                "),
            "9,2|1,2"
        );
    }

    #[test]
    fn a_listed_buffer_is_detached_by_the_call() {
        assert_eq!(
            run(r"
                const b = new ArrayBuffer(3);
                structuredClone(b, { transfer: [b] });
                String(b.detached);
                "),
            "true"
        );
    }

    #[test]
    fn the_clone_of_a_listed_buffer_holds_its_bytes() {
        assert_eq!(
            run(r"
                const b = new Uint8Array([1, 2, 3]).buffer;
                const c = structuredClone(b, { transfer: [b] });
                new Uint8Array(c).join();
                "),
            "1,2,3"
        );
    }

    #[test]
    fn a_view_of_a_listed_buffer_is_cloned_over_the_clone_of_the_buffer() {
        assert_eq!(
            run(r"
                const b = new Uint8Array([1, 2, 3]).buffer;
                const v = new Uint8Array(b);
                const c = structuredClone(v, { transfer: [b] });
                [c.join(), c.buffer !== b].join('|');
                "),
            "1,2,3|true"
        );
    }

    #[test]
    fn a_view_of_a_listed_buffer_reads_length_zero_after_the_call() {
        assert_eq!(
            run(r"
                const b = new Uint8Array([1, 2, 3]).buffer;
                const v = new Uint8Array(b);
                structuredClone(v, { transfer: [b] });
                String(v.length);
                "),
            "0"
        );
    }

    #[test]
    fn a_listed_buffer_the_value_does_not_reach_is_detached() {
        assert_eq!(
            run(r"
                const b = new ArrayBuffer(4);
                structuredClone({}, { transfer: [b] });
                String(b.detached);
                "),
            "true"
        );
    }

    #[test]
    fn a_buffer_listed_twice_is_refused() {
        assert_eq!(
            run(r"
                const b = new ArrayBuffer(4);
                let text;
                try {
                    structuredClone({}, { transfer: [b, b] });
                } catch (e) {
                    text = `${e.name}: ${e.message}`;
                }
                `${text}|${b.detached}`;
                "),
            format!("{TWICE}|false")
        );
    }

    #[test]
    fn a_typed_array_in_the_list_is_refused() {
        assert_eq!(
            thrown("structuredClone({}, { transfer: [new Uint8Array(1)] })"),
            ITEM
        );
    }

    #[test]
    fn an_entry_after_a_valid_buffer_is_refused() {
        assert_eq!(
            run(r"
                const b = new ArrayBuffer(4);
                let text;
                try {
                    structuredClone({}, { transfer: [b, {}] });
                } catch (e) {
                    text = `${e.name}: ${e.message}`;
                }
                `${text}|${b.detached}`;
                "),
            format!("{ITEM}|false")
        );
    }

    #[test]
    fn a_buffer_reached_twice_and_listed_gives_one_clone() {
        assert_eq!(
            run(r"
                const b = new ArrayBuffer(4);
                const r = structuredClone({ p: b, q: b }, { transfer: [b] });
                String(r.p === r.q && r.p !== b);
                "),
            "true"
        );
    }

    #[test]
    fn a_listed_resizable_buffer_clones_resizable() {
        assert_eq!(
            run(r"
                const b = new ArrayBuffer(4, { maxByteLength: 8 });
                const c = structuredClone(b, { transfer: [b] });
                [c.resizable, c.maxByteLength].join();
                "),
            "true,8"
        );
    }

    #[test]
    fn a_buffer_detached_before_the_call_gives_the_detached_item_error() {
        assert_eq!(
            thrown(
                "const b = new ArrayBuffer(4); b.transfer(); structuredClone(1, { transfer: [b] })"
            ),
            DETACHED_ITEM
        );
    }

    #[test]
    fn a_getter_after_a_listed_buffer_that_writes_a_byte_shows_in_the_clone() {
        assert_eq!(
            run(r"
                let runs = 0;
                const b = new ArrayBuffer(4);
                const value = {
                    b,
                    get g() {
                        runs += 1;
                        new Uint8Array(b)[0] = 9;
                        return 1;
                    },
                };
                const r = structuredClone(value, { transfer: [b] });
                [new Uint8Array(r.b)[0], b.detached, runs].join();
                "),
            "9,true,1"
        );
    }

    #[test]
    fn a_getter_before_a_listed_buffer_that_writes_a_byte_shows_in_the_clone() {
        assert_eq!(
            run(r"
                const b = new ArrayBuffer(4);
                const value = {
                    get g() {
                        new Uint8Array(b)[0] = 9;
                        return 1;
                    },
                    b,
                };
                const r = structuredClone(value, { transfer: [b] });
                String(new Uint8Array(r.b)[0]);
                "),
            "9"
        );
    }

    #[test]
    fn a_getter_that_shrinks_a_listed_resizable_buffer_shrinks_the_clone() {
        assert_eq!(
            run(r"
                const b = new ArrayBuffer(8, { maxByteLength: 8 });
                const value = {
                    b,
                    get g() {
                        b.resize(4);
                        return 1;
                    },
                };
                const r = structuredClone(value, { transfer: [b] });
                [r.b.byteLength, r.b.maxByteLength].join();
                "),
            "4,8"
        );
    }

    #[test]
    fn a_view_made_after_a_listed_buffer_grew_fits_the_clone() {
        assert_eq!(
            run(r"
                const rab = new ArrayBuffer(8, { maxByteLength: 32 });
                const x = {
                    a: rab,
                    get g() {
                        rab.resize(16);
                        x.v = new Uint8Array(rab, 0, 16);
                        return 1;
                    },
                    v: null,
                };
                const c = structuredClone(x, { transfer: [rab] });
                [c.a.byteLength, c.v.byteLength, c.v.buffer === c.a, c.a.resizable, rab.detached].join();
                "),
            "16,16,true,true,true"
        );
    }

    #[test]
    fn a_view_over_a_buffer_that_grew_after_it_was_cloned_throws_a_range_error() {
        assert_eq!(
            run(r"
                const rab = new ArrayBuffer(8, { maxByteLength: 32 });
                const x = {
                    a: rab,
                    get g() {
                        rab.resize(16);
                        x.v = new Uint8Array(rab, 0, 16);
                        return 1;
                    },
                    v: null,
                };
                try {
                    structuredClone(x);
                } catch (e) {
                    e.name;
                }
                "),
            "RangeError"
        );
    }

    #[test]
    fn an_own_property_of_a_typed_array_is_not_copied() {
        assert_eq!(
            run(r"
                const u = new Uint8Array(1);
                u.foo = 1;
                String('foo' in structuredClone(u));
                "),
            "false"
        );
    }

    #[rstest]
    #[case::array_buffer("ArrayBuffer", "new ArrayBuffer(2, { maxByteLength: 4 })")]
    #[case::data_view("DataView", "new DataView(new ArrayBuffer(2))")]
    #[case::uint8_array("Uint8Array", "new Uint8Array(2)")]
    fn a_replaced_global_changes_no_buffer_or_view_clone(
        #[case] global: &str,
        #[case] source: &str,
    ) {
        assert_eq!(
            run(&format!(
                r"
                const source = {source};
                globalThis.{global} = function () {{
                    throw new Error('used');
                }};
                typeof structuredClone(source);
                "
            )),
            "object"
        );
    }

    #[test]
    fn a_replaced_max_byte_length_getter_changes_no_resizable_clone() {
        assert_eq!(
            run(r"
                const original = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, 'maxByteLength').get;
                const source = new ArrayBuffer(4, { maxByteLength: 8 });
                Object.defineProperty(ArrayBuffer.prototype, 'maxByteLength', {
                    get() {
                        return 1;
                    },
                });
                String(original.call(structuredClone(source)));
                "),
            "8"
        );
    }

    #[test]
    fn replaced_length_and_offset_getters_change_no_typed_array_clone() {
        assert_eq!(
            run(r"
                const proto = Object.getPrototypeOf(Uint8Array.prototype);
                const length = Object.getOwnPropertyDescriptor(proto, 'length').get;
                const offset = Object.getOwnPropertyDescriptor(proto, 'byteOffset').get;
                const source = new Uint8Array(new ArrayBuffer(16), 4, 2);
                Object.defineProperty(proto, 'length', {
                    get() {
                        return 99;
                    },
                });
                Object.defineProperty(proto, 'byteOffset', {
                    get() {
                        return 99;
                    },
                });
                const c = structuredClone(source);
                [length.call(c), offset.call(c)].join();
                "),
            "2,4"
        );
    }
}
