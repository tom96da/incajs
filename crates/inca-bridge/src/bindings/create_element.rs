// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `createElement` native: element names and the node they create.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Array, Ctx, Exception, Result as JsResult};

use super::convert::throw_tree_error;
use super::host::Host;

/// Creates the element `local_name` names, lowered in ASCII, and gives its id and tag. A name
/// that is no valid element local name throws `InvalidCharacterError`.
pub(super) fn create_element<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, String) -> JsResult<Array<'js>> {
    move |ctx, local_name| {
        if !is_element_local_name(&local_name) {
            return Err(Exception::throw_dom(
                &ctx,
                "InvalidCharacterError",
                "the element name is not valid. pass a name such as div",
            ));
        }
        let tag = local_name.to_ascii_lowercase();
        let id = host
            .borrow_mut()
            .tree
            .create_node(tag.clone())
            .map_err(|err| throw_tree_error(&ctx, err))?;
        let pair = Array::new(ctx)?;
        pair.set(0, id)?;
        pair.set(1, tag)?;
        Ok(pair)
    }
}

/// An ASCII letter followed by no ASCII whitespace, NUL, `/` or `>`, or a name that starts with
/// `:`, `_` or a code point from U+0080 on and continues with name characters.
fn is_element_local_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => {
            !chars.any(|c| c.is_ascii_whitespace() || matches!(c, '\0' | '/' | '>'))
        }
        Some(':' | '_' | '\u{80}'..) => chars.all(is_name_char),
        _ => false,
    }
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, ':' | '_' | '-' | '.') || c >= '\u{80}'
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::test_support::{engine_with_bindings, error_name};

    #[rstest]
    #[case::upper("DIV", "div")]
    #[case::colon_first(":a", ":a")]
    #[case::underscore_first("_a", "_a")]
    #[case::colon_inside("a:b", "a:b")]
    #[case::non_ascii_stays("\u{c9}", "\u{c9}")]
    #[case::underscore_then_name_characters("_a-1.b", "_a-1.b")]
    #[case::underscore_then_a_non_xml_character("_\u{d7}", "_\u{d7}")]
    fn a_valid_name_creates_an_element_with_the_lowered_tag(#[case] name: &str, #[case] tag: &str) {
        let (engine, host) = engine_with_bindings();

        engine
            .eval::<()>(&format!(
                "globalThis.pair = __inca_native__.createElement('{name}');"
            ))
            .unwrap();
        let id: u32 = engine.eval("pair[0]").unwrap();
        let given: String = engine.eval("pair[1]").unwrap();

        assert_eq!(host.borrow().tree.get(id).unwrap().tag_name(), tag);
        assert_eq!(given, tag);
    }

    #[rstest]
    #[case::empty("")]
    #[case::hash("#document")]
    #[case::space("a b")]
    #[case::greater_than("a>")]
    #[case::slash("a/")]
    #[case::nul("a\\u0000")]
    #[case::colon_first_then_a_space(":a b")]
    fn an_invalid_name_throws_invalid_character_error(#[case] name: &str) {
        let (engine, _host) = engine_with_bindings();

        assert_eq!(
            error_name(&engine, &format!("__inca_native__.createElement('{name}')")),
            "InvalidCharacterError"
        );
    }
}
