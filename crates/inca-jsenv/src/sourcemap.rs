// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Decodes source maps and rewrites the frames of a JS stack through them.

use std::{
    cell::OnceCell,
    ops::Range,
    path::{Component, Path, PathBuf},
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use rquickjs::{Ctx, Error, Result as JsResult};

use store::SourceMaps;

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

/// The next unique id of a compiled function's filename query.
static NEXT_COMPILE_ID: AtomicU64 = AtomicU64::new(0);

struct Entry {
    /// The filename the caller asked for. A compile prints it for a frame its map
    /// does not cover. `None` for a loader or engine module, whose key is its filename.
    display: Option<String>,
    /// The generated source. Empty when the entry has no map.
    source: String,
    raw: Option<RawMap>,
    parsed: OnceCell<Option<Parsed>>,
}

/// The map text captured at registration.
enum RawMap {
    /// The byte range of the marker line's `data:` URL in the entry's `source`.
    Inline(Range<usize>),
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

/// The runtime's maps, created on first use. `None` when the userdata store is borrowed, which
/// leaves the registration unrecorded.
fn maps_or_insert<'a>(ctx: &'a Ctx<'_>) -> Option<impl std::ops::Deref<Target = SourceMaps> + 'a> {
    if ctx.userdata::<SourceMaps>().is_none() {
        let _ = ctx.store_userdata(SourceMaps::default());
    }
    let maps = ctx.userdata::<SourceMaps>();
    debug_assert!(
        maps.is_some(),
        "the userdata store is borrowed, so a source map registration is lost"
    );
    maps
}

/// Drops the map of `filename`, if any.
fn forget(ctx: &Ctx<'_>, filename: &str) {
    if let Some(maps) = ctx.userdata::<SourceMaps>() {
        maps.0.borrow_mut().remove(filename);
    }
}

/// Captures the map that the marker `url` of `source` names. A sidecar map file is read here,
/// next to `filename`. `None` for an unreadable map file.
fn capture(filename: &str, source: &str, url: &str) -> Option<RawMap> {
    if url.starts_with("data:") {
        let start = url.as_ptr().addr() - source.as_ptr().addr();
        return Some(RawMap::Inline(start..start + url.len()));
    }
    let path = Path::new(filename).parent()?.join(url);
    let json = std::fs::read_to_string(&path).ok()?;
    Some(RawMap::Sidecar {
        json,
        dir: path.parent().map(Path::to_path_buf),
    })
}

/// Registers the generated `source` of a loader or engine module under `filename`. Registering the
/// same filename again replaces its source and map. A sidecar map file is read here and the map is
/// parsed on the first remap.
pub(crate) fn register(ctx: &Ctx<'_>, filename: &str, source: &str) {
    let Some(raw) = marker_url(source).and_then(|url| capture(filename, source, url)) else {
        return forget(ctx, filename);
    };
    insert(ctx, filename.to_owned(), None, source, Some(raw));
}

fn insert(ctx: &Ctx<'_>, key: String, display: Option<String>, source: &str, raw: Option<RawMap>) {
    if let Some(maps) = maps_or_insert(ctx) {
        let entry = Entry {
            display,
            source: raw.as_ref().map_or_else(String::new, |_| source.to_owned()),
            raw,
            parsed: OnceCell::new(),
        };
        maps.0.borrow_mut().insert(key, Rc::new(entry));
    }
}

/// Takes the next id from `counter`.
fn next_compile_id(counter: &AtomicU64) -> JsResult<u64> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map_err(|_| Error::Unknown)
}

/// Registers the generated `source` that [`compile_async_function`](crate::compile_async_function)
/// compiles for `filename` and returns the key to eval it as. The first compile of a `filename`
/// keeps it as the key. A later compile gets a unique `?t=N` query before any original query, as a
/// browser names an HMR update. A frame of the key prints `filename`, or the position its own map
/// gives.
///
/// # Errors
///
/// Returns an error once the process has taken `u64::MAX` ids.
pub(crate) fn register_compile(ctx: &Ctx<'_>, filename: &str, source: &str) -> JsResult<String> {
    let taken = ctx
        .userdata::<SourceMaps>()
        .is_some_and(|maps| maps.0.borrow().contains_key(filename));
    let key = if taken {
        let id = next_compile_id(&NEXT_COMPILE_ID)?;
        match filename.split_once('?') {
            Some((path, query)) => format!("{path}?t={id}&{query}"),
            None => format!("{filename}?t={id}"),
        }
    } else {
        filename.to_owned()
    };
    let raw = marker_url(source).and_then(|url| capture(filename, source, url));
    insert(ctx, key.clone(), Some(filename.to_owned()), source, raw);
    Ok(key)
}

/// Rewrites every `at NAME (FILE:LINE:COLUMN)` and `at FILE:LINE:COLUMN`
/// frame of `stack` whose FILE is registered. A frame that its map covers names the original
/// file, line and column. A frame of a compile that its map does not cover names the requested
/// file with the generated line and column. Every other line is returned unchanged.
pub(crate) fn remap_stack(ctx: &Ctx<'_>, stack: &str) -> String {
    let Some(maps) = ctx.userdata::<SourceMaps>() else {
        return stack.to_owned();
    };
    stack
        .split('\n')
        .map(|line| {
            let (text, cr) = line
                .strip_suffix('\r')
                .map_or((line, ""), |text| (text, "\r"));
            remap_frame(&maps, text).map_or_else(|| line.to_owned(), |new| new + cr)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn remap_frame(maps: &SourceMaps, line: &str) -> Option<String> {
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

    let Some((file, line, col)) = original_position(&entry, row, column) else {
        let display = entry.display.as_ref()?;
        return Some(format!("{prefix}{display}:{row}:{column}{close}"));
    };
    Some(format!("{prefix}{file}:{line}:{col}{close}"))
}

/// The original file, line and column of generated `row` and byte `column` (both 1-based) of
/// `entry`. `None` when the entry has no valid map or the map has no segment for the position.
fn original_position(entry: &Entry, row: usize, column: usize) -> Option<(&str, u64, u64)> {
    let parsed = entry.parsed.get_or_init(|| parse(entry)).as_ref()?;
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
    Some((original, u64::from(pos.line) + 1, u64::from(pos.column) + 1))
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

/// Decodes the map `entry` captured. `None` for an invalid map or no map.
fn parse(entry: &Entry) -> Option<Parsed> {
    let raw = entry.raw.as_ref()?;
    let (map, sidecar_dir): (MapJson, _) = match raw {
        RawMap::Inline(url) => {
            let (_, payload) = entry.source[url.clone()].split_once("base64,")?;
            (
                serde_json::from_slice(&STANDARD.decode(payload).ok()?).ok()?,
                None,
            )
        }
        RawMap::Sidecar { json, dir } => (serde_json::from_str(json).ok()?, dir.as_deref()),
    };
    if map.version != 3 {
        return None;
    }
    let lines = decode_mappings(&map.mappings)?;
    let root = map.source_root.as_str().unwrap_or_default();
    let sources = map
        .sources
        .into_iter()
        .map(|name| name.map_or_else(String::new, |name| resolve(root, &name, sidecar_dir)))
        .collect();
    Some(Parsed { lines, sources })
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

mod store;
#[cfg(test)]
mod tests;
