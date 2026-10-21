// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

/// The `keyCode` of the key named `key`, as a US keyboard layout gives it. A key outside that
/// layout is 0.
pub(super) fn key_code(key: &str) -> u32 {
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return char_code(c);
    }
    match key {
        "Backspace" => 8,
        "Tab" => 9,
        "Enter" => 13,
        "Shift" => 16,
        "Control" => 17,
        "Alt" => 18,
        "Escape" => 27,
        "PageUp" => 33,
        "PageDown" => 34,
        "End" => 35,
        "Home" => 36,
        "ArrowLeft" => 37,
        "ArrowUp" => 38,
        "ArrowRight" => 39,
        "ArrowDown" => 40,
        "Insert" => 45,
        "Delete" => 46,
        "Meta" => 91,
        "BrowserBack" => 166,
        "BrowserForward" => 167,
        _ => function_key(key).unwrap_or(0),
    }
}

/// The `keyCode` of `F1` to `F24`.
fn function_key(key: &str) -> Option<u32> {
    let number: u32 = key.strip_prefix('F')?.parse().ok()?;
    (1..=24).contains(&number).then_some(111 + number)
}

/// The `keyCode` of the key that types `c`.
fn char_code(c: char) -> u32 {
    match c {
        'a'..='z' => u32::from(c.to_ascii_uppercase()),
        'A'..='Z' | '0'..='9' => u32::from(c),
        ' ' => 32,
        ')' => 48,
        '!' => 49,
        '@' => 50,
        '#' => 51,
        '$' => 52,
        '%' => 53,
        '^' => 54,
        '&' => 55,
        '*' => 56,
        '(' => 57,
        ';' | ':' => 186,
        '=' | '+' => 187,
        ',' | '<' => 188,
        '-' | '_' => 189,
        '.' | '>' => 190,
        '/' | '?' => 191,
        '`' | '~' => 192,
        '[' | '{' => 219,
        '\\' | '|' => 220,
        ']' | '}' => 221,
        '\'' | '"' => 222,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::lower_letter("a", 65)]
    #[case::last_lower_letter("z", 90)]
    #[case::upper_letter("A", 65)]
    #[case::last_upper_letter("Z", 90)]
    #[case::digit("0", 48)]
    #[case::last_digit("9", 57)]
    #[case::space(" ", 32)]
    #[case::close_paren(")", 48)]
    #[case::bang("!", 49)]
    #[case::at("@", 50)]
    #[case::hash("#", 51)]
    #[case::dollar("$", 52)]
    #[case::percent("%", 53)]
    #[case::caret("^", 54)]
    #[case::ampersand("&", 55)]
    #[case::star("*", 56)]
    #[case::open_paren("(", 57)]
    #[case::semicolon(";", 186)]
    #[case::colon(":", 186)]
    #[case::equals("=", 187)]
    #[case::plus("+", 187)]
    #[case::comma(",", 188)]
    #[case::less("<", 188)]
    #[case::minus("-", 189)]
    #[case::underscore("_", 189)]
    #[case::period(".", 190)]
    #[case::greater(">", 190)]
    #[case::slash("/", 191)]
    #[case::question("?", 191)]
    #[case::backquote("`", 192)]
    #[case::tilde("~", 192)]
    #[case::open_bracket("[", 219)]
    #[case::open_brace("{", 219)]
    #[case::backslash("\\", 220)]
    #[case::pipe("|", 220)]
    #[case::close_bracket("]", 221)]
    #[case::close_brace("}", 221)]
    #[case::quote("'", 222)]
    #[case::double_quote("\"", 222)]
    #[case::other_character("é", 0)]
    #[case::backspace("Backspace", 8)]
    #[case::tab("Tab", 9)]
    #[case::enter("Enter", 13)]
    #[case::shift("Shift", 16)]
    #[case::control("Control", 17)]
    #[case::alt("Alt", 18)]
    #[case::escape("Escape", 27)]
    #[case::page_up("PageUp", 33)]
    #[case::page_down("PageDown", 34)]
    #[case::end("End", 35)]
    #[case::home("Home", 36)]
    #[case::arrow_left("ArrowLeft", 37)]
    #[case::arrow_up("ArrowUp", 38)]
    #[case::arrow_right("ArrowRight", 39)]
    #[case::arrow_down("ArrowDown", 40)]
    #[case::insert("Insert", 45)]
    #[case::delete("Delete", 46)]
    #[case::meta("Meta", 91)]
    #[case::browser_back("BrowserBack", 166)]
    #[case::browser_forward("BrowserForward", 167)]
    #[case::first_function_key("F1", 112)]
    #[case::last_function_key("F24", 135)]
    #[case::function_key_past_the_last("F25", 0)]
    #[case::function_key_zero("F0", 0)]
    #[case::other_name("Unidentified", 0)]
    fn a_key_code_follows_the_us_layout(#[case] key: &str, #[case] want: u32) {
        assert_eq!(key_code(key), want);
    }
}
