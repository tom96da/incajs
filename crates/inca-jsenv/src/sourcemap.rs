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
mod tests;
