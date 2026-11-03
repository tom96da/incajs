// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Decodes source maps and rewrites the frames of a JS stack through them.

use std::{
    borrow::Cow,
    cell::OnceCell,
    iter,
    ops::Range,
    path::{Path, PathBuf},
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use rquickjs::{Ctx, Error, Result as JsResult};

use load::{capture, fold, marker_url, parse};
use mappings::{Segment, lookup};
use store::SourceMaps;

/// How a stack names a file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum StackPaths {
    /// As the map or the engine gives it.
    #[default]
    Absolute,
    /// From the root when the file lies under it, in full otherwise. `lexical` is the root as
    /// written with `.` and `..` folded; `real` its canonical path, when it exists.
    Relative {
        lexical: PathBuf,
        real: Option<PathBuf>,
    },
}

impl StackPaths {
    /// Names files from `root`.
    #[must_use]
    pub fn relative(root: &Path) -> Self {
        Self::Relative {
            lexical: fold(root),
            real: root.canonicalize().ok(),
        }
    }

    /// The name `path` takes in a stack. A path equal to the root shows as `.`. A `?query` rides
    /// on the last component and stays.
    fn show<'a>(&self, path: &'a str) -> Cow<'a, str> {
        let Self::Relative { lexical, real } = self else {
            return Cow::Borrowed(path);
        };
        let file = Path::new(path);
        if !file.is_absolute() {
            return Cow::Borrowed(path);
        }
        iter::once(lexical)
            .chain(real)
            .find_map(|root| file.strip_prefix(root).ok())
            .map_or(Cow::Borrowed(path), |rest| {
                if rest.as_os_str().is_empty() {
                    Cow::Borrowed(".")
                } else {
                    rest.to_string_lossy()
                }
            })
    }
}

/// The next unique id of a compiled function's filename query.
static NEXT_COMPILE_ID: AtomicU64 = AtomicU64::new(0);

struct Entry {
    /// The filename the caller asked for: the key of a loader or engine module, the filename of a
    /// compile without its `?t=` query. A frame its map does not cover prints it.
    display: String,
    /// The generated source. Empty when the entry has no map.
    source: String,
    raw: Option<RawMap>,
    parsed: OnceCell<Option<Parsed>>,
}

/// The map text captured at registration.
enum RawMap {
    /// The byte range of the marker line's `data:` URL in the entry's `source`. The body after
    /// the first comma is percent-decoded, then base64-decoded when the head ends with `;base64`.
    /// Its relative sources resolve against the directory of the entry's file.
    Inline(Range<usize>),
    /// The sidecar file's text and the directory it was read from.
    Sidecar { json: String, dir: Option<PathBuf> },
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

/// Sets how the runtime's stacks name a file. Call it before any registration.
pub(crate) fn set_paths(ctx: &Ctx<'_>, paths: StackPaths) -> JsResult<()> {
    ctx.store_userdata(SourceMaps {
        paths,
        ..SourceMaps::default()
    })
    .map(drop)
    .map_err(|_| Error::Unknown)
}

/// Registers the generated `source` of a loader or engine module under `filename`, with or without
/// a map. Registering the same filename again replaces its source and map. A sidecar map file is
/// read here and the map is parsed on the first remap.
pub(crate) fn register(ctx: &Ctx<'_>, filename: &str, source: &str) {
    let raw = marker_url(source).and_then(|url| capture(filename, source, url));
    insert(ctx, filename.to_owned(), filename.to_owned(), source, raw);
}

fn insert(ctx: &Ctx<'_>, key: String, display: String, source: &str, raw: Option<RawMap>) {
    if let Some(maps) = maps_or_insert(ctx) {
        let entry = Entry {
            display,
            source: raw.as_ref().map_or_else(String::new, |_| source.to_owned()),
            raw,
            parsed: OnceCell::new(),
        };
        maps.entries.borrow_mut().insert(key, Rc::new(entry));
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
        .is_some_and(|maps| maps.entries.borrow().contains_key(filename));
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
    insert(ctx, key.clone(), filename.to_owned(), source, raw);
    Ok(key)
}

/// Rewrites every `at NAME (FILE:LINE:COLUMN)` and `at FILE:LINE:COLUMN`
/// frame of `stack` whose FILE is registered. A frame that its map covers names the original
/// file, line and column. A frame that its map does not cover names the registered
/// file with the generated line and column. A frame names its file as the engine's [`StackPaths`]
/// says. Every other line is returned unchanged.
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
            let entry = maps.entries.borrow().get(file).map(Rc::clone)?;
            Some((prefix, entry))
        })?;

    let Some((file, line, col)) = original_position(&entry, row, column) else {
        let display = maps.paths.show(&entry.display);
        return Some(format!("{prefix}{display}:{row}:{column}{close}"));
    };
    let file = maps.paths.show(file);
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

/// A function body for tests in other modules. Its first line is `code`, and
/// an inline map sends that line to `src/app.ts` line 12, column 5.
#[cfg(test)]
pub(crate) fn mapped_body(code: &str) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let json = r#"{"version":3,"mappings":";AAWI","sources":["src/app.ts"]}"#;
    format!(
        "{code}\n//# sourceMappingURL=data:application/json;base64,{}",
        STANDARD.encode(json)
    )
}

mod load;
mod mappings;
mod store;

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::cell::RefCell;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    use rquickjs::{Function, Promise};
    use rstest::rstest;

    use super::*;
    use crate::Engine;
    use crate::tests::{ScratchDir, inline, map_json, one, remap, with_realm};

    fn relative(root: &str) -> StackPaths {
        StackPaths::relative(Path::new(root))
    }

    #[rstest]
    #[case::under("/app", "/app/src/a.ts", "src/a.ts")]
    #[case::outside("/app", "/other/a.ts", "/other/a.ts")]
    #[case::relative_input("app", "app/x.ts", "app/x.ts")]
    #[case::the_root("/app", "/app", ".")]
    #[case::query("/app", "/app/src/a.ts?t=1", "src/a.ts?t=1")]
    fn relative_paths_show_a_file_under_the_root_from_the_root(
        #[case] root: &str,
        #[case] path: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(relative(root).show(path), expected);
    }

    #[test]
    fn absolute_paths_show_every_file_in_full() {
        assert_eq!(StackPaths::Absolute.show("/app/src/a.ts"), "/app/src/a.ts");
    }

    /// The first frame of a function that `register_compile` registers under `file`, in an engine
    /// that names files as `paths` says. The inline map sends it to the source `main.ts`.
    fn compile_frame(paths: StackPaths, file: &str) -> String {
        let source = format!(
            "h\nabc\n{}",
            inline(&map_json(3, ";AAAA", r#"["main.ts"]"#, None))
        );
        let engine = Engine::builder().stack_paths(paths).build().unwrap();
        engine.with(|ctx| {
            let key = register_compile(&ctx, file, &source).unwrap();
            remap_stack(&ctx, &format!("    at f ({key}:2:1)"))
        })
    }

    /// The first frame of a function compiled from `file_dir/src/main.ts`, in an engine rooted at
    /// `root_dir`, both under `base`.
    fn frame_under(base: &Path, file_dir: &str, root_dir: &str) -> String {
        let file = base.join(file_dir).join("src/main.ts");
        let paths = StackPaths::relative(&base.join(root_dir));
        compile_frame(paths, &file.to_string_lossy())
    }

    #[test]
    fn a_mapped_frame_names_its_source_from_the_root() {
        let dir = ScratchDir::new("paths");
        let base = dir.canonicalize().unwrap();
        std::fs::create_dir_all(base.join("real/src")).unwrap();
        assert_eq!(
            frame_under(&base, "real", "real"),
            "    at f (src/main.ts:1:1)"
        );
    }

    #[cfg(unix)]
    #[rstest]
    #[case::through_symlink("link", "link")]
    #[case::real_id("real", "link")]
    fn a_mapped_frame_names_its_source_from_a_symlinked_root(
        #[case] file_dir: &str,
        #[case] root_dir: &str,
    ) {
        let dir = ScratchDir::new("paths");
        let base = dir.canonicalize().unwrap();
        std::fs::create_dir_all(base.join("real/src")).unwrap();
        symlink(base.join("real"), base.join("link")).unwrap();
        assert_eq!(
            frame_under(&base, file_dir, root_dir),
            "    at f (src/main.ts:1:1)"
        );
    }

    /// Registers `source` as `/app/dist/a.js` in an engine rooted at `/app` and remaps `stack`.
    fn remap_from_app(source: &str, stack: &str) -> String {
        let engine = Engine::builder()
            .stack_paths(relative("/app"))
            .build()
            .unwrap();
        engine.with(|ctx| {
            register(&ctx, "/app/dist/a.js", source);
            remap_stack(&ctx, stack)
        })
    }

    #[test]
    fn an_uncovered_frame_names_its_file_from_the_root() {
        let source = format!(
            "h\nabc\n{}",
            inline(&map_json(3, ";AAAA", r#"["../src/a.ts"]"#, None))
        );
        assert_eq!(
            remap_from_app(&source, "    at f (/app/dist/a.js:9:1)"),
            "    at f (dist/a.js:9:1)"
        );
    }

    #[test]
    fn a_module_without_a_map_names_its_file_from_the_root() {
        assert_eq!(
            remap_from_app("h\nabc", "    at f (/app/dist/a.js:2:1)"),
            "    at f (dist/a.js:2:1)"
        );
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

    #[rstest]
    #[case::crlf("h\r\nabc")]
    #[case::lone_cr("h\rabc")]
    fn a_crlf_or_a_lone_cr_ends_a_generated_line(#[case] source: &str) {
        assert_eq!(nth_line(source, 1), Some("abc"));
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

    #[test]
    fn file_path_containing_a_parenthesis_maps() {
        let json = map_json(3, ";AAAA", r#"["o.ts"]"#, None);
        let src = format!("h\nabc\n{}", inline(&json));
        let out = remap(
            &[("/My App (1)/bundle.js", &src)],
            "    at f (/My App (1)/bundle.js:2:1)",
        );
        assert_eq!(out, "    at f (/My App (1)/o.ts:1:1)");
    }

    #[test]
    fn a_trace_from_a_getter_in_mapped_code_prints_one_trace() {
        let code = r"
            Object.defineProperty(Object.prototype, 'sourceRoot', {
                get() {
                    console.trace('inner');
                },
                configurable: true,
            });
            console.trace('outer');
            "
        .trim();
        let body = format!(
            "{code}\n{}",
            inline(&map_json(
                3,
                ";AAAA;AAAA;AAAA;AAAA;AAAA;AAAA;AAAA",
                r#"["src/app.ts"]"#,
                None
            ))
        );
        let lines = with_realm(|ctx| {
            let lines = Rc::new(RefCell::new(Vec::<String>::new()));
            let sink = Rc::clone(&lines);
            let output: crate::console::Output =
                Rc::new(move |_, line: &str| sink.borrow_mut().push(line.to_owned()));
            crate::console::install(&ctx, &output, false).unwrap();
            let f = crate::compile_async_function(&ctx, &[], &body, "/app/a.js").unwrap();
            let _: rquickjs::Value = f.call(()).unwrap();
            while ctx.execute_pending_job() {}
            lines.take()
        });
        assert!(
            lines.len() == 1 && !lines[0].starts_with("Trace: inner"),
            "{lines:?}"
        );
    }

    /// Compiles a function that returns its own stack, under `filename`. A `map` is the inline map
    /// of its source.
    fn compile_stack<'js>(ctx: &Ctx<'js>, filename: &str, map: Option<&str>) -> Function<'js> {
        let marker = map.map_or_else(String::new, |json| format!("\n{}", inline(json)));
        let body = format!("return new Error('x').stack;{marker}");
        crate::compile_async_function(ctx, &[], &body, filename).unwrap()
    }

    /// The stack `function` returns, as `QuickJS` prints it.
    fn stack_of(function: &Function<'_>) -> String {
        function.call::<_, Promise>(()).unwrap().finish().unwrap()
    }

    #[test]
    fn a_recompiled_function_keeps_its_original_map() {
        let first_map = map_json(3, ";AAAA", r#"["first.ts"]"#, None);
        let second_map = map_json(3, ";AAEA", r#"["second.ts"]"#, None);
        with_realm(|ctx| {
            let first = compile_stack(&ctx, "app.js", Some(&first_map));
            let second = compile_stack(&ctx, "app.js", Some(&second_map));

            assert!(remap_stack(&ctx, &stack_of(&first)).contains("(first.ts:1:1)"));
            assert!(remap_stack(&ctx, &stack_of(&second)).contains("(second.ts:3:1)"));
        });
    }

    #[test]
    fn a_compile_without_a_map_keeps_its_requested_filename() {
        let map = map_json(3, ";AAAA", r#"["orig.ts"]"#, None);
        with_realm(|ctx| {
            let mapped = compile_stack(&ctx, "app.js", Some(&map));
            let unmapped = compile_stack(&ctx, "app.js", None);

            assert!(remap_stack(&ctx, &stack_of(&mapped)).contains("(orig.ts:1:1)"));
            assert!(remap_stack(&ctx, &stack_of(&unmapped)).contains("(app.js:2:12)"));
        });
    }

    #[test]
    fn a_filename_with_colons_comes_out_unchanged() {
        with_realm(|ctx| {
            let function = compile_stack(&ctx, "/a:2:3/b.js", None);

            let out = remap_stack(&ctx, &stack_of(&function));

            assert!(out.contains("(/a:2:3/b.js:2:12)"), "{out}");
        });
    }

    #[test]
    fn the_second_compile_of_a_file_evaluates_with_a_t_query() {
        with_realm(|ctx| {
            let first = stack_of(&compile_stack(&ctx, "app.js", None));
            let second = stack_of(&compile_stack(&ctx, "app.js", None));

            assert!(first.contains("(app.js:2:12)"), "{first}");
            assert!(second.contains("(app.js?t="), "{second}");
            assert!(remap_stack(&ctx, &second).contains("(app.js:2:12)"));
        });
    }

    #[test]
    fn an_hmr_style_query_keeps_its_query() {
        with_realm(|ctx| {
            compile_stack(&ctx, "app.js?a=1&b=2", None);
            let second = stack_of(&compile_stack(&ctx, "app.js?a=1&b=2", None));

            assert!(second.contains("(app.js?t="), "{second}");
            assert!(second.contains("&a=1&b=2:2:12)"), "{second}");
            assert!(remap_stack(&ctx, &second).contains("(app.js?a=1&b=2:2:12)"));
        });
    }

    #[test]
    fn compile_ids_run_out_with_an_error() {
        assert!(next_compile_id(&AtomicU64::new(u64::MAX)).is_err());
    }
}
