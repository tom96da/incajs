// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use rstest::rstest;

#[rstest]
#[case::elements("new Uint8Array(3)", "Uint8Array(3) [ 0, 0, 0 ]")]
#[case::empty("new Uint8Array(0)", "Uint8Array(0) []")]
#[case::detached(
    r"
    (() => {
        const buffer = new ArrayBuffer(4);
        const array = new Uint8Array(buffer);
        buffer.transfer();
        return array;
    })()
    ",
    "Uint8Array(0) []"
)]
#[case::subclass(
    r"
    (() => {
        class U extends Uint8Array {}
        return new U(2);
    })()
    ",
    "U(2) [Uint8Array] [ 0, 0 ]"
)]
#[case::null_prototype(
    "Object.setPrototypeOf(new Uint8Array(1), null)",
    "[Uint8Array(1): null prototype] [ 0 ]"
)]
#[case::past_the_depth(
    "({ a: { b: { c: new Uint8Array(1) } } })",
    "{ a: { b: { c: [Uint8Array] } } }"
)]
#[case::subclass_past_the_depth(
    r"
    (() => {
        class U extends Uint8Array {}
        return [[[new U(1)]]];
    })()
    ",
    "[ [ [ [U [Uint8Array]] ] ] ]"
)]
#[case::null_prototype_past_the_depth(
    "[[[Object.setPrototypeOf(new Uint8Array(1), null)]]]",
    "[ [ [ [Uint8Array: null prototype] ] ] ]"
)]
#[case::prototype_only("Object.create(Uint8Array.prototype)", "Uint8Array {}")]
fn a_typed_array_prints_its_elements(#[case] expression: &str, #[case] want: &str) {
    assert_eq!(rendered(expression), want);
}

#[test]
fn a_typed_array_past_a_hundred_entries_counts_the_rest() {
    let text = rendered("new Uint8Array(101)");

    let zeros = vec!["0"; 100].join(", ");
    assert_eq!(
        text,
        format!("Uint8Array(101) [ {zeros}, ... 1 more item ]")
    );
}

#[test]
fn a_typed_array_reads_no_getter_a_script_replaced() {
    let (text, calls) = counted(
        r"
        const typed = Object.getPrototypeOf(Uint8Array.prototype);
        for (const key of ['length', Symbol.toStringTag]) {
            Object.defineProperty(typed, key, {
                get() {
                    calls++;
                    return 9;
                },
            });
        }
        new Uint8Array(2);
        ",
    );

    assert_eq!((text.as_str(), calls), ("Uint8Array(2) [ 0, 0 ]", 0));
}
