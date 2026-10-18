// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use crate::tests::{ScratchDir, with_realm};
use rstest::rstest;

fn sp(source: u32, line: u32, column: u32) -> SourcePos {
    SourcePos {
        source,
        line,
        column,
    }
}

fn decode(mappings: &str) -> Vec<Vec<Segment>> {
    decode_mappings(mappings).unwrap()
}

#[test]
fn single_digit_deltas_apply_their_sign() {
    assert_eq!(
        decode("CAAA"),
        [[Segment {
            column: 1,
            source: Some(sp(0, 0, 0)),
        }]]
    );
    assert_eq!(decode("AAIA;AAFA")[0][0].source, Some(sp(0, 4, 0)));
    assert_eq!(decode("AAIA;AAFA")[1][0].source, Some(sp(0, 2, 0)));
}

#[test]
fn multi_digit_vlq_joins_continuation_digits() {
    assert_eq!(decode("gBAAA")[0][0].column, 16);
}

#[test]
fn empty_lines_between_semicolons_have_no_segments() {
    let lines = decode("AAAA;;AAAA");
    assert_eq!(lines.len(), 3);
    assert!(lines[1].is_empty());
    assert_eq!(lines[2].len(), 1);
}

#[test]
fn one_field_segment_has_no_source() {
    assert_eq!(
        decode("C"),
        [[Segment {
            column: 1,
            source: None,
        }]]
    );
}

#[test]
fn four_and_five_field_segments_carry_a_source() {
    // The fifth field, a name index, is read and ignored.
    assert_eq!(
        decode("AAAA,CAAAC"),
        [[
            Segment {
                column: 0,
                source: Some(sp(0, 0, 0)),
            },
            Segment {
                column: 1,
                source: Some(sp(0, 0, 0)),
            },
        ]]
    );
}

#[test]
fn source_deltas_carry_across_lines() {
    let lines = decode("CAIE;AAAA");
    assert_eq!(lines[0][0].source, Some(sp(0, 4, 2)));
    assert_eq!(lines[1][0].source, Some(sp(0, 4, 2)));
}

#[test]
fn the_generated_column_resets_on_each_line() {
    let lines = decode("CAIE;AAAA");
    assert_eq!(lines[0][0].column, 1);
    assert_eq!(lines[1][0].column, 0);
}

#[test]
fn invalid_base64_character_is_none() {
    assert_eq!(decode_mappings("A%AA"), None);
}

#[test]
fn truncated_continuation_is_none() {
    assert_eq!(decode_mappings("AAAg"), None);
}

#[test]
fn vlq_beyond_twelve_digits_is_none() {
    let mappings = format!("{}A", "g".repeat(12));
    assert_eq!(decode_mappings(&mappings), None);
}

#[test]
fn column_beyond_u32_is_none() {
    // Digits 0-5 are continuation digits of zero, digit 6 is 8; the value is 2^33.
    assert_eq!(decode_mappings("ggggggIAAA"), None);
}

#[test]
fn segment_with_two_or_three_fields_is_none() {
    assert_eq!(decode_mappings("AA"), None);
    assert_eq!(decode_mappings("AAA"), None);
}

#[test]
fn segment_with_six_fields_is_none() {
    assert_eq!(decode_mappings("AAAAAA"), None);
}

#[test]
fn empty_segment_is_none() {
    assert_eq!(decode_mappings("AAAA,,AAAA"), None);
}

#[test]
fn negative_absolute_value_is_none() {
    assert_eq!(decode_mappings("DAAA"), None);
    assert_eq!(decode_mappings("ADAA"), None);
}

#[test]
fn lookup_before_first_segment_is_none() {
    let lines = vec![vec![Segment {
        column: 3,
        source: Some(sp(0, 0, 0)),
    }]];
    assert_eq!(lookup(&lines, 0, 2), None);
}

#[test]
fn lookup_on_segment_column_returns_its_source() {
    let lines = sample_lines();
    assert_eq!(lookup(&lines, 0, 5), Some(sp(1, 3, 4)));
}

#[test]
fn lookup_between_segments_picks_the_earlier() {
    let lines = sample_lines();
    assert_eq!(lookup(&lines, 0, 3), Some(sp(0, 1, 2)));
}

#[test]
fn lookup_past_last_segment_picks_the_last() {
    let lines = sample_lines();
    assert_eq!(lookup(&lines, 0, 100), Some(sp(1, 3, 4)));
}

#[test]
fn lookup_sourceless_segment_is_none() {
    let lines = vec![vec![
        Segment {
            column: 0,
            source: Some(sp(0, 0, 0)),
        },
        Segment {
            column: 5,
            source: None,
        },
    ]];
    assert_eq!(lookup(&lines, 0, 6), None);
}

#[test]
fn lookup_line_out_of_range_is_none() {
    let lines = sample_lines();
    assert_eq!(lookup(&lines, 1, 0), None);
}

/// One line with segments at columns 0 and 5, both with sources.
fn sample_lines() -> Vec<Vec<Segment>> {
    vec![vec![
        Segment {
            column: 0,
            source: Some(sp(0, 1, 2)),
        },
        Segment {
            column: 5,
            source: Some(sp(1, 3, 4)),
        },
    ]]
}

fn b64(text: &str) -> String {
    STANDARD.encode(text)
}

fn map_json(version: u32, mappings: &str, sources: &str, root: Option<&str>) -> String {
    let root = root.map_or(String::new(), |r| format!(r#","sourceRoot":"{r}""#));
    format!(r#"{{"version":{version},"mappings":"{mappings}","sources":{sources}{root}}}"#)
}

fn inline(json: &str) -> String {
    format!(
        "//# sourceMappingURL=data:application/json;base64,{}",
        b64(json)
    )
}

/// Registers `files` (name, generated source) and remaps `stack`.
fn remap(files: &[(&str, &str)], stack: &str) -> String {
    with_realm(|ctx| {
        for (name, source) in files {
            register(&ctx, name, source);
        }
        remap_stack(&ctx, stack)
    })
}

fn one(source: &str, stack: &str) -> String {
    remap(&[("a.js", source)], stack)
}

#[test]
fn inline_map_rewrites_frame_to_original_position() {
    let json = map_json(3, ";IAAO", r#"["orig.ts"]"#, None);
    let src = format!("header\nabcdefghij\n{}", inline(&json));
    assert_eq!(
        one(&src, "Error: x\n    at f (a.js:2:5)\n"),
        "Error: x\n    at f (orig.ts:1:8)\n"
    );
}

#[test]
fn function_name_containing_a_parenthesis_is_rewritten() {
    let json = map_json(3, ";AAAA", r#"["orig.ts"]"#, None);
    let src = format!("h\nabc\n{}", inline(&json));
    assert_eq!(
        one(&src, "    at f (g) (a.js:2:1)"),
        "    at f (g) (orig.ts:1:1)"
    );
}

#[test]
fn unnamed_frame_is_rewritten() {
    let json = map_json(3, ";AAAA", r#"["orig.ts"]"#, None);
    let src = format!("h\nabc\n{}", inline(&json));
    assert_eq!(one(&src, "    at a.js:2:1"), "    at orig.ts:1:1");
}

#[test]
fn utf8_byte_column_converts_to_utf16_units_for_japanese_text() {
    let json = map_json(3, ";IAAO,IAAC", r#"["orig.ts"]"#, None);
    let src = format!("h\nab日本x\n{}", inline(&json));
    assert_eq!(one(&src, "    at f (a.js:2:9)"), "    at f (orig.ts:1:8)");
}

#[test]
fn utf8_byte_column_converts_to_utf16_units_for_astral_text() {
    let json = map_json(3, ";GAAO,EAAC", r#"["orig.ts"]"#, None);
    let src = format!("h\na\u{1F600}x\n{}", inline(&json));
    assert_eq!(one(&src, "    at f (a.js:2:6)"), "    at f (orig.ts:1:8)");
}

#[test]
fn column_past_line_end_clamps() {
    let json = map_json(3, ";AAAA,GAAG", r#"["orig.ts"]"#, None);
    let src = format!("h\nabc\n{}", inline(&json));
    assert_eq!(one(&src, "    at f (a.js:2:99)"), "    at f (orig.ts:1:4)");
}

#[test]
fn column_inside_a_multibyte_char_walks_back_to_its_start() {
    let json = map_json(3, ";AAAA", r#"["orig.ts"]"#, None);
    let src = format!("h\n日x\n{}", inline(&json));
    assert_eq!(one(&src, "    at f (a.js:2:2)"), "    at f (orig.ts:1:1)");
}

#[rstest]
#[case::trailing_slash("root/")]
#[case::no_slash("root")]
fn source_root_joins_with_one_slash(#[case] root: &str) {
    let json = map_json(3, ";AAAA", r#"["a.ts"]"#, Some(root));
    let src = format!("h\nabc\n{}", inline(&json));
    assert_eq!(one(&src, "    at f (a.js:2:1)"), "    at f (root/a.ts:1:1)");
}

#[test]
fn inline_map_keeps_a_relative_source_as_written() {
    let json = map_json(3, ";AAAA", r#"["../src/a.ts"]"#, None);
    let src = format!("h\nabc\n{}", inline(&json));
    assert_eq!(
        one(&src, "    at f (a.js:2:1)"),
        "    at f (../src/a.ts:1:1)"
    );
}

#[test]
fn absolute_and_url_sources_stay_as_they_are() {
    let dir = ScratchDir::new("abs");
    let json = map_json(3, ";AAAA;AACA", r#"["/abs/a.ts","https://h/b.ts"]"#, None);
    std::fs::write(dir.join("b.js.map"), json).unwrap();
    let file = format!("{}/b.js", dir.display());
    let src = "h\nabc\n//# sourceMappingURL=b.js.map";
    let out = remap(&[(&file, src)], &format!("    at f ({file}:2:1)"));
    assert_eq!(out, "    at f (/abs/a.ts:1:1)");
}

#[test]
fn unreadable_sidecar_leaves_frame_unchanged() {
    let frame = "    at f (/nonexistent-inca/x.js:2:1)";
    let src = "h\nabc\n//# sourceMappingURL=x.js.map";
    assert_eq!(remap(&[("/nonexistent-inca/x.js", src)], frame), frame);
}

#[test]
fn invalid_maps_leave_frame_unchanged() {
    let frame = "    at f (a.js:2:1)";
    let cases = [
        "h\nabc".to_owned(),
        format!("h\nabc\n{}", inline("{not json")),
        format!(
            "h\nabc\n{}",
            inline(&map_json(2, ";AAAA", r#"["a.ts"]"#, None))
        ),
        format!(
            "h\nabc\n{}",
            inline(&map_json(3, ";!!", r#"["a.ts"]"#, None))
        ),
        format!(
            "h\nabc\n{}",
            inline(&map_json(3, ";AA", r#"["a.ts"]"#, None))
        ),
        "h\nabc\n//# sourceMappingURL=data:application/json;base64,@@@".to_owned(),
        "h\nabc\n//# sourceMappingURL=data:application/json,{}".to_owned(),
    ];
    for src in cases {
        assert_eq!(one(&src, frame), frame, "{src}");
    }
}

#[test]
fn source_index_out_of_range_leaves_frame_unchanged() {
    let json = map_json(3, ";ACAA", r#"["a.ts"]"#, None);
    let src = format!("h\nabc\n{}", inline(&json));
    assert_eq!(one(&src, "    at f (a.js:2:1)"), "    at f (a.js:2:1)");
}

#[test]
fn last_marker_line_wins() {
    let good = map_json(3, ";AAAA", r#"["good.ts"]"#, None);
    let bad = map_json(3, ";AAAA", r#"["bad.ts"]"#, None);
    let src = format!("h\nabc\n{}\n{}", inline(&bad), inline(&good));
    assert_eq!(one(&src, "    at f (a.js:2:1)"), "    at f (good.ts:1:1)");
}

#[test]
fn non_frame_lines_pass_through() {
    let json = map_json(3, ";AAAA", r#"["o.ts"]"#, None);
    let src = format!("h\nabc\n{}", inline(&json));
    let stack =
        "Error: boom (a.js:2:1)\n    at g (native)\n    at h (missing)\n\n    at q (a.js:x:1)";
    assert_eq!(one(&src, stack), stack);
}

#[test]
fn unregistered_file_passes_through() {
    let stack = "    at f (other.js:2:1)";
    assert_eq!(one("h", stack), stack);
}

#[test]
fn unregistered_runtime_passes_stack_through() {
    let out = with_realm(|ctx| remap_stack(&ctx, "    at f (a.js:2:1)\n"));
    assert_eq!(out, "    at f (a.js:2:1)\n");
}

#[test]
fn reregistering_replaces_the_map() {
    let first = format!(
        "h\nabc\n{}",
        inline(&map_json(3, ";AAAA", r#"["one.ts"]"#, None))
    );
    let second = format!(
        "h\nabc\n{}",
        inline(&map_json(3, ";AAAA", r#"["two.ts"]"#, None))
    );
    with_realm(|ctx| {
        register(&ctx, "a.js", &first);
        assert_eq!(
            remap_stack(&ctx, "    at f (a.js:2:1)"),
            "    at f (one.ts:1:1)"
        );
        register(&ctx, "a.js", &second);
        assert_eq!(
            remap_stack(&ctx, "    at f (a.js:2:1)"),
            "    at f (two.ts:1:1)"
        );
    });
}

#[test]
fn windows_path_and_url_files_parse_from_the_right() {
    let json = map_json(3, ";AAAA", r#"["o.ts"]"#, None);
    let src = format!("h\nabc\n{}", inline(&json));
    let out = remap(
        &[("C:\\x\\bundle.js", &src), ("file:///a/b.js", &src)],
        "    at f (C:\\x\\bundle.js:2:1)\n    at file:///a/b.js:2:1",
    );
    assert_eq!(out, "    at f (o.ts:1:1)\n    at o.ts:1:1");
}

#[test]
fn trailing_newline_and_crlf_are_preserved() {
    let json = map_json(3, ";AAAA", r#"["o.ts"]"#, None);
    let src = format!("h\nabc\n{}", inline(&json));
    assert_eq!(
        one(&src, "E\r\n    at f (a.js:2:1)\r\n"),
        "E\r\n    at f (o.ts:1:1)\r\n"
    );
}

#[test]
fn generated_line_beyond_the_map_leaves_frame_unchanged() {
    let json = map_json(3, ";AAAA", r#"["o.ts"]"#, None);
    let src = format!("h\nabc\n{}", inline(&json));
    assert_eq!(one(&src, "    at f (a.js:3:1)"), "    at f (a.js:3:1)");
}

#[test]
fn lf_crlf_and_lone_cr_split_generated_lines() {
    // Lines: h, '' with a U+2028 inside, 日本x, abc (after a lone CR).
    let json = map_json(3, ";;AAAA,EAAC;AAAA", r#"["o.ts"]"#, None);
    let src = format!("h\n'\u{2028}'\n日本x\rabc\n{}", inline(&json));
    // Byte column 7 follows 日本, which is 2 UTF-16 units.
    assert_eq!(one(&src, "    at f (a.js:3:7)"), "    at f (o.ts:1:2)");
    assert_eq!(one(&src, "    at f (a.js:4:1)"), "    at f (o.ts:1:2)");
}

#[test]
fn line_separator_inside_a_string_keeps_the_column() {
    let json = map_json(3, ";GAAA", r#"["o.ts"]"#, None);
    let src = format!("h\n'\u{2028}'x\n{}", inline(&json));
    assert_eq!(one(&src, "    at f (a.js:2:6)"), "    at f (o.ts:1:1)");
}

#[test]
fn source_position_at_u32_max_maps() {
    // Source line delta 4294967295 is the VLQ `+/////H`.
    let json = map_json(3, ";AA+/////HA", r#"["o.ts"]"#, None);
    let src = format!("h\nabc\n{}", inline(&json));
    assert_eq!(
        one(&src, "    at f (a.js:2:1)"),
        "    at f (o.ts:4294967296:1)"
    );
}

#[test]
fn marker_less_source_registers_nothing() {
    assert_eq!(one("h\nabc", "    at f (a.js:2:1)"), "    at f (a.js:2:1)");
}

#[test]
fn reregistering_a_marker_less_source_clears_the_map() {
    let mapped = format!(
        "h\nabc\n{}",
        inline(&map_json(3, ";AAAA", r#"["one.ts"]"#, None))
    );
    with_realm(|ctx| {
        register(&ctx, "a.js", &mapped);
        register(&ctx, "a.js", "h\nabc");
        assert_eq!(
            remap_stack(&ctx, "    at f (a.js:2:1)"),
            "    at f (a.js:2:1)"
        );
    });
}

/// Registers a sidecar module in `dir`, then runs `after` before remapping.
fn sidecar_remap(
    name: &str,
    json: &str,
    source: &str,
    frame_file: &str,
    after: impl Fn(&Path),
) -> (String, String) {
    let dir = ScratchDir::new(name);
    std::fs::create_dir_all(dir.join("dist")).unwrap();
    std::fs::write(dir.join("dist/bundle.js.map"), json).unwrap();
    let file = format!("{}/dist/{frame_file}", dir.display());
    let out = with_realm(|ctx| {
        register(&ctx, &file, source);
        after(&dir);
        remap_stack(&ctx, &format!("    at f ({file}:2:1)"))
    });
    let root = dir.display().to_string();
    (out, root)
}

#[test]
fn sidecar_map_is_captured_at_registration() {
    let first = map_json(3, ";AAAA", r#"["one.ts"]"#, None);
    let second = map_json(3, ";AAAA", r#"["two.ts"]"#, None);
    let (out, root) = sidecar_remap(
        "drift",
        &first,
        "h\nabc\n//# sourceMappingURL=bundle.js.map",
        "bundle.js",
        |dir| std::fs::write(dir.join("dist/bundle.js.map"), &second).unwrap(),
    );
    assert_eq!(out, format!("    at f ({root}/dist/one.ts:1:1)"));
}

#[test]
fn marker_text_inside_a_line_registers_nothing() {
    let json = map_json(3, ";AAAA", r#"["o.ts"]"#, None);
    let src = format!("h\nvar s = `x` + {}\nabc", inline(&json));
    assert_eq!(one(&src, "    at f (a.js:2:1)"), "    at f (a.js:2:1)");
}

#[test]
fn dot_dot_source_folds_against_the_sidecar_directory() {
    let json = map_json(3, ";AAAA", r#"["../app/./main.ts"]"#, None);
    let (out, root) = sidecar_remap(
        "fold",
        &json,
        "h\nabc\n//# sourceMappingURL=bundle.js.map",
        "bundle.js",
        |_| {},
    );
    assert_eq!(out, format!("    at f ({root}/app/main.ts:1:1)"));
}

#[test]
fn file_path_containing_a_parenthesis_maps() {
    let json = map_json(3, ";AAAA", r#"["o.ts"]"#, None);
    let src = format!("h\nabc\n{}", inline(&json));
    let out = remap(
        &[("/My App (1)/bundle.js", &src)],
        "    at f (/My App (1)/bundle.js:2:1)",
    );
    assert_eq!(out, "    at f (o.ts:1:1)");
}

#[test]
fn null_source_name_leaves_frame_unchanged() {
    let json = map_json(3, ";AAAA;ACAA", r#"[null,"a.ts"]"#, None);
    let src = format!("h\nabc\ndef\n{}", inline(&json));
    assert_eq!(
        one(&src, "    at f (a.js:2:1)\n    at g (a.js:3:1)"),
        "    at f (a.js:2:1)\n    at g (a.ts:1:1)"
    );
}
