// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Behavior of `gpui_cases`, with `#[test]` as the test attribute.

#![allow(clippy::unwrap_used)]

use inca_test_macros::gpui_cases;
#[gpui_cases]
#[test]
#[case::one(1, 2)]
#[case::two(2, 4)]
#[case(3, 6)]
fn each_case_binds_its_values(n: u32, doubled: u32) {
    assert_eq!(n * 2, doubled);
}

#[gpui_cases]
#[test]
#[case::first("a")]
#[case::second("b")]
#[case::third("c")]
#[case::fourth("d")]
#[case::fifth("e")]
#[case::sixth("f")]
#[case::seventh("g")]
#[case::eighth("h")]
#[case::ninth("i")]
#[case::tenth("j")]
fn ten_cases_pad_the_number(letter: &str) {
    // rstest pads the number to the width of the case count.
    let thread = std::thread::current();
    let name = thread.name().unwrap();
    assert!(
        name.contains("::case_0") || name.contains("::case_10_"),
        "{name}"
    );
    assert_eq!(letter.len(), 1);
}

#[gpui_cases]
#[test]
#[should_panic(expected = "boom")]
#[case::panics(7)]
fn other_attributes_reach_every_case(code: u8) {
    panic!("boom {code}");
}

#[gpui_cases]
#[test]
#[case::moved(vec![1, 2, 3])]
fn a_value_is_moved_into_its_parameter(v: Vec<i32>) {
    assert_eq!(v.iter().sum::<i32>(), 6);
}
