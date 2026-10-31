// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

const FRAME: &str = "Error: x\n    at f (a.js:2:5)\n";
const MAPPED: &str = "Error: x\n    at f (origin.ts:1:8)\n";

fn origin_json() -> String {
    map_json(3, ";IAAO", r#"["origin.ts"]"#, None)
}

fn percent_encoded(text: &str) -> String {
    text.replace('"', "%22")
        .replace('{', "%7B")
        .replace('}', "%7D")
        .replace(',', "%2C")
}

fn mapped_by(url: &str) -> String {
    let src = format!("header\nabcdefghij\n//# sourceMappingURL={url}");
    one(&src, FRAME)
}

#[rstest]
#[case::percent_encoded(format!("data:application/json,{}", percent_encoded(&origin_json())))]
#[case::raw(format!("data:application/json,{}", origin_json()))]
fn a_plain_data_url_maps_a_frame(#[case] url: String) {
    assert_eq!(mapped_by(&url), MAPPED);
}

#[rstest]
#[case::charset(format!("data:application/json;charset=utf-8;base64,{}", b64(&origin_json())))]
#[case::upper_case(format!("data:application/json;BASE64,{}", b64(&origin_json())))]
fn a_base64_data_url_head_variant_maps_a_frame(#[case] url: String) {
    assert_eq!(mapped_by(&url), MAPPED);
}

#[test]
fn a_percent_escaped_base64_payload_maps_a_frame() {
    let payload = b64(&origin_json());
    assert!(payload.ends_with('='), "{payload}");
    let url = format!(
        "data:application/json;base64,{}",
        payload.replace('=', "%3D")
    );

    assert_eq!(mapped_by(&url), MAPPED);
}

#[test]
fn a_percent_encoded_sidecar_name_is_found() {
    let dir = ScratchDir::new("sidecar-escape");
    let json = map_json(3, ";AAAA;AACA", r#"["/abs/a.ts"]"#, None);
    std::fs::write(dir.join("my map.js.map"), json).unwrap();
    let file = format!("{}/b.js", dir.display());
    let src = "h\nabc\n//# sourceMappingURL=my%20map.js.map";

    let out = remap(&[(&file, src)], &format!("    at f ({file}:2:1)"));

    assert_eq!(out, "    at f (/abs/a.ts:1:1)");
}

#[test]
fn a_sidecar_name_that_decodes_to_invalid_utf8_is_found_as_written() {
    let dir = ScratchDir::new("sidecar-invalid-utf8");
    let json = map_json(3, ";AAAA;AACA", r#"["/abs/a.ts"]"#, None);
    std::fs::write(dir.join("a%FFb.js.map"), json).unwrap();
    let file = format!("{}/b.js", dir.display());
    let src = "h\nabc\n//# sourceMappingURL=a%FFb.js.map";

    let out = remap(&[(&file, src)], &format!("    at f ({file}:2:1)"));

    assert_eq!(out, "    at f (/abs/a.ts:1:1)");
}
