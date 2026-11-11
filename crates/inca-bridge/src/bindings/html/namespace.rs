// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The namespace of the nodes a parse gives back.

#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::tests::{engine, eval};

#[test]
fn a_parsed_input_reads_its_value_attribute_as_typed_text() {
    let (engine, _host) = engine();

    let got = eval(
        &engine,
        r"
        const parsed = n.setInnerHtml(host, '<input value=a>');
        return n.inputValue(parsed[1][0][0]);
        ",
    );

    assert_eq!(got, r#""a""#);
}

#[test]
fn a_parsed_svg_input_gets_no_typed_text() {
    let (engine, host) = engine();

    let id: u32 = engine
        .eval(
            r"
            const parsed = n.setInnerHtml(host, '<svg><input value=a></svg>');
            parsed[1][1][0];
            ",
        )
        .unwrap();

    assert!(!host.borrow().inputs.borrow().contains_key(&id));
}

#[rstest]
#[case::inner_html(
    "n.setInnerHtml(svg, '<title><b>x</b></title>')",
    r##"[["title",-1,null],["b",0,null],["#text",1,"x"]]"##
)]
#[case::adjacent_html(
    "n.insertAdjacentHtml(svg, 'beforeend', '<title><b>x</b></title>')",
    r##"[["title",-1,null],["b",0,null],["#text",1,"x"]]"##
)]
#[case::outer_html(
    "n.setOuterHtml(circle, '<title><b>x</b></title>')",
    r##"[["title",-1,null],["b",0,null],["#text",1,"x"]]"##
)]
fn the_content_of_an_svg_element_parses_as_svg(#[case] call: &str, #[case] expected: &str) {
    let (engine, _host) = engine();

    let got = eval(
        &engine,
        &format!(
            r"
            const svg = n.setInnerHtml(host, '<svg><circle></circle></svg>')[1][0][0];
            const circle = n.setInnerHtml(svg, '<circle></circle>')[1][0][0];
            return rows({call});
            "
        ),
    );

    assert_eq!(got, expected);
}
