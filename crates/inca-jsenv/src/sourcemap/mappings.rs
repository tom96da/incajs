// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Decodes the `mappings` of a source map and looks a position up in them.

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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn sp(source: u32, line: u32, column: u32) -> SourcePos {
        SourcePos {
            source,
            line,
            column,
        }
    }

    fn decode(mappings: &str) -> Vec<Vec<Segment>> {
        decode_mappings(mappings).unwrap()
    }

    #[test]
    fn single_digit_deltas_apply_their_sign() {
        assert_eq!(
            decode("CAAA"),
            [[Segment {
                column: 1,
                source: Some(sp(0, 0, 0)),
            }]]
        );
        assert_eq!(decode("AAIA;AAFA")[0][0].source, Some(sp(0, 4, 0)));
        assert_eq!(decode("AAIA;AAFA")[1][0].source, Some(sp(0, 2, 0)));
    }

    #[test]
    fn multi_digit_vlq_joins_continuation_digits() {
        assert_eq!(decode("gBAAA")[0][0].column, 16);
    }

    #[test]
    fn empty_lines_between_semicolons_have_no_segments() {
        let lines = decode("AAAA;;AAAA");
        assert_eq!(lines.len(), 3);
        assert!(lines[1].is_empty());
        assert_eq!(lines[2].len(), 1);
    }

    #[test]
    fn one_field_segment_has_no_source() {
        assert_eq!(
            decode("C"),
            [[Segment {
                column: 1,
                source: None,
            }]]
        );
    }

    #[test]
    fn four_and_five_field_segments_carry_a_source() {
        // The fifth field, a name index, is read and ignored.
        assert_eq!(
            decode("AAAA,CAAAC"),
            [[
                Segment {
                    column: 0,
                    source: Some(sp(0, 0, 0)),
                },
                Segment {
                    column: 1,
                    source: Some(sp(0, 0, 0)),
                },
            ]]
        );
    }

    #[test]
    fn source_deltas_carry_across_lines() {
        let lines = decode("CAIE;AAAA");
        assert_eq!(lines[0][0].source, Some(sp(0, 4, 2)));
        assert_eq!(lines[1][0].source, Some(sp(0, 4, 2)));
    }

    #[test]
    fn the_generated_column_resets_on_each_line() {
        let lines = decode("CAIE;AAAA");
        assert_eq!(lines[0][0].column, 1);
        assert_eq!(lines[1][0].column, 0);
    }

    #[test]
    fn invalid_base64_character_is_none() {
        assert_eq!(decode_mappings("A%AA"), None);
    }

    #[test]
    fn truncated_continuation_is_none() {
        assert_eq!(decode_mappings("AAAg"), None);
    }

    #[test]
    fn vlq_beyond_twelve_digits_is_none() {
        let mappings = format!("{}A", "g".repeat(12));
        assert_eq!(decode_mappings(&mappings), None);
    }

    #[test]
    fn column_beyond_u32_is_none() {
        // Digits 0-5 are continuation digits of zero, digit 6 is 8; the value is 2^33.
        assert_eq!(decode_mappings("ggggggIAAA"), None);
    }

    #[test]
    fn segment_with_two_or_three_fields_is_none() {
        assert_eq!(decode_mappings("AA"), None);
        assert_eq!(decode_mappings("AAA"), None);
    }

    #[test]
    fn segment_with_six_fields_is_none() {
        assert_eq!(decode_mappings("AAAAAA"), None);
    }

    #[test]
    fn empty_segment_is_none() {
        assert_eq!(decode_mappings("AAAA,,AAAA"), None);
    }

    #[test]
    fn negative_absolute_value_is_none() {
        assert_eq!(decode_mappings("DAAA"), None);
        assert_eq!(decode_mappings("ADAA"), None);
    }

    #[test]
    fn lookup_before_first_segment_is_none() {
        let lines = vec![vec![Segment {
            column: 3,
            source: Some(sp(0, 0, 0)),
        }]];
        assert_eq!(lookup(&lines, 0, 2), None);
    }

    #[test]
    fn lookup_on_segment_column_returns_its_source() {
        let lines = sample_lines();
        assert_eq!(lookup(&lines, 0, 5), Some(sp(1, 3, 4)));
    }

    #[test]
    fn lookup_between_segments_picks_the_earlier() {
        let lines = sample_lines();
        assert_eq!(lookup(&lines, 0, 3), Some(sp(0, 1, 2)));
    }

    #[test]
    fn lookup_past_last_segment_picks_the_last() {
        let lines = sample_lines();
        assert_eq!(lookup(&lines, 0, 100), Some(sp(1, 3, 4)));
    }

    #[test]
    fn lookup_sourceless_segment_is_none() {
        let lines = vec![vec![
            Segment {
                column: 0,
                source: Some(sp(0, 0, 0)),
            },
            Segment {
                column: 5,
                source: None,
            },
        ]];
        assert_eq!(lookup(&lines, 0, 6), None);
    }

    #[test]
    fn lookup_line_out_of_range_is_none() {
        let lines = sample_lines();
        assert_eq!(lookup(&lines, 1, 0), None);
    }

    /// One line with segments at columns 0 and 5, both with sources.
    fn sample_lines() -> Vec<Vec<Segment>> {
        vec![vec![
            Segment {
                column: 0,
                source: Some(sp(0, 1, 2)),
            },
            Segment {
                column: 5,
                source: Some(sp(1, 3, 4)),
            },
        ]]
    }
}
