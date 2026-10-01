// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Resolves and loads `import`s against real filesystem paths.

use std::fs;
use std::path::{Component, Path, PathBuf};

use rquickjs::loader::{ImportAttributes, Loader, Resolver};
use rquickjs::{Ctx, Error, Module, Result};

/// Resolves an `import` specifier to a file on disk.
///
/// A specifier must already carry its extension. A `./`- or `../`-relative
/// one resolves next to the importing module (`base`, the specifier it was
/// declared under). A bare one is searched for under each root, in the order
/// added, and must stay inside it: an absolute path, or a `..` that climbs
/// above the root, resolves to nothing.
#[derive(Debug, Default)]
pub struct DiskResolver {
    roots: Vec<PathBuf>,
}

impl DiskResolver {
    /// A resolver searching bare specifiers under `roots`, in order.
    pub fn new(roots: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            roots: roots.into_iter().collect(),
        }
    }
}

/// Whether a bare specifier stays inside the root it is joined onto.
fn stays_in_root(name: &str) -> bool {
    let mut depth = 0usize;
    for component in Path::new(name).components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => match depth.checked_sub(1) {
                Some(shallower) => depth = shallower,
                None => return false,
            },
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

/// `name` canonicalized when it is a file on disk, else unchanged.
pub(crate) fn canonical_name(name: &str) -> String {
    fs::canonicalize(name).map_or_else(
        |_| name.to_owned(),
        |path| path.to_string_lossy().into_owned(),
    )
}

impl Resolver for DiskResolver {
    fn resolve<'js>(
        &mut self,
        _ctx: &Ctx<'js>,
        base: &str,
        name: &str,
        _attributes: Option<ImportAttributes<'js>>,
    ) -> Result<String> {
        let candidate = if name.starts_with('.') {
            Path::new(base).parent().map(|dir| dir.join(name))
        } else if stays_in_root(name) {
            self.roots
                .iter()
                .map(|root| root.join(name))
                .find(|path| path.is_file())
        } else {
            None
        };

        candidate
            .and_then(|path| fs::canonicalize(path).ok())
            .map(|path| path.to_string_lossy().into_owned())
            .ok_or_else(|| Error::new_resolving(base, name))
    }
}

/// Reads and declares the module at the path a [`DiskResolver`] resolved.
#[derive(Debug, Default)]
pub struct DiskLoader;

impl Loader for DiskLoader {
    fn load<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        path: &str,
        _attributes: Option<ImportAttributes<'js>>,
    ) -> Result<Module<'js>> {
        let source = fs::read_to_string(path)
            .map_err(|err| Error::new_loading_message(path, err.to_string()))?;
        Module::declare(ctx.clone(), path, source)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::env;

    use super::*;
    use crate::Engine;

    /// A directory under the OS temp root, unique per test invocation, torn
    /// down on drop.
    struct ScratchDir(PathBuf);

    impl ScratchDir {
        fn new(name: &str) -> Self {
            let path = env::temp_dir().join(format!(
                "inca-jsenv-loader-test-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A root holding `sub/dep.js` (sets `globalThis.seen`), beside a
    /// `outside.js` that is not under the root.
    fn layout(name: &str) -> (ScratchDir, PathBuf) {
        let dir = ScratchDir::new(name);
        let root = dir.0.join("root");
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("sub/dep.js"), "globalThis.seen = 1;").unwrap();
        fs::write(dir.0.join("outside.js"), "globalThis.seen = 2;").unwrap();
        (dir, root)
    }

    fn import_seen(root: &Path, specifier: &str) -> Option<i32> {
        let engine = Engine::builder().module_root(root).build().unwrap();
        engine
            .eval_module("entry.mjs", &format!("import '{specifier}';"))
            .ok()?;
        engine.eval("globalThis.seen").ok()
    }

    #[test]
    fn a_bare_specifier_inside_the_root_resolves() {
        let (_dir, root) = layout("inside");
        assert_eq!(import_seen(&root, "sub/dep.js"), Some(1));
    }

    #[test]
    fn a_dot_segment_is_ignored() {
        let (_dir, root) = layout("dot");
        assert_eq!(import_seen(&root, "sub/./dep.js"), Some(1));
    }

    #[test]
    fn a_duplicate_separator_is_ignored() {
        let (_dir, root) = layout("duplicate-separator");
        assert_eq!(import_seen(&root, "sub//dep.js"), Some(1));
    }

    #[test]
    fn a_parent_segment_that_stays_in_the_root_resolves() {
        let (_dir, root) = layout("parent-inside");
        assert_eq!(import_seen(&root, "sub/../sub/dep.js"), Some(1));
    }

    #[test]
    fn a_parent_segment_that_leaves_the_root_is_unresolved() {
        let (_dir, root) = layout("parent-escape");
        assert_eq!(import_seen(&root, "sub/../../outside.js"), None);
        assert_eq!(import_seen(&root, "sub/../../root/sub/dep.js"), None);
    }

    #[test]
    fn an_absolute_specifier_is_unresolved() {
        let (_dir, root) = layout("absolute");
        let inside = root.join("sub/dep.js");
        let outside = root.parent().unwrap().join("outside.js");
        assert_eq!(import_seen(&root, &inside.to_string_lossy()), None);
        assert_eq!(import_seen(&root, &outside.to_string_lossy()), None);
    }

    #[test]
    fn a_relative_specifier_still_resolves_next_to_its_importer() {
        let (_dir, root) = layout("relative");
        let entry = root.join("sub/entry.js");
        let engine = Engine::builder().module_root(&root).build().unwrap();
        engine
            .eval_module(&entry.to_string_lossy(), "import '../sub/dep.js';")
            .unwrap();
        assert_eq!(engine.eval::<i32>("globalThis.seen").unwrap(), 1);
    }

    #[test]
    fn canonical_name_collapses_every_spelling_of_a_file() {
        let (_dir, root) = layout("canonical");
        let plain = root.join("sub/dep.js");
        let canonical = fs::canonicalize(&plain)
            .unwrap()
            .to_string_lossy()
            .into_owned();

        assert_eq!(canonical_name(&plain.to_string_lossy()), canonical);
        let dotted = root.join("sub/../sub/./dep.js");
        assert_eq!(canonical_name(&dotted.to_string_lossy()), canonical);
        let doubled = format!("{}//sub//dep.js", root.display());
        assert_eq!(canonical_name(&doubled), canonical);
    }

    #[test]
    fn canonical_name_leaves_a_name_that_is_not_a_file_alone() {
        assert_eq!(canonical_name("probe.mjs"), "probe.mjs");
        assert_eq!(canonical_name("/no/such/file.js"), "/no/such/file.js");
    }
}
