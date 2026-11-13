// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `CSSStyleDeclaration` class of `style`.
//!
//! A declaration block holds the id of its node and reads the inline style at every call. A block
//! of a freed node reads empty, and its writes change nothing.

use std::cell::RefCell;
use std::iter::once;
use std::rc::Rc;

// `IncaStyle` derives `JsLifetime`, and the class holds a node id and no JS value.
#[allow(clippy::disallowed_types)]
use rquickjs::JsLifetime;
use rquickjs::convert::Coerced;
use rquickjs::{Class, Ctx, FromJs, Object, Result as JsResult, Value};

use inca_jsenv::class::{
    define_this_accessor, define_this_getter, define_this_method, plain_class, receiver, require,
    text,
};
use inca_node::{
    NodeId, STYLE_KEYS, TreeError, VirtualNode, css_text, style_item, style_length,
    style_property_value,
};

use super::Natives;
use super::attributes::print_style_warning;
use super::convert::{JsNodeId, saturated, unsigned};
use super::host::Host;
use crate::interfaces::define_unconstructable;
use inca_jsenv::required_prototype;

/// A `CSSStyleDeclaration` instance. It holds the node id and no JS value.
// `IncaStyle` holds a node id and no JS value, so the derive is sound.
#[allow(clippy::disallowed_macros)]
#[derive(JsLifetime)]
pub(super) struct IncaStyle {
    node: NodeId,
}

plain_class!(IncaStyle, "CSSStyleDeclaration");

/// `style`, and the class `CSSStyleDeclaration`.
pub(super) fn install_style(n: &Natives<'_, '_>) -> JsResult<()> {
    let ctx = n.ctx;
    let proto = Object::new(ctx.clone())?;
    define_members(ctx, &proto, n.host)?;
    define_properties(ctx, &proto, n.host)?;
    define_unconstructable(
        ctx,
        &proto,
        "CSSStyleDeclaration",
        "a style declaration cannot be constructed. read it from the style of an element",
    )?;
    n.set("styleOf", style_of)
}

/// The block of the node `node_id`, which may be freed.
fn style_of<'js>(
    _host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<Class<'js, IncaStyle>> {
    move |ctx, JsNodeId(node)| {
        let proto = required_prototype(&ctx, "CSSStyleDeclaration")?;
        Class::instance_proto(IncaStyle { node }, proto)
    }
}

fn node_of<'js>(ctx: &Ctx<'js>, this: &Value<'js>) -> JsResult<NodeId> {
    Ok(receiver::<IncaStyle>(ctx, this)?.borrow().node)
}

/// What `read` makes of the node, the default when the node is freed.
fn read<T: Default>(host: &RefCell<Host>, node: NodeId, read: impl Fn(&VirtualNode) -> T) -> T {
    host.borrow().tree.get(node).map_or_else(T::default, read)
}

/// Prints the warning lines of a write that reached the node and queues the focus re-check when
/// the write touched `display`. A write to a freed node is dropped.
fn finish(
    host: &RefCell<Host>,
    node: NodeId,
    display: bool,
    result: Result<Vec<String>, TreeError>,
) {
    let Ok(lines) = result else { return };
    let mut host = host.borrow_mut();
    for line in &lines {
        print_style_warning(&mut host, node, line);
    }
    if display {
        host.mark_focus_dirty(node);
    }
}

fn is_display(name: &str) -> bool {
    name.eq_ignore_ascii_case("display")
}

/// The text of a written value: `null` is empty and a `Symbol` throws a `TypeError`.
fn value_text<'js>(ctx: &Ctx<'js>, value: &Value<'js>) -> JsResult<String> {
    if value.is_null() {
        return Ok(String::new());
    }
    text(ctx, value)
}

fn define_members<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    host: &Rc<RefCell<Host>>,
) -> JsResult<()> {
    let h = host.clone();
    define_this_getter(ctx, proto, "length", move |ctx, this| {
        let length = read(&h, node_of(ctx, &this)?, style_length);
        Ok(saturated(length))
    })?;
    let (get, set) = (host.clone(), host.clone());
    define_this_accessor(
        ctx,
        proto,
        "cssText",
        move |ctx, this| Ok(read(&get, node_of(ctx, &this)?, css_text)),
        move |ctx, this, value: Value<'js>| {
            let node = node_of(ctx, &this)?;
            let value = value_text(ctx, &value)?;
            let result = set.borrow_mut().tree.set_style_text(node, &value);
            finish(&set, node, true, result);
            Ok(())
        },
    )?;
    let h = host.clone();
    define_this_method(ctx, proto, "getPropertyValue", 1, move |ctx, this, args| {
        let node = node_of(ctx, &this)?;
        require(ctx, "getPropertyValue", args, 1)?;
        let name = text(ctx, &args[0])?;
        Ok(read(&h, node, |node| style_property_value(node, &name)))
    })?;
    let h = host.clone();
    define_this_method(ctx, proto, "setProperty", 2, move |ctx, this, args| {
        let node = node_of(ctx, &this)?;
        require(ctx, "setProperty", args, 2)?;
        let name = text(ctx, &args[0])?;
        let value = value_text(ctx, &args[1])?;
        let priority = match args.get(2) {
            Some(priority) if !priority.is_undefined() && !priority.is_null() => {
                text(ctx, priority)?
            }
            _ => String::new(),
        };
        let refused = !priority.is_empty() && !priority.eq_ignore_ascii_case("important");
        if refused && !value.is_empty() {
            return Ok(());
        }
        let result = h.borrow_mut().tree.set_style_property(node, &name, &value);
        finish(&h, node, is_display(&name), result);
        Ok(())
    })?;
    let h = host.clone();
    define_this_method(ctx, proto, "removeProperty", 1, move |ctx, this, args| {
        let node = node_of(ctx, &this)?;
        require(ctx, "removeProperty", args, 1)?;
        let name = text(ctx, &args[0])?;
        let former = h.borrow_mut().tree.remove_style_property(node, &name);
        if former.is_ok() && is_display(&name) {
            h.borrow_mut().mark_focus_dirty(node);
        }
        Ok(former.unwrap_or_default())
    })?;
    define_this_method(ctx, proto, "getPropertyPriority", 1, |ctx, this, args| {
        node_of(ctx, &this)?;
        require(ctx, "getPropertyPriority", args, 1)?;
        text(ctx, &args[0])?;
        Ok(String::new())
    })?;
    let h = host.clone();
    define_this_method(ctx, proto, "item", 1, move |ctx, this, args| {
        let node = node_of(ctx, &this)?;
        require(ctx, "item", args, 1)?;
        let index = unsigned(Coerced::<i32>::from_js(ctx, args[0].clone())?);
        Ok(read(&h, node, |node| {
            style_item(node, index as usize).unwrap_or_default()
        }))
    })?;
    Ok(())
}

/// The accessor of every property name: each key and CSS alias of the host in camelCase and in
/// kebab-case.
fn define_properties<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    host: &Rc<RefCell<Host>>,
) -> JsResult<()> {
    for (name, css) in property_names() {
        let (get, set) = (host.clone(), host.clone());
        let (read_name, write_name) = (css.clone(), css);
        define_this_accessor(
            ctx,
            proto,
            &name,
            move |ctx, this| {
                let node = node_of(ctx, &this)?;
                Ok(read(&get, node, |node| {
                    style_property_value(node, &read_name)
                }))
            },
            move |ctx, this, value: Value<'js>| {
                let node = node_of(ctx, &this)?;
                let value = value_text(ctx, &value)?;
                let result = set
                    .borrow_mut()
                    .tree
                    .set_style_property(node, &write_name, &value);
                finish(&set, node, is_display(&write_name), result);
                Ok(())
            },
        )?;
    }
    Ok(())
}

/// The distinct camelCase and kebab-case spellings of every key and CSS alias, each with the CSS
/// name that reads and writes it.
fn property_names() -> Vec<(String, String)> {
    let mut names: Vec<(String, String)> = Vec::new();
    for entry in STYLE_KEYS {
        for raw in once(entry.key).chain(entry.aliases.iter().copied()) {
            let mut words = raw.split('_');
            let mut camel = words.next().unwrap_or_default().to_owned();
            for word in words {
                camel.extend(word.chars().take(1).flat_map(char::to_uppercase));
                camel.push_str(word.get(1..).unwrap_or_default());
            }
            let css = raw.replace('_', "-");
            for name in [css.clone(), camel] {
                if names.iter().all(|(known, _)| *known != name) {
                    names.push((name, css.clone()));
                }
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::cell::RefCell;
    use std::rc::Rc;

    use rstest::rstest;
    use serde_json::{Value, json};

    use crate::test_support::engine_with_bindings;

    /// Runs `body` as a function body next to the natives `n`, a new div `id` and its block
    /// `style`. Returns what `body` returns as JSON, and the warning lines the host printed.
    fn exec(body: &str) -> (Value, Vec<String>) {
        let (engine, host) = engine_with_bindings();
        let warnings = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&warnings);
        host.borrow_mut().warn = Rc::new(move |line| sink.borrow_mut().push(line.to_owned()));
        let json: String = engine
            .eval(&format!(
                r"
                JSON.stringify((() => {{
                    const n = __inca_native__;
                    const id = n.createNode('div');
                    const style = n.styleOf(id);
                    {body}
                }})())
                "
            ))
            .unwrap();
        let lines = warnings.borrow().clone();
        (serde_json::from_str(&json).unwrap(), lines)
    }

    fn run(body: &str) -> Value {
        exec(body).0
    }

    /// [`exec`] with the div destroyed after the block is read from it.
    fn exec_freed(body: &str) -> (Value, Vec<String>) {
        exec(&format!("n.destroyNode(id);\n{body}"))
    }

    /// The `name` of the error `call` throws, `'none'` when it throws nothing.
    fn thrown(call: &str) -> Value {
        run(&format!(
            r"
            try {{
                {call};
                return 'none';
            }} catch (e) {{
                return e.name;
            }}
            "
        ))
    }

    #[test]
    fn get_property_value_reads_the_canonical_text() {
        let got = run(r"
            style.setProperty('width', '10PX');
            return style.getPropertyValue('width');
            ");

        assert_eq!(got, json!("10px"));
    }

    #[test]
    fn get_property_value_of_a_camel_case_name_is_empty() {
        let got = run(r"
            style.setProperty('background-color', '#ff0000');
            return style.getPropertyValue('backgroundColor');
            ");

        assert_eq!(got, json!(""));
    }

    #[test]
    fn length_counts_the_declarations() {
        let got = run(r"
            style.setProperty('width', '1px');
            style.setProperty('height', '2px');
            return style.length;
            ");

        assert_eq!(got, json!(2));
    }

    #[test]
    fn item_names_the_declarations_in_order_and_is_empty_past_the_last() {
        let got = run(r"
            style.setProperty('width', '1px');
            style.setProperty('height', '2px');
            return [style.item(0), style.item(1), style.item(2)];
            ");

        assert_eq!(got, json!(["width", "height", ""]));
    }

    #[rstest]
    #[case::numeric_string("'1'", "height")]
    #[case::fraction("1.9", "height")]
    #[case::negative("-1", "")]
    fn item_converts_its_index_to_an_unsigned_integer(#[case] index: &str, #[case] name: &str) {
        let got = run(&format!(
            r"
            style.cssText = 'width: 1px; height: 2px';
            return style.item({index});
            "
        ));

        assert_eq!(got, json!(name));
    }

    #[test]
    fn remove_property_returns_the_former_text() {
        let got = run(r"
            style.setProperty('width', '1px');
            return style.removeProperty('width');
            ");

        assert_eq!(got, json!("1px"));
    }

    #[test]
    fn remove_property_removes_the_declaration() {
        let got = run(r"
            style.setProperty('width', '1px');
            style.removeProperty('width');
            return [style.getPropertyValue('width'), style.length];
            ");

        assert_eq!(got, json!(["", 0]));
    }

    #[test]
    fn css_text_reads_the_declarations_in_the_order_they_were_set() {
        let got = run(r"
            style.setProperty('height', '2px');
            style.setProperty('width', '1px');
            return style.cssText;
            ");

        assert_eq!(got, json!("height: 2px; width: 1px;"));
    }

    #[test]
    fn writing_css_text_replaces_the_declarations() {
        let got = run(r"
            style.setProperty('width', '1px');
            style.cssText = 'height: 2px';
            return style.cssText;
            ");

        assert_eq!(got, json!("height: 2px;"));
    }

    #[test]
    fn writing_css_text_stores_the_style_attribute() {
        let got = run(r"
            style.cssText = 'height: 2px';
            return n.getAttribute(id, 'style');
            ");

        assert_eq!(got, json!("height: 2px;"));
    }

    #[test]
    fn writing_null_to_css_text_empties_the_block_without_a_warning() {
        let (got, warnings) = exec(
            r"
            style.cssText = 'width: 1px';
            style.cssText = null;
            return style.cssText;
            ",
        );

        assert_eq!((got, warnings), (json!(""), Vec::<String>::new()));
    }

    #[test]
    fn writing_undefined_to_css_text_prints_the_dropped_declaration() {
        let (_, warnings) = exec(
            r"
            style.cssText = undefined;
            return 0;
            ",
        );

        assert_eq!(warnings.len(), 1, "{warnings:?}");
    }

    #[test]
    fn a_property_written_with_a_unit_is_stored() {
        let (got, warnings) = exec(
            r"
            style.width = '300px';
            return style.width;
            ",
        );

        assert_eq!((got, warnings), (json!("300px"), Vec::<String>::new()));
    }

    #[test]
    fn a_number_written_to_a_property_leaves_the_value_and_prints_a_warning() {
        let (got, warnings) = exec(
            r"
            style.width = '1px';
            style.width = 300;
            return style.width;
            ",
        );

        assert_eq!((got, warnings.len()), (json!("1px"), 1), "{warnings:?}");
    }

    #[test]
    fn a_warning_prints_once_for_each_node_and_line() {
        let (_, warnings) = exec(
            r"
            style.width = 300;
            style.width = 300;
            style.setProperty('width', '300');
            return 0;
            ",
        );

        assert_eq!(warnings.len(), 1, "{warnings:?}");
    }

    #[rstest]
    #[case::empty_string("''")]
    #[case::null("null")]
    fn writing_an_empty_value_to_a_property_removes_it(#[case] empty: &str) {
        let got = run(&format!(
            r"
            style.width = '1px';
            style.width = {empty};
            return [style.width, style.length];
            "
        ));

        assert_eq!(got, json!(["", 0]));
    }

    #[test]
    fn a_symbol_written_to_a_property_leaves_the_value() {
        let got = run(r"
            style.width = '1px';
            try {
                style.width = Symbol('x');
            } catch {
            }
            return style.width;
            ");

        assert_eq!(got, json!("1px"));
    }

    #[rstest]
    #[case::property_write("style.width = Symbol('x')")]
    #[case::css_text_write("style.cssText = Symbol('x')")]
    #[case::get_property_value("style.getPropertyValue(Symbol('x'))")]
    #[case::set_property_name("style.setProperty(Symbol('x'), '1px')")]
    #[case::set_property_value("style.setProperty('width', Symbol('x'))")]
    #[case::remove_property("style.removeProperty(Symbol('x'))")]
    #[case::get_property_priority("style.getPropertyPriority(Symbol('x'))")]
    fn a_symbol_argument_throws_a_type_error(#[case] call: &str) {
        assert_eq!(thrown(call), json!("TypeError"));
    }

    #[test]
    fn text_color_and_color_share_one_declaration() {
        let got = run(r"
            style.textColor = '#ff0000';
            return [style.color, style['text-color'], style.item(0), style.length];
            ");

        assert_eq!(got, json!(["rgb(255, 0, 0)", "rgb(255, 0, 0)", "color", 1]));
    }

    #[test]
    fn camel_case_and_kebab_case_share_one_declaration() {
        let got = run(r"
            style.flexDirection = 'column';
            return [style['flex-direction'], style.length];
            ");

        assert_eq!(got, json!(["column", 1]));
    }

    #[rstest]
    #[case::one_word("width", true)]
    #[case::camel_case("flexDirection", true)]
    #[case::kebab_case("flex-direction", true)]
    #[case::host_key("textColor", true)]
    #[case::css_alias("color", true)]
    #[case::snake_case("flex_direction", false)]
    #[case::unknown("foo", false)]
    fn a_property_name_is_a_member_of_the_block(#[case] name: &str, #[case] member: bool) {
        let got = run(&format!("return '{name}' in style;"));

        assert_eq!(got, json!(member));
    }

    #[test]
    fn a_name_outside_the_vocabulary_is_a_plain_property() {
        let (got, warnings) = exec(
            r"
            style.foo = 'x';
            return [style.foo, style.length];
            ",
        );

        assert_eq!((got, warnings), (json!(["x", 0]), Vec::<String>::new()));
    }

    #[test]
    fn set_property_of_an_unknown_name_prints_a_warning() {
        let (got, warnings) = exec(
            r"
            style.setProperty('foo', 'x');
            return style.length;
            ",
        );

        assert_eq!((got, warnings.len()), (json!(0), 1), "{warnings:?}");
    }

    #[rstest]
    #[case::important("'important'", "1px")]
    #[case::important_in_upper_case("'IMPORTANT'", "1px")]
    #[case::null("null", "1px")]
    #[case::undefined("undefined", "1px")]
    #[case::other_text("'foo'", "")]
    fn set_property_stores_the_value_for_a_valid_priority(
        #[case] priority: &str,
        #[case] stored: &str,
    ) {
        let got = run(&format!(
            r"
            style.setProperty('width', '1px', {priority});
            return style.width;
            "
        ));

        assert_eq!(got, json!(stored));
    }

    #[test]
    fn set_property_with_an_empty_value_and_an_invalid_priority_removes_the_property() {
        let got = run(r"
            style.width = '1px';
            style.setProperty('width', '', 'foo');
            return style.length;
            ");

        assert_eq!(got, json!(0));
    }

    #[test]
    fn get_property_priority_is_empty_for_a_property_set_as_important() {
        let got = run(r"
            style.setProperty('width', '1px', 'important');
            return style.getPropertyPriority('width');
            ");

        assert_eq!(got, json!(""));
    }

    #[rstest]
    #[case::get_property_value("style.getPropertyValue()")]
    #[case::set_property("style.setProperty('width')")]
    #[case::remove_property("style.removeProperty()")]
    #[case::get_property_priority("style.getPropertyPriority()")]
    #[case::item("style.item()")]
    fn a_method_called_with_too_few_arguments_throws_a_type_error(#[case] call: &str) {
        assert_eq!(thrown(call), json!("TypeError"));
    }

    #[rstest]
    #[case::plain_object("CSSStyleDeclaration.prototype.item.call({}, 0)")]
    #[case::made_from_the_prototype("Object.create(CSSStyleDeclaration.prototype).length")]
    #[case::property_write("Object.create(CSSStyleDeclaration.prototype).width = '1px'")]
    fn a_member_called_on_a_foreign_receiver_throws_a_type_error(#[case] call: &str) {
        assert_eq!(thrown(call), json!("TypeError"));
    }

    #[test]
    fn the_string_tag_is_css_style_declaration() {
        let got = run("return Object.prototype.toString.call(style);");

        assert_eq!(got, json!("[object CSSStyleDeclaration]"));
    }

    #[test]
    fn a_freed_node_reads_empty() {
        let (got, _) = exec_freed(
            r"
            return [
                style.width,
                style.getPropertyValue('height'),
                style.removeProperty('width'),
                style.cssText,
                style.item(0),
                style.length,
            ];
            ",
        );

        assert_eq!(got, json!(["", "", "", "", "", 0]));
    }

    #[test]
    fn a_write_to_a_freed_node_changes_nothing_and_prints_no_warning() {
        let (_, warnings) = exec_freed(
            r"
            style.width = 300;
            style.setProperty('foo', 'x');
            style.cssText = 'nope: 1';
            return 0;
            ",
        );

        assert_eq!(warnings, Vec::<String>::new());
    }

    #[test]
    fn set_attribute_style_reads_back_as_declarations() {
        let got = run(r"
            n.setAttribute(id, 'style', 'width:1px');
            return [n.getAttribute(id, 'style'), style.width];
            ");

        assert_eq!(got, json!(["width: 1px;", "1px"]));
    }

    #[test]
    fn set_attribute_style_keeps_the_position_of_the_attribute() {
        let got = run(r"
            n.setAttribute(id, 'a', '1');
            n.setAttribute(id, 'style', 'width:1px');
            n.setAttribute(id, 'b', '2');
            n.setAttribute(id, 'style', 'height:2px');
            return [n.attributeNames(id), style.cssText];
            ");

        assert_eq!(got, json!([["a", "style", "b"], "height: 2px;"]));
    }

    #[test]
    fn set_attribute_style_prints_the_dropped_declarations() {
        let (got, warnings) = exec(
            r"
            n.setAttribute(id, 'style', 'width:1px; nope: 1');
            return style.cssText;
            ",
        );

        assert_eq!(
            (got, warnings.len()),
            (json!("width: 1px;"), 1),
            "{warnings:?}"
        );
    }

    #[test]
    fn remove_attribute_style_clears_the_declarations() {
        let got = run(r"
            n.setAttribute(id, 'style', 'width:1px');
            n.removeAttribute(id, 'style');
            return [n.attributeNames(id), style.length];
            ");

        assert_eq!(got, json!([[], 0]));
    }

    #[rstest]
    #[case::property_write("style.display = 'none'", true)]
    #[case::set_property("style.setProperty('display', 'none')", true)]
    #[case::remove_property("style.removeProperty('display')", true)]
    #[case::css_text("style.cssText = 'width: 1px'", true)]
    #[case::set_attribute("n.setAttribute(id, 'style', 'width: 1px')", true)]
    #[case::remove_attribute("n.removeAttribute(id, 'style')", true)]
    #[case::other_property("style.width = '1px'", false)]
    fn a_style_write_queues_the_focus_check_when_it_can_change_display(
        #[case] write: &str,
        #[case] queued: bool,
    ) {
        let (engine, host) = engine_with_bindings();
        let node: u32 = engine
            .eval(
                r"
                globalThis.n = __inca_native__;
                globalThis.id = n.createNode('div');
                n.setAttribute(id, 'tabindex', 0);
                globalThis.style = n.styleOf(id);
                id;
                ",
            )
            .unwrap();
        host.borrow_mut().focus.take_tab_dirty();

        engine.eval::<()>(write).unwrap();

        let dirty = host.borrow_mut().focus.take_tab_dirty();
        assert_eq!(dirty.contains(&node), queued);
    }
}
