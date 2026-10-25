// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

#[cfg(unix)]
use std::os::unix::fs::symlink;

use super::*;
use crate::Engine;

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
