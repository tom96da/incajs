// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Reads the map a source names: its marker, its data URL or sidecar file, and its sources.

use std::{
    borrow::Cow,
    path::{Component, Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use percent_encoding::percent_decode_str;

use super::mappings::decode_mappings;
use super::{Entry, Parsed, RawMap};

const MARKER: &str = "//# sourceMappingURL=";

/// The URL of the last line of `source` that starts with the marker.
pub(super) fn marker_url(source: &str) -> Option<&str> {
    source
        .lines()
        .rev()
        .find_map(|line| line.strip_prefix(MARKER))
        .map(str::trim)
}

/// Captures the map that the marker `url` of `source` names. A sidecar name is percent-decoded and
/// its file is read here, next to `filename`. `None` for an unreadable map file.
pub(super) fn capture(filename: &str, source: &str, url: &str) -> Option<RawMap> {
    if url.starts_with("data:") {
        let start = url.as_ptr().addr() - source.as_ptr().addr();
        return Some(RawMap::Inline(start..start + url.len()));
    }
    let name = percent_decode_str(url)
        .decode_utf8()
        .unwrap_or(Cow::Borrowed(url));
    let path = Path::new(filename).parent()?.join(name.as_ref());
    let json = std::fs::read_to_string(&path).ok()?;
    Some(RawMap::Sidecar {
        json,
        dir: path.parent().map(Path::to_path_buf),
    })
}

/// The fields of a source map this module reads. Other fields are ignored.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MapJson {
    version: u32,
    mappings: String,
    sources: Vec<Option<String>>,
    /// A root that is not a string counts as none.
    #[serde(default)]
    source_root: serde_json::Value,
}

/// The bytes of the map that the `data:` URL `url` carries. `None` for no comma or invalid base64.
fn data_url_body(url: &str) -> Option<Vec<u8>> {
    let (head, body) = url.split_once(',')?;
    let bytes: Vec<u8> = percent_decode_str(body).collect();
    let base64 = head
        .rsplit_once(';')
        .is_some_and(|(_, last)| last.eq_ignore_ascii_case("base64"));
    if base64 {
        STANDARD.decode(bytes).ok()
    } else {
        Some(bytes)
    }
}

/// Decodes the map `entry` captured. `None` for an invalid map or no map.
pub(super) fn parse(entry: &Entry) -> Option<Parsed> {
    let raw = entry.raw.as_ref()?;
    let (map, base): (MapJson, Option<PathBuf>) = match raw {
        RawMap::Inline(url) => (
            serde_json::from_slice(&data_url_body(&entry.source[url.clone()])?).ok()?,
            inline_base(&entry.display),
        ),
        RawMap::Sidecar { json, dir } => (serde_json::from_str(json).ok()?, dir.clone()),
    };
    if map.version != 3 {
        return None;
    }
    let lines = decode_mappings(&map.mappings)?;
    let root = map.source_root.as_str().unwrap_or_default();
    let sources = map
        .sources
        .into_iter()
        .map(|name| name.map_or_else(String::new, |name| resolve(root, &name, base.as_deref())))
        .collect();
    Some(Parsed { lines, sources })
}

/// The directory an inline map's relative sources resolve against: the parent of `display` without
/// its query, when that path is absolute. `None` for a relative filename.
fn inline_base(display: &str) -> Option<PathBuf> {
    let file = Path::new(display.split_once('?').map_or(display, |(path, _)| path));
    if file.is_absolute() {
        file.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

/// Joins `name` under `root`. A relative result is folded against `base`, when there is one.
fn resolve(root: &str, name: &str, base: Option<&Path>) -> String {
    let joined = if root.is_empty() {
        name.to_owned()
    } else {
        format!("{}/{name}", root.trim_end_matches('/'))
    };
    let absolute =
        joined.starts_with('/') || joined.contains("://") || Path::new(&joined).is_absolute();
    match base {
        Some(dir) if !absolute => fold(&dir.join(joined)).to_string_lossy().into_owned(),
        _ => joined,
    }
}

/// Folds `.` and `..` components lexically.
pub(super) fn fold(path: &Path) -> PathBuf {
    let mut parts: Vec<Component> = Vec::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir if matches!(parts.last(), Some(Component::Normal(_))) => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    parts.iter().collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::rc::Rc;

    use rstest::rstest;

    use super::*;
    use crate::sourcemap::store::SourceMaps;
    use crate::sourcemap::{register, register_compile, remap_stack};
    use crate::tests::{ScratchDir, b64, inline, map_json, one, remap, with_realm};

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

    #[rstest]
    #[case::trailing_slash("root/")]
    #[case::no_slash("root")]
    fn source_root_joins_with_one_slash(#[case] root: &str) {
        let json = map_json(3, ";AAAA", r#"["a.ts"]"#, Some(root));
        let src = format!("h\nabc\n{}", inline(&json));
        assert_eq!(one(&src, "    at f (a.js:2:1)"), "    at f (root/a.ts:1:1)");
    }

    #[test]
    fn inline_map_of_a_relative_filename_keeps_its_source_as_written() {
        let json = map_json(3, ";AAAA", r#"["../src/a.ts"]"#, None);
        let src = format!("h\nabc\n{}", inline(&json));
        assert_eq!(
            remap(&[("dist/a.js", &src)], "    at f (dist/a.js:2:1)"),
            "    at f (../src/a.ts:1:1)"
        );
    }

    #[test]
    fn inline_map_resolves_a_relative_source_against_its_file() {
        let json = map_json(3, ";AAAA", r#"["../src/a.ts"]"#, None);
        let src = format!("h\nabc\n{}", inline(&json));
        assert_eq!(
            remap(&[("/app/dist/a.js", &src)], "    at f (/app/dist/a.js:2:1)"),
            "    at f (/app/src/a.ts:1:1)"
        );
    }

    #[test]
    fn a_compile_resolves_its_inline_sources_against_the_requested_file() {
        let json = map_json(3, ";AAAA", r#"["main.ts"]"#, None);
        let src = format!("h\nabc\n{}", inline(&json));
        with_realm(|ctx| {
            let first = register_compile(&ctx, "/app/src/main.js?x=/y", &src).unwrap();
            let second = register_compile(&ctx, "/app/src/main.js?x=/y", &src).unwrap();
            assert!(second.contains("?t="), "{second}");
            for key in [first, second] {
                assert_eq!(
                    remap_stack(&ctx, &format!("    at f ({key}:2:1)")),
                    "    at f (/app/src/main.ts:1:1)"
                );
            }
        });
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

    #[rstest]
    #[case::not_json(inline("{not json"))]
    #[case::version_2(inline(&map_json(2, ";AAAA", r#"["a.ts"]"#, None)))]
    #[case::bad_vlq(inline(&map_json(3, ";!!", r#"["a.ts"]"#, None)))]
    #[case::two_field_segment(inline(&map_json(3, ";AA", r#"["a.ts"]"#, None)))]
    #[case::bad_base64("//# sourceMappingURL=data:application/json;base64,@@@".to_owned())]
    fn invalid_maps_leave_frame_unchanged(#[case] marker: String) {
        let frame = "    at f (a.js:2:1)";
        let src = format!("h\nabc\n{marker}");
        assert_eq!(one(&src, frame), frame);
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
    fn marker_less_source_registers_nothing() {
        assert_eq!(one("h\nabc", "    at f (a.js:2:1)"), "    at f (a.js:2:1)");
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
    fn a_sidecar_source_root_resolves_against_the_sidecar_directory() {
        let json = map_json(3, ";AAAA", r#"["a.ts"]"#, Some("lib"));
        let (out, root) = sidecar_remap(
            "root",
            &json,
            "h\nabc\n//# sourceMappingURL=bundle.js.map",
            "bundle.js",
            |_| {},
        );
        assert_eq!(out, format!("    at f ({root}/dist/lib/a.ts:1:1)"));
    }

    #[test]
    fn an_unreadable_sidecar_stores_an_entry_without_a_map() {
        let src = "h\nabc\n//# sourceMappingURL=x.js.map";
        let entry = with_realm(|ctx| {
            register(&ctx, "/nonexistent-inca/x.js", src);
            ctx.userdata::<SourceMaps>().and_then(|maps| {
                maps.entries
                    .borrow()
                    .get("/nonexistent-inca/x.js")
                    .map(Rc::clone)
            })
        });
        assert!(entry.is_some_and(|entry| entry.raw.is_none()));
    }

    #[test]
    fn a_sidecar_for_a_filename_without_a_directory_stores_no_map() {
        let dir = ScratchDir::new("rootless");
        let json = map_json(3, ";AAAA", r#"["o.ts"]"#, None);
        std::fs::write(dir.join("b.js.map"), json).unwrap();
        let src = format!("h\nabc\n//# sourceMappingURL={}/b.js.map", dir.display());
        let entry = with_realm(|ctx| {
            register(&ctx, "/", &src);
            ctx.userdata::<SourceMaps>()
                .and_then(|maps| maps.entries.borrow().get("/").map(Rc::clone))
        });
        assert!(entry.is_some_and(|entry| entry.raw.is_none()));
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

    /// A mapped source of `one.ts` for `a.js`.
    fn one_ts_source() -> String {
        format!(
            "h\nabc\n{}",
            inline(&map_json(3, ";AAAA", r#"["one.ts"]"#, None))
        )
    }

    #[test]
    fn an_object_prototype_source_root_leaves_the_sources_alone() {
        let out = with_realm(|ctx| {
            register(&ctx, "a.js", &one_ts_source());
            ctx.eval::<(), _>("Object.prototype.sourceRoot = 'evil';")
                .unwrap();
            remap_stack(&ctx, "    at f (a.js:2:1)")
        });
        assert_eq!(out, "    at f (one.ts:1:1)");
    }

    #[test]
    fn an_object_prototype_getter_leaves_no_exception_pending() {
        let pending = with_realm(|ctx| {
            register(&ctx, "a.js", &one_ts_source());
            ctx.eval::<(), _>(
                r"
                Object.defineProperty(Object.prototype, 'sourceRoot', {
                    get() {
                        throw new Error('x');
                    },
                    configurable: true,
                });
                ",
            )
            .unwrap();
            let _ = remap_stack(&ctx, "    at f (a.js:2:1)");
            ctx.has_exception()
        });
        assert!(!pending);
    }
}
