// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Compiles source text into an async function that reports its own file.

use rquickjs::context::EvalOptions;
use rquickjs::{Ctx, Function, Result};

/// Compiles `body` into an async function that takes `params`, named
/// `filename` in its stack frames.
///
/// The body runs in strict mode in the global scope. `params` are bound in
/// the order given. The source is `(async function (params) {` on line 1,
/// then `body` from line 2, so a frame on line `N` of `body` reports line
/// `N + 1` of `filename`, and columns on every line of `body` are exact.
/// Lines are counted as `QuickJS` counts them, and each `params` entry must
/// fit on one line. A `//` comment on the last line of `body` is allowed.
/// An inline `data:` source map in `body` maps its frames to the original
/// file, line and column. A syntax error is mapped too. A recompile of the
/// same `filename` replaces its source and map.
///
/// `params` and `body` run as code, so pass only trusted source.
///
/// # Errors
///
/// Returns [`rquickjs::Error::Exception`] for a syntax error in `params` or
/// `body`. The thrown `SyntaxError`, whose stack names `filename` and the
/// line, is pending on `ctx`. Returns other errors if `filename`, `params` or
/// `body` contains a NUL byte, or if the source evaluates to a non-function.
pub fn compile_async_function<'js>(
    ctx: &Ctx<'js>,
    params: &[&str],
    body: &str,
    filename: &str,
) -> Result<Function<'js>> {
    let source = format!("(async function ({}) {{\n{body}\n}})", params.join(", "));
    // Registered before the eval so a syntax error's frame is remapped too.
    crate::sourcemap::register(ctx, filename, &source);
    let mut options = EvalOptions::default();
    options.filename = Some(filename.to_owned());
    ctx.eval_with_options(source, options)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::{Engine, EngineError, EngineResult};

    use super::*;

    /// Compiles `body` as `f` in `engine`'s global scope.
    fn compile(engine: &Engine, params: &[&str], body: &str, filename: &str) -> EngineResult<()> {
        engine.with(|ctx| {
            let f = compile_async_function(&ctx, params, body, filename)
                .map_err(|err| EngineError::capture(&ctx, &err))?;
            ctx.globals().set("f", f).unwrap();
            Ok(())
        })
    }

    /// Calls `f(args)` and returns what its promise settled with: the value
    /// as text, or the name and stack of a rejection.
    fn settle(engine: &Engine, args: &str) -> String {
        engine
            .eval::<()>(&format!(
                "f({args}).then(\
                    (v) => {{ globalThis.out = String(v); }},\
                    (e) => {{ globalThis.out = e.name + '\\n' + e.stack; }});"
            ))
            .unwrap();
        assert!(engine.run_jobs().is_empty());
        engine.eval("globalThis.out").unwrap()
    }

    #[test]
    fn body_line_n_is_frame_line_n_plus_one_at_the_exact_column() {
        let engine = Engine::new().unwrap();
        let body = "const a = 1;\n  const b = 2;\n  throw new Error('x');";
        compile(&engine, &[], body, "probe.js").unwrap();

        // Column 13 is the `Error` in the third line's `new Error`.
        assert!(settle(&engine, "").contains("probe.js:4:13"));
    }

    #[test]
    fn crlf_line_endings_keep_the_line_mapping() {
        let engine = Engine::new().unwrap();
        compile(
            &engine,
            &[],
            "const a = 1;\r\nthrow new Error('x');",
            "crlf.js",
        )
        .unwrap();

        assert!(settle(&engine, "").contains("crlf.js:3:"));
    }

    #[test]
    fn a_trailing_line_comment_in_the_body_is_allowed() {
        let engine = Engine::new().unwrap();
        compile(&engine, &[], "return 7; // tail", "tail.js").unwrap();

        assert_eq!(settle(&engine, ""), "7");
    }

    #[test]
    fn the_body_is_strict() {
        let engine = Engine::new().unwrap();
        compile(
            &engine,
            &[],
            "undeclared = 1; return 'sloppy';",
            "strict.js",
        )
        .unwrap();

        assert!(settle(&engine, "").starts_with("ReferenceError"));
    }

    #[test]
    fn the_function_is_async_and_resolves_with_its_return_value() {
        let engine = Engine::new().unwrap();
        compile(&engine, &[], "return await Promise.resolve(7);", "await.js").unwrap();

        assert_eq!(settle(&engine, ""), "7");
    }

    #[test]
    fn params_bind_in_the_order_given() {
        let engine = Engine::new().unwrap();
        compile(&engine, &["a", "b"], "return a - b;", "order.js").unwrap();

        assert_eq!(settle(&engine, "5, 2"), "3");
    }

    #[test]
    fn a_syntax_error_names_the_file_and_the_line() {
        let engine = Engine::new().unwrap();
        let err = compile(&engine, &[], "const a = 1;\nconst = ;", "broken.js").unwrap_err();

        assert!(err.to_string().contains("broken.js:3"), "{err}");
    }

    #[test]
    fn a_syntax_error_in_a_mapped_body_names_the_original_file() {
        let engine = Engine::new().unwrap();
        let body = crate::sourcemap::app_vue_body("const = ;");
        let err = compile(&engine, &[], &body, "virtual.js").unwrap_err();

        assert!(err.to_string().contains("src/App.vue:12:5"), "{err}");
    }

    #[test]
    fn a_filename_with_a_nul_byte_is_an_error() {
        let engine = Engine::new().unwrap();

        assert!(compile(&engine, &[], "return 1;", "a\0b.js").is_err());
    }

    #[test]
    fn a_body_that_ends_the_function_early_is_an_error() {
        let engine = Engine::new().unwrap();

        assert!(compile(&engine, &[], "}), 42, ({", "early.js").is_err());
    }
}
