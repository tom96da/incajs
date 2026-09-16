// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Resolves and loads `import`s against real filesystem paths.

use std::fs;
use std::path::{Path, PathBuf};

use rquickjs::loader::{ImportAttributes, Loader, Resolver};
use rquickjs::{Ctx, Error, Module, Result};

/// Resolves an `import` specifier to a file on disk.
///
/// A `./`- or `../`-relative specifier resolves next to the importing
/// module (`base`, the specifier it was declared under). A bare specifier
/// is searched for under each root, in the order added, and must already
/// carry its extension — neither form appends one.
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
        } else {
            self.roots
                .iter()
                .map(|root| root.join(name))
                .find(|path| path.is_file())
        };

        candidate
            .filter(|path| path.is_file())
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
