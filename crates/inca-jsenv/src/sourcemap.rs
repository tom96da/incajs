// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Decodes source maps and rewrites the frames of a JS stack through them.

use std::{
    cell::OnceCell,
    cell::RefCell,
    collections::HashMap,
    path::{Component, Path, PathBuf},
    rc::Rc,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use rquickjs::{Ctx, Object};

use crate::quickjs::Plain;

/// A generated position and the source position it maps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Segment {
    pub column: u32,
    pub source: Option<SourcePos>,
}

/// A position in an original source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourcePos {
    pub source: u32,
    pub line: u32,
    pub column: u32,
}

const CONTINUATION: u8 = 0x20;
const PAYLOAD_MASK: u8 = 0x1f;
const MAX_SHIFT: u32 = 60;

/// Decodes `mappings` into one segment list per generated line.
///
/// Returns `None` for an invalid base64 digit, a truncated VLQ, an empty segment, a segment of
/// 2, 3 or more than 5 fields, or a value outside 0..=`u32::MAX`.
pub(crate) fn decode_mappings(mappings: &str) -> Option<Vec<Vec<Segment>>> {
    let mut lines = Vec::new();
    let mut source = 0_i64;
    let mut source_line = 0_i64;
    let mut source_column = 0_i64;

    for line in mappings.split(';') {
        let mut segments = Vec::new();
        let mut column = 0_i64;
        if !line.is_empty() {
            for segment in line.split(',') {
                let fields = decode_fields(segment)?;
                let [column_delta, rest @ ..] = fields.as_slice() else {
                    return None;
                };
                column = column.checked_add(*column_delta)?;
                let source_pos = match rest {
                    [] => None,
                    [source_delta, line_delta, column_delta]
                    | [source_delta, line_delta, column_delta, _] => {
                        source = source.checked_add(*source_delta)?;
                        source_line = source_line.checked_add(*line_delta)?;
                        source_column = source_column.checked_add(*column_delta)?;
                        Some(SourcePos {
                            source: u32::try_from(source).ok()?,
                            line: u32::try_from(source_line).ok()?,
                            column: u32::try_from(source_column).ok()?,
                        })
                    }
                    _ => return None,
                };
                segments.push(Segment {
                    column: u32::try_from(column).ok()?,
                    source: source_pos,
                });
            }
        }
        lines.push(segments);
    }
    Some(lines)
}

/// Returns the source position of the last segment on `line` whose column is
/// at most `column`, or `None` when that segment has no source.
pub(crate) fn lookup(lines: &[Vec<Segment>], line: usize, column: u32) -> Option<SourcePos> {
    lines
        .get(line)?
        .iter()
        .rev()
        .find(|segment| segment.column <= column)?
        .source
}

/// Reads every VLQ in one segment.
fn decode_fields(segment: &str) -> Option<Vec<i64>> {
    let bytes = segment.as_bytes();
    let mut fields = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        fields.push(read_vlq(bytes, &mut pos)?);
    }
    Some(fields)
}

/// Reads one VLQ at `pos`, advancing it past the digits read.
fn read_vlq(bytes: &[u8], pos: &mut usize) -> Option<i64> {
    let mut value = 0_i64;
    let mut shift: u32 = 0;
    loop {
        let digit = base64_digit(*bytes.get(*pos)?)?;
        *pos += 1;
        if shift >= MAX_SHIFT {
            return None;
        }
        value |= i64::from(digit & PAYLOAD_MASK) << shift;
        shift += 5;
        if digit & CONTINUATION == 0 {
            break;
        }
    }
    let magnitude = value >> 1;
    Some(if value & 1 == 1 {
        -magnitude
    } else {
        magnitude
    })
}

fn base64_digit(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

const MARKER: &str = "//# sourceMappingURL=";

/// The source maps of one runtime, keyed by the filename `QuickJS` reports.
#[derive(Default)]
struct SourceMaps(RefCell<HashMap<String, Rc<Entry>>>);

struct Entry {
    source: String,
    raw: Option<RawMap>,
    parsed: OnceCell<Option<Parsed>>,
}

/// The map text captured at registration.
enum RawMap {
    /// The `data:` URL of the marker line.
    Inline(String),
    /// The sidecar file's text and the directory it was read from.
    Sidecar { json: String, dir: Option<PathBuf> },
}

/// The URL of the last line of `source` that starts with the marker.
fn marker_url(source: &str) -> Option<&str> {
    source
        .lines()
        .rev()
        .find_map(|line| line.strip_prefix(MARKER))
        .map(str::trim)
}

struct Parsed {
    lines: Vec<Vec<Segment>>,
    sources: Vec<String>,
}

/// Registers the generated `source` of `filename`. A recompile of the same
/// filename replaces its source and map. A sidecar map file is read here and the map
/// is parsed on the first remap.
pub(crate) fn register(ctx: &Ctx<'_>, filename: &str, source: &str) {
    let Some(url) = marker_url(source) else {
        if let Some(maps) = ctx.userdata::<Plain<SourceMaps>>() {
            maps.0.0.borrow_mut().remove(filename);
        }
        return;
    };
    let raw = if url.starts_with("data:") {
        Some(RawMap::Inline(url.to_owned()))
    } else {
        Path::new(filename)
            .parent()
            .map(|dir| dir.join(url))
            .and_then(|path| {
                let json = std::fs::read_to_string(&path).ok()?;
                Some(RawMap::Sidecar {
                    json,
                    dir: path.parent().map(Path::to_path_buf),
                })
            })
    };
    if ctx.userdata::<Plain<SourceMaps>>().is_none() {
        let _ = ctx.store_userdata(Plain(SourceMaps::default()));
    }
    if let Some(maps) = ctx.userdata::<Plain<SourceMaps>>() {
        let entry = Entry {
            source: source.to_owned(),
            raw,
            parsed: OnceCell::new(),
        };
        maps.0
            .0
            .borrow_mut()
            .insert(filename.to_owned(), Rc::new(entry));
    }
}

/// Rewrites every `at NAME (FILE:LINE:COLUMN)` and `at FILE:LINE:COLUMN`
/// frame of `stack` whose FILE has a registered map to the original file,
/// line and column. Every other line is returned unchanged.
pub(crate) fn remap_stack(ctx: &Ctx<'_>, stack: &str) -> String {
    let Some(maps) = ctx.userdata::<Plain<SourceMaps>>() else {
        return stack.to_owned();
    };
    stack
        .split('\n')
        .map(|line| {
            let (text, cr) = line
                .strip_suffix('\r')
                .map_or((line, ""), |text| (text, "\r"));
            remap_frame(ctx, &maps, text).map_or_else(|| line.to_owned(), |new| new + cr)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn remap_frame(ctx: &Ctx<'_>, maps: &SourceMaps, line: &str) -> Option<String> {
    if !line.trim_start().starts_with("at ") {
        return None;
    }
    let (body, close) = line
        .strip_suffix(')')
        .map_or((line, ""), |body| (body, ")"));
    let mut parts = body.rsplitn(3, ':');
    let column = number(parts.next()?)?;
    let row = number(parts.next()?)?;
    let head = parts.next()?;
    let start = head.find("at ")? + 3;
    let (prefix, entry) = head[start..]
        .match_indices(" (")
        .map(|(at, _)| start + at + 2)
        .chain([start])
        .find_map(|split| {
            let (prefix, file) = head.split_at(split);
            let entry = maps.0.borrow().get(file).map(Rc::clone)?;
            Some((prefix, entry))
        })?;
    let parsed = entry
        .parsed
        .get_or_init(|| parse(ctx, entry.raw.as_ref()?))
        .as_ref()?;
    let text = nth_line(&entry.source, row.checked_sub(1)?)?;
    let mut end = column.saturating_sub(1).min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let units = u32::try_from(text[..end].encode_utf16().count()).ok()?;
    let pos = lookup(&parsed.lines, row - 1, units)?;
    let original = parsed.sources.get(usize::try_from(pos.source).ok()?)?;
    if original.is_empty() {
        return None;
    }
    Some(format!(
        "{prefix}{original}:{}:{}{close}",
        u64::from(pos.line) + 1,
        u64::from(pos.column) + 1
    ))
}

/// The `index`th line of `source`, split at `\n`, `\r\n` and `\r`.
fn nth_line(source: &str, index: usize) -> Option<&str> {
    let mut rest = source;
    for _ in 0..index {
        let at = rest.find(['\n', '\r'])?;
        let skip = if rest[at..].starts_with("\r\n") {
            2
        } else {
            rest[at..].chars().next()?.len_utf8()
        };
        rest = &rest[at + skip..];
    }
    rest.split(['\n', '\r']).next()
}

fn number(text: &str) -> Option<usize> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Decodes the captured map text. `None` for an invalid map.
fn parse(ctx: &Ctx<'_>, raw: &RawMap) -> Option<Parsed> {
    let (json, sidecar_dir) = match raw {
        RawMap::Inline(url) => {
            let (_, payload) = url.split_once("base64,")?;
            (
                String::from_utf8(STANDARD.decode(payload).ok()?).ok()?,
                None,
            )
        }
        RawMap::Sidecar { json, dir } => (json.clone(), dir.as_deref()),
    };
    let Ok(value) = ctx.json_parse(json) else {
        let _ = ctx.catch();
        return None;
    };
    let map = value.into_object()?;
    if map.get::<_, i32>("version").ok()? != 3 {
        return None;
    }
    let lines = decode_mappings(&map.get::<_, String>("mappings").ok()?)?;
    let root = map
        .get::<_, Option<String>>("sourceRoot")
        .ok()
        .flatten()
        .unwrap_or_default();
    let sources = sources_of(&map)?
        .into_iter()
        .map(|name| name.map_or_else(String::new, |name| resolve(&root, &name, sidecar_dir)))
        .collect();
    Some(Parsed { lines, sources })
}

fn sources_of(map: &Object<'_>) -> Option<Vec<Option<String>>> {
    map.get::<_, Vec<Option<String>>>("sources").ok()
}

fn resolve(root: &str, name: &str, sidecar_dir: Option<&Path>) -> String {
    let joined = if root.is_empty() {
        name.to_owned()
    } else {
        format!("{}/{name}", root.trim_end_matches('/'))
    };
    let absolute =
        joined.starts_with('/') || joined.contains("://") || Path::new(&joined).is_absolute();
    match sidecar_dir {
        Some(dir) if !absolute => fold(&dir.join(joined)).to_string_lossy().into_owned(),
        _ => joined,
    }
}

/// Folds `.` and `..` components lexically.
fn fold(path: &Path) -> PathBuf {
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

/// A function body for tests in other modules. Its first line is `code`, and
/// an inline map sends that line to `src/app.ts` line 12, column 5.
#[cfg(test)]
pub(crate) fn mapped_body(code: &str) -> String {
    let json = r#"{"version":3,"mappings":";AAWI","sources":["src/app.ts"]}"#;
    format!(
        "{code}\n//# sourceMappingURL=data:application/json;base64,{}",
        STANDARD.encode(json)
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

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
    fn source_deltas_carry_across_lines_and_generated_column_resets() {
        let lines = decode("CAIE;AAAA");
        assert_eq!(lines[0][0].column, 1);
        assert_eq!(lines[0][0].source, Some(sp(0, 4, 2)));
        assert_eq!(lines[1][0].column, 0);
        assert_eq!(lines[1][0].source, Some(sp(0, 4, 2)));
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

    use rquickjs::{Context, Runtime};

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
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            for (name, source) in files {
                register(&ctx, name, source);
            }
            remap_stack(&ctx, stack)
        })
    }

    fn one(source: &str, stack: &str) -> String {
        remap(&[("a.js", source)], stack)
    }

    /// A directory under the OS temp root, unique per test invocation, torn
    /// down on drop.
    struct ScratchDir(std::path::PathBuf);

    impl std::ops::Deref for ScratchDir {
        type Target = Path;

        fn deref(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_dir(name: &str) -> ScratchDir {
        let dir = std::env::temp_dir().join(format!(
            "inca-sm-{}-{name}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        ScratchDir(dir)
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

    #[test]
    fn source_root_joins_with_one_slash() {
        let mut mismatches = Vec::new();
        for root in ["root/", "root"] {
            let json = map_json(3, ";AAAA", r#"["a.ts"]"#, Some(root));
            let src = format!("h\nabc\n{}", inline(&json));
            let out = one(&src, "    at f (a.js:2:1)");
            if out != "    at f (root/a.ts:1:1)" {
                mismatches.push((root, out));
            }
        }
        assert_eq!(mismatches, []);
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
        let dir = temp_dir("abs");
        let json = map_json(3, ";AAAA;AACA", r#"["/abs/a.ts","https://h/b.ts"]"#, None);
        std::fs::write(dir.join("b.js.map"), json).unwrap();
        let file = format!("{}/b.js", dir.display());
        let src = "h\nabc\n//# sourceMappingURL=b.js.map";
        let out = remap(&[(&file, src)], &format!("    at f ({file}:2:1)"));
        assert_eq!(out, "    at f (/abs/a.ts:1:1)");
    }

    #[test]
    fn sidecar_map_resolves_against_the_module_directory() {
        let dir = temp_dir("sidecar");
        let json = map_json(3, ";AAAA", r#"["src/a.ts"]"#, None);
        std::fs::write(dir.join("bundle.js.map"), json).unwrap();
        let file = format!("{}/bundle.js", dir.display());
        let src = "h\nabc\n//# sourceMappingURL=bundle.js.map\n";
        let out = remap(&[(&file, src)], &format!("    at f ({file}:2:1)"));
        let expected = format!("    at f ({}/src/a.ts:1:1)", dir.display());
        assert_eq!(out, expected);
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
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        let out = context.with(|ctx| remap_stack(&ctx, "    at f (a.js:2:1)\n"));
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
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
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
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
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
        let dir = temp_dir(name);
        std::fs::create_dir_all(dir.join("dist")).unwrap();
        std::fs::write(dir.join("dist/bundle.js.map"), json).unwrap();
        let file = format!("{}/dist/{frame_file}", dir.display());
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        let out = context.with(|ctx| {
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
}
