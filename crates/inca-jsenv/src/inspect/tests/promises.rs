// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use rstest::rstest;

#[rstest]
#[case::resolved("Promise.resolve(1)", "Promise { 1 }")]
#[case::pending("new Promise(() => {})", "Promise { <pending> }")]
#[case::rejected(
    r"
    (() => {
        const p = Promise.reject(1);
        p.catch(() => {});
        return p;
    })()
    ",
    "Promise { <rejected> 1 }"
)]
#[case::nested_string("Promise.resolve('a')", "Promise { 'a' }")]
#[case::own_property("Object.assign(Promise.resolve(1), { x: 2 })", "Promise { 1, x: 2 }")]
#[case::subclass(
    r"
    (() => {
        class P extends Promise {}
        return P.resolve(1);
    })()
    ",
    "P [Promise] { 1 }"
)]
#[case::null_prototype(
    "Object.setPrototypeOf(Promise.resolve(1), null)",
    "[Promise: null prototype] { 1 }"
)]
#[case::past_the_depth("[[[Promise.resolve(1)]]]", "[ [ [ [Promise] ] ] ]")]
#[case::prototype_only("Object.create(Promise.prototype)", "Promise {}")]
fn a_promise_prints_its_state(#[case] expression: &str, #[case] want: &str) {
    assert_eq!(rendered(expression), want);
}

#[rstest]
#[case::pending("new Promise(() => {})", format!("Promise {{ {} }}", cyan("<pending>")))]
#[case::rejected(
    r"
    (() => {
        const p = Promise.reject(1);
        p.catch(() => {});
        return p;
    })()
    ",
    format!("Promise {{ {} {} }}", cyan("<rejected>"), yellow("1"))
)]
fn a_promise_state_marker_is_cyan(#[case] expression: &str, #[case] want: String) {
    assert_eq!(colored(expression), want);
}
