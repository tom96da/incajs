// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `URL` and `URLSearchParams` values.
//!
//! The text is read from the state of the instance, so printing runs no script code, whatever a
//! script replaced or defined on the value.

use super::array::write_more;
use super::object::write_key;
use super::{MAX_ENTRIES, Mode, put, quote, write_value};
use crate::paint::Style;
use crate::url::{Printed, printed};

/// Appends the members of a `URL` or the pairs of a `URLSearchParams`. Returns false for any
/// other value.
pub(super) fn write(
    out: &mut String,
    value: &rquickjs::Value<'_>,
    depth: usize,
    mode: Mode,
) -> bool {
    let at_limit = depth >= mode.max_depth;
    let Some(printed) = printed(value, if at_limit { 0 } else { MAX_ENTRIES }) else {
        return false;
    };
    let name = match printed {
        Printed::Url { .. } => "URL",
        Printed::Params { .. } => "URLSearchParams",
    };
    if at_limit {
        put(out, mode, Style::Cyan, &format!("[{name}]"));
        return true;
    }
    match printed {
        Printed::Url { members, params } => {
            out.push_str("URL { ");
            for (index, (key, text)) in members.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write_key(out, key, false, mode);
                put(out, mode, Style::Green, &quote(text));
                if *key == "search" {
                    out.push_str(", ");
                    write_key(out, "searchParams", false, mode);
                    write_value(out, &params, depth + 1, mode);
                }
            }
            out.push_str(" }");
        }
        Printed::Params { len: 0, .. } => out.push_str("URLSearchParams {}"),
        Printed::Params { pairs, len } => {
            out.push_str("URLSearchParams { ");
            for (index, (key, text)) in pairs.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                put(out, mode, Style::Green, &quote(key));
                out.push_str(" => ");
                put(out, mode, Style::Green, &quote(text));
            }
            if len > MAX_ENTRIES {
                let mut written = true;
                #[allow(clippy::cast_precision_loss)]
                write_more(out, &mut written, (len - MAX_ENTRIES) as f64, mode);
            }
            out.push_str(" }");
        }
    }
    true
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::tests::{balanced, cyan, green, written_by};

    /// The lines `source` logs in a realm that has `URL` installed.
    fn url_lines(source: &str, color: bool) -> Vec<String> {
        written_by(color, |ctx| {
            crate::vault::create_vault(ctx).unwrap();
            crate::url::install(ctx).unwrap();
            ctx.eval::<(), _>(source).unwrap();
        })
        .into_iter()
        .map(|(_, text)| text)
        .collect()
    }

    fn first(source: &str) -> String {
        url_lines(source, false).remove(0)
    }

    #[test]
    fn a_url_prints_its_twelve_members_in_order() {
        assert_eq!(
            first("console.log(new URL('https://u:p@a.example:8080/x/y?q=1#f'))"),
            "URL { href: 'https://u:p@a.example:8080/x/y?q=1#f', origin: 'https://a.example:8080', \
             protocol: 'https:', username: 'u', password: 'p', host: 'a.example:8080', \
             hostname: 'a.example', port: '8080', pathname: '/x/y', search: '?q=1', \
             searchParams: URLSearchParams { 'q' => '1' }, hash: '#f' }"
        );
    }

    #[test]
    fn a_printed_value_equals_its_getter() {
        assert!(
            first("console.log(new URL('file:///a'))").contains("origin: 'file://'"),
            "the printer reads the origin the getter returns"
        );
    }

    #[test]
    fn an_empty_query_prints_empty_search_params() {
        assert!(
            first("console.log(new URL('http://a/'))").contains("searchParams: URLSearchParams {}")
        );
    }

    #[rstest]
    #[case::keeps_order_and_duplicates(
        "b=2&a=1&b=3",
        "URLSearchParams { 'b' => '2', 'a' => '1', 'b' => '3' }"
    )]
    #[case::decodes_the_text("a=%20", "URLSearchParams { 'a' => ' ' }")]
    fn search_params_print_their_pairs(#[case] query: &str, #[case] want: &str) {
        assert_eq!(
            first(&format!("console.log(new URLSearchParams('{query}'))")),
            want
        );
    }

    #[rstest]
    #[case::url("new URL('http://a/')", "[URL]")]
    #[case::params("new URLSearchParams('x=1')", "[URLSearchParams]")]
    fn a_value_at_the_depth_limit_prints_its_name_in_cyan(#[case] value: &str, #[case] name: &str) {
        assert_eq!(
            url_lines(
                &format!("console.log({{ a: {{ b: {{ c: {value} }} }} }})"),
                true
            )[0],
            format!("{{ a: {{ b: {{ c: {} }} }} }}", cyan(name))
        );
    }

    #[test]
    fn the_search_params_of_a_url_one_level_down_print_their_name() {
        assert_eq!(
            first("console.log({ a: { b: new URL('http://a/?x=1') } })"),
            "{ a: { b: URL { href: 'http://a/?x=1', origin: 'http://a', protocol: 'http:', \
             username: '', password: '', host: 'a', hostname: 'a', port: '', pathname: '/', \
             search: '?x=1', searchParams: [URLSearchParams], hash: '' } } }"
        );
    }

    #[test]
    fn search_params_past_one_hundred_pairs_print_the_rest_as_a_count() {
        let out = first(
            r"
            const p = new URLSearchParams();
            for (let i = 0; i < 101; i++) {
                p.append('k' + i, 'v');
            }
            console.log(p);
            ",
        );

        assert!(out.contains("'k99' => 'v', ... 1 more item }"), "{out}");
    }

    #[rstest]
    #[case::url(
        "new URL('http://a/')",
        format!("URL {{ href: {}", green("'http://a/'"))
    )]
    #[case::params(
        "new URLSearchParams('a=1')",
        format!("URLSearchParams {{ {} => {} }}", green("'a'"), green("'1'"))
    )]
    fn keys_print_bare_before_green_strings(#[case] value: &str, #[case] want: String) {
        let out = url_lines(&format!("console.log({value})"), true).remove(0);

        assert!(out.starts_with(&want), "{out}");
    }

    #[test]
    fn a_pair_value_with_a_newline_prints_escaped_in_a_table_cell() {
        let lines = url_lines(
            "console.table([{ p: new URLSearchParams('a=%0A') }])",
            false,
        );

        assert!(
            lines[0].contains("URLSearchParams { 'a' => '\\n' }"),
            "{lines:?}"
        );
    }

    #[test]
    fn color_stays_balanced_across_a_line_break_in_a_pair() {
        let lines = url_lines("console.table([{ p: new URLSearchParams('a=%0A') }])", true);

        assert!(lines[0].lines().all(balanced), "{lines:?}");
    }

    #[test]
    fn a_replaced_accessor_does_not_run_while_a_url_prints() {
        let lines = url_lines(
            r"
            globalThis.calls = 0;
            Object.defineProperty(URL.prototype, 'href', {
                get() {
                    calls++;
                    return 'x';
                },
            });
            console.log(new URL('http://a/'));
            console.log(calls);
            ",
            false,
        );

        assert_eq!(lines[1], "0");
    }

    #[test]
    fn a_proxy_around_a_url_prints_its_members_and_runs_no_trap() {
        let lines = url_lines(
            r"
            globalThis.calls = 0;
            console.log(
                new Proxy(new URL('http://a/'), {
                    get() {
                        calls++;
                    },
                }),
            );
            console.log(calls);
            ",
            false,
        );

        assert!(lines[0].starts_with("URL { href: 'http://a/'"), "{lines:?}");
        assert_eq!(lines[1], "0");
    }

    #[test]
    fn a_throwing_getter_does_not_change_the_printed_url() {
        let lines = url_lines(
            r"
            Object.defineProperty(URL.prototype, 'href', {
                get() {
                    throw 1;
                },
            });
            console.log(new URL('http://a/'));
            ",
            false,
        );

        assert!(lines[0].starts_with("URL { href: 'http://a/'"));
    }

    #[test]
    fn a_subclass_instance_prints_as_the_plain_url() {
        assert!(
            first(
                r"
                class A extends URL {}
                console.log(new A('http://a/'));
                "
            )
            .starts_with("URL { ")
        );
    }

    #[test]
    fn a_property_added_to_an_instance_is_left_out() {
        assert!(
            !first(
                r"
                const u = new URL('http://a/');
                u.extra = 1;
                console.log(u);
                "
            )
            .contains("extra")
        );
    }

    #[rstest]
    #[case::url("new URL('http://a/?x=1')")]
    #[case::params("new URLSearchParams('x=1')")]
    fn percent_o_prints_the_same_text(#[case] value: &str) {
        assert_eq!(
            first(&format!("console.log('%o', {value})")),
            first(&format!("console.log({value})"))
        );
    }

    #[rstest]
    #[case::url("URL", "URL {}")]
    #[case::params("URLSearchParams", "URLSearchParams {}")]
    fn an_object_with_only_the_prototype_prints_plain(#[case] class: &str, #[case] want: &str) {
        assert_eq!(
            first(&format!("console.log(Object.create({class}.prototype))")),
            want
        );
    }
}
