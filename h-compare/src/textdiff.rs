//! Line and intra-line text diffing with navigation and search support.

use crate::diff_common::BlockSpan;
use crate::model::CompareOptions;
use similar::{capture_diff_slices, Algorithm, ChangeTag, DiffOp, TextDiff};
use std::ops::Range;

pub const MAX_INTRALINE_CHARS: usize = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Equal,
    Modified,
    LeftOnly,
    RightOnly,
}

#[derive(Debug, Clone)]
pub struct DiffRow {
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub kind: RowKind,
    /// Character-index ranges for intra-line highlighting.
    pub left_spans: Vec<Range<usize>>,
    pub right_spans: Vec<Range<usize>>,
    /// One entry per contiguous intra-line change group.
    pub group_left: Vec<Option<Range<usize>>>,
    pub group_right: Vec<Option<Range<usize>>>,
    pub change_groups: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextStats {
    pub left_lines: usize,
    pub right_lines: usize,
    pub unchanged: usize,
    pub modified: usize,
    pub added: usize,
    pub removed: usize,
    pub hunks: usize,
}

impl TextStats {
    #[allow(dead_code)]
    pub fn changes(&self) -> usize {
        self.modified + self.added + self.removed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    None,
    Lf,
    Crlf,
    Cr,
    Mixed,
}

impl LineEnding {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "no newline",
            Self::Lf => "LF",
            Self::Crlf => "CRLF",
            Self::Cr => "CR",
            Self::Mixed => "mixed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecodedText {
    pub text: String,
    pub encoding: &'static str,
    pub had_bom: bool,
}

#[derive(Debug, Clone)]
pub struct TextDiffView {
    pub left_lines: Vec<String>,
    pub right_lines: Vec<String>,
    pub rows: Vec<DiffRow>,
    pub stats: TextStats,
    pub change_rows: Vec<usize>,
    pub left_encoding: &'static str,
    pub right_encoding: &'static str,
    pub left_bom: bool,
    pub right_bom: bool,
    pub left_line_ending: LineEnding,
    pub right_line_ending: LineEnding,
    pub left_bytes: usize,
    pub right_bytes: usize,
}

impl TextDiffView {
    pub fn left_line(&self, idx: usize) -> &str {
        self.left_lines.get(idx).map(String::as_str).unwrap_or("")
    }

    pub fn right_line(&self, idx: usize) -> &str {
        self.right_lines.get(idx).map(String::as_str).unwrap_or("")
    }

    pub fn next_change(&self, current_row: Option<usize>) -> Option<usize> {
        match current_row {
            Some(row) => self.change_rows.iter().copied().find(|idx| *idx > row),
            None => self.change_rows.first().copied(),
        }
    }

    pub fn previous_change(&self, current_row: Option<usize>) -> Option<usize> {
        match current_row {
            Some(row) => self
                .change_rows
                .iter()
                .rev()
                .copied()
                .find(|idx| *idx < row),
            None => self.change_rows.last().copied(),
        }
    }

    pub fn count_matches(&self, needle: &str, case_sensitive: bool) -> usize {
        if needle.is_empty() {
            return 0;
        }
        self.left_lines
            .iter()
            .chain(self.right_lines.iter())
            .map(|line| find_ranges(line, needle, case_sensitive).len())
            .sum()
    }

    /// Rows containing at least one search hit, in display order.
    pub fn matching_rows(&self, needle: &str, case_sensitive: bool) -> Vec<usize> {
        if needle.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (row_idx, row) in self.rows.iter().enumerate() {
            let left_hit = row
                .left
                .map(|line| !find_ranges(self.left_line(line), needle, case_sensitive).is_empty())
                .unwrap_or(false);
            let right_hit = row
                .right
                .map(|line| !find_ranges(self.right_line(line), needle, case_sensitive).is_empty())
                .unwrap_or(false);
            if left_hit || right_hit {
                out.push(row_idx);
            }
        }
        out
    }
}

/// Build a side-by-side line diff. The original lines are retained for display;
/// the comparison uses normalized copies based on `options`.
pub fn build_text_diff(
    left_text: &str,
    right_text: &str,
    options: &CompareOptions,
) -> TextDiffView {
    let left_raw = split_lines(left_text);
    let right_raw = split_lines(right_text);
    let left_norm: Vec<String> = left_raw
        .iter()
        .map(|line| normalize_line(line, options))
        .collect();
    let right_norm: Vec<String> = right_raw
        .iter()
        .map(|line| normalize_line(line, options))
        .collect();

    let ops = capture_diff_slices(Algorithm::Myers, &left_norm, &right_norm);
    let mut rows = Vec::new();

    let mut i = 0;
    while i < ops.len() {
        match ops[i] {
            DiffOp::Equal {
                old_index,
                new_index,
                len,
            } => {
                for offset in 0..len {
                    rows.push(DiffRow {
                        left: Some(old_index + offset),
                        right: Some(new_index + offset),
                        kind: RowKind::Equal,
                        left_spans: Vec::new(),
                        right_spans: Vec::new(),
                        group_left: Vec::new(),
                        group_right: Vec::new(),
                        change_groups: 0,
                    });
                }
                i += 1;
            }
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                push_pair_block(
                    &mut rows,
                    &left_norm,
                    &right_norm,
                    BlockSpan {
                        old_index,
                        old_len,
                        new_index,
                        new_len,
                    },
                );
                i += 1;
            }
            DiffOp::Delete {
                old_index,
                old_len,
                new_index,
            } => {
                if let Some(DiffOp::Insert {
                    new_index: insert_index,
                    new_len,
                    ..
                }) = ops.get(i + 1).copied()
                {
                    push_pair_block(
                        &mut rows,
                        &left_norm,
                        &right_norm,
                        BlockSpan {
                            old_index,
                            old_len,
                            new_index: insert_index,
                            new_len,
                        },
                    );
                    i += 2;
                } else {
                    for offset in 0..old_len {
                        rows.push(DiffRow {
                            left: Some(old_index + offset),
                            right: None,
                            kind: RowKind::LeftOnly,
                            left_spans: Vec::new(),
                            right_spans: Vec::new(),
                            group_left: Vec::new(),
                            group_right: Vec::new(),
                            change_groups: 1,
                        });
                    }
                    let _ = new_index;
                    i += 1;
                }
            }
            DiffOp::Insert {
                new_index, new_len, ..
            } => {
                for offset in 0..new_len {
                    rows.push(DiffRow {
                        left: None,
                        right: Some(new_index + offset),
                        kind: RowKind::RightOnly,
                        left_spans: Vec::new(),
                        right_spans: Vec::new(),
                        group_left: Vec::new(),
                        group_right: Vec::new(),
                        change_groups: 1,
                    });
                }
                i += 1;
            }
        }
    }

    let mut stats = TextStats {
        left_lines: left_raw.len(),
        right_lines: right_raw.len(),
        ..Default::default()
    };
    let mut change_rows = Vec::new();
    let mut in_hunk = false;
    for (idx, row) in rows.iter().enumerate() {
        match row.kind {
            RowKind::Equal => {
                stats.unchanged += 1;
                in_hunk = false;
            }
            RowKind::Modified => {
                stats.modified += 1;
                change_rows.push(idx);
                if !in_hunk {
                    stats.hunks += 1;
                    in_hunk = true;
                }
            }
            RowKind::LeftOnly => {
                stats.removed += 1;
                change_rows.push(idx);
                if !in_hunk {
                    stats.hunks += 1;
                    in_hunk = true;
                }
            }
            RowKind::RightOnly => {
                stats.added += 1;
                change_rows.push(idx);
                if !in_hunk {
                    stats.hunks += 1;
                    in_hunk = true;
                }
            }
        }
    }

    TextDiffView {
        left_lines: left_raw.into_iter().map(str::to_string).collect(),
        right_lines: right_raw.into_iter().map(str::to_string).collect(),
        rows,
        stats,
        change_rows,
        left_encoding: "UTF-8",
        right_encoding: "UTF-8",
        left_bom: false,
        right_bom: false,
        left_line_ending: detect_line_ending(left_text),
        right_line_ending: detect_line_ending(right_text),
        left_bytes: left_text.len(),
        right_bytes: right_text.len(),
    }
}

fn push_pair_block(
    rows: &mut Vec<DiffRow>,
    left_norm: &[String],
    right_norm: &[String],
    span: BlockSpan,
) {
    let BlockSpan {
        old_index,
        old_len,
        new_index,
        new_len,
    } = span;
    let common = old_len.min(new_len);
    for offset in 0..common {
        let left = old_index + offset;
        let right = new_index + offset;
        let kind = if left_norm[left] == right_norm[right] {
            RowKind::Equal
        } else {
            RowKind::Modified
        };
        let (left_spans, right_spans, group_left, group_right, change_groups) =
            if kind == RowKind::Modified {
                let groups = intraline_groups(&left_norm[left], &right_norm[right]);
                (
                    groups.left_spans,
                    groups.right_spans,
                    groups.group_left,
                    groups.group_right,
                    groups.change_groups,
                )
            } else {
                (Vec::new(), Vec::new(), Vec::new(), Vec::new(), 0)
            };
        rows.push(DiffRow {
            left: Some(left),
            right: Some(right),
            kind,
            left_spans,
            right_spans,
            group_left,
            group_right,
            change_groups,
        });
    }
    for offset in common..old_len {
        rows.push(DiffRow {
            left: Some(old_index + offset),
            right: None,
            kind: RowKind::LeftOnly,
            left_spans: Vec::new(),
            right_spans: Vec::new(),
            group_left: Vec::new(),
            group_right: Vec::new(),
            change_groups: 1,
        });
    }
    for offset in common..new_len {
        rows.push(DiffRow {
            left: None,
            right: Some(new_index + offset),
            kind: RowKind::RightOnly,
            left_spans: Vec::new(),
            right_spans: Vec::new(),
            group_left: Vec::new(),
            group_right: Vec::new(),
            change_groups: 1,
        });
    }
}

fn normalize_line(line: &str, options: &CompareOptions) -> String {
    let mut out = if options.ignore_whitespace {
        line.split_whitespace().collect::<Vec<_>>().join(" ")
    } else if options.trim_trailing {
        line.trim_end().to_string()
    } else {
        line.to_string()
    };
    if options.ignore_case {
        out = out.to_lowercase();
    }
    out
}

/// Character ranges that differ. Returns empty ranges when either line is too
/// long to be worth highlighting.
#[derive(Debug, Clone, Default)]
pub(crate) struct IntralineGroups {
    pub left_spans: Vec<Range<usize>>,
    pub right_spans: Vec<Range<usize>>,
    pub group_left: Vec<Option<Range<usize>>>,
    pub group_right: Vec<Option<Range<usize>>>,
    pub change_groups: usize,
}

pub(crate) fn intraline_groups(left: &str, right: &str) -> IntralineGroups {
    let left_len = left.chars().count();
    let right_len = right.chars().count();
    if left_len > MAX_INTRALINE_CHARS || right_len > MAX_INTRALINE_CHARS {
        return IntralineGroups {
            left_spans: Vec::new(),
            right_spans: Vec::new(),
            group_left: vec![None],
            group_right: vec![None],
            change_groups: 1,
        };
    }
    let diff = TextDiff::from_chars(left, right);
    let mut group_left: Vec<Option<Range<usize>>> = Vec::new();
    let mut group_right: Vec<Option<Range<usize>>> = Vec::new();
    let mut current_left: Option<Range<usize>> = None;
    let mut current_right: Option<Range<usize>> = None;

    let push_group = |group_left: &mut Vec<Option<Range<usize>>>,
                      group_right: &mut Vec<Option<Range<usize>>>,
                      current_left: &mut Option<Range<usize>>,
                      current_right: &mut Option<Range<usize>>| {
        if current_left.is_some() || current_right.is_some() {
            group_left.push(current_left.take());
            group_right.push(current_right.take());
        }
    };

    for change in diff.iter_all_changes() {
        let len = change.value().chars().count();
        match change.tag() {
            ChangeTag::Equal => {
                push_group(
                    &mut group_left,
                    &mut group_right,
                    &mut current_left,
                    &mut current_right,
                );
            }
            ChangeTag::Delete => {
                if let Some(start) = change.old_index() {
                    extend_char_range(&mut current_left, start, len);
                }
            }
            ChangeTag::Insert => {
                if let Some(start) = change.new_index() {
                    extend_char_range(&mut current_right, start, len);
                }
            }
        }
    }
    push_group(
        &mut group_left,
        &mut group_right,
        &mut current_left,
        &mut current_right,
    );

    if group_left.is_empty() {
        group_left.push(None);
        group_right.push(None);
    }
    let left_spans = group_left.iter().flatten().cloned().collect();
    let right_spans = group_right.iter().flatten().cloned().collect();
    let change_groups = group_left.len();
    IntralineGroups {
        left_spans,
        right_spans,
        group_left,
        group_right,
        change_groups,
    }
}

fn extend_char_range(range: &mut Option<Range<usize>>, start: usize, len: usize) {
    let end = start + len;
    match range {
        Some(range) => range.end = range.end.max(end),
        None => *range = Some(start..end),
    }
}

pub(crate) fn intraline_spans(left: &str, right: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    let groups = intraline_groups(left, right);
    (groups.left_spans, groups.right_spans)
}

pub fn split_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    let separator = if text.contains('\n') { '\n' } else { '\r' };
    let mut lines: Vec<&str> = text.split(separator).collect();
    if text.ends_with(separator) {
        lines.pop();
    }
    if separator == '\n' {
        lines
            .into_iter()
            .map(|line| line.strip_suffix('\r').unwrap_or(line))
            .collect()
    } else {
        lines
    }
}

pub fn detect_line_ending(text: &str) -> LineEnding {
    let mut lf = 0usize;
    let mut crlf = 0usize;
    let mut cr = 0usize;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' => {
                if bytes.get(i + 1) == Some(&b'\n') {
                    crlf += 1;
                    i += 1;
                } else {
                    cr += 1;
                }
            }
            b'\n' => lf += 1,
            _ => {}
        }
        i += 1;
    }
    match (lf, crlf, cr) {
        (0, 0, 0) => LineEnding::None,
        (_, 0, 0) if lf > 0 => LineEnding::Lf,
        (0, _, 0) if crlf > 0 => LineEnding::Crlf,
        (0, 0, _) if cr > 0 => LineEnding::Cr,
        _ => LineEnding::Mixed,
    }
}

pub fn decode_text(bytes: &[u8]) -> DecodedText {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return DecodedText {
            text: String::from_utf8_lossy(rest).into_owned(),
            encoding: "UTF-8",
            had_bom: true,
        };
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let units: Vec<u16> = rest
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        return DecodedText {
            text: String::from_utf16_lossy(&units),
            encoding: "UTF-16 LE",
            had_bom: true,
        };
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        let units: Vec<u16> = rest
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect();
        return DecodedText {
            text: String::from_utf16_lossy(&units),
            encoding: "UTF-16 BE",
            had_bom: true,
        };
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => DecodedText {
            text: text.to_string(),
            encoding: "UTF-8",
            had_bom: false,
        },
        Err(_) => DecodedText {
            text: String::from_utf8_lossy(bytes).into_owned(),
            encoding: "UTF-8 (lossy)",
            had_bom: false,
        },
    }
}

pub fn looks_binary(bytes: &[u8]) -> bool {
    // UTF-16 files legitimately contain NUL bytes.
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return false;
    }
    bytes.iter().take(8192).any(|byte| *byte == 0)
}

/// Byte ranges of `needle` inside `haystack`.
pub fn find_ranges(haystack: &str, needle: &str, case_sensitive: bool) -> Vec<Range<usize>> {
    if needle.is_empty() || haystack.is_empty() {
        return Vec::new();
    }
    if case_sensitive {
        return haystack
            .match_indices(needle)
            .map(|(start, found)| start..start + found.len())
            .collect();
    }
    let haystack_lower = haystack.to_lowercase();
    let needle_lower = needle.to_lowercase();
    if needle_lower.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (start, _) in haystack_lower.match_indices(&needle_lower) {
        let end = start + needle_lower.len();
        if haystack.is_char_boundary(start) && haystack.is_char_boundary(end) {
            out.push(start..end);
        }
    }
    out
}

/// Convert a character-index range into a byte range suitable for `&str`.
pub fn char_range_to_bytes(text: &str, range: Range<usize>) -> Option<Range<usize>> {
    let mut chars = text.char_indices().map(|(idx, _)| idx).collect::<Vec<_>>();
    chars.push(text.len());
    if range.start <= range.end && range.end < chars.len() {
        Some(chars[range.start]..chars[range.end])
    } else {
        None
    }
}

pub fn unified_diff(left: &str, right: &str, left_name: &str, right_name: &str) -> String {
    TextDiff::from_lines(left, right)
        .unified_diff()
        .context_radius(3)
        .header(left_name, right_name)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> CompareOptions {
        CompareOptions::default()
    }

    #[test]
    fn intraline_group_sample() {
        let left = [
            r#"    <textarea name="query" klk cols="30" rows="8" id="query-builder" spellcheck="false" class="u-full-width" maxlength="1500" required>Return on invested capital &gt; 25 lkld and"#,
            r#"    <p id="query-help-description" style="white-space: pre-line">Market capitalization &gt; 500 AND djjfk"#,
            r#"        <a href="/guides/terms/" class="underline-link">Terms</a> & <a href="/guides/privacy/" class="underline-link">Privacy</a>."#,
        ];
        let right = [
            r#"    <textkdjsarea name="query" cols="30" rows="8" id="query-builder" spellcheck="false" class="u-full-width" maxlength="1500" required>Return jjhj on invested capital &gt; 25 and"#,
            r#"    <p id="query-help-description" style="white-spackjde: pre-line">Market capitalization &gt; 500 AND"#,
            r#"        <a href="/guides/terms/" class="underline-ljuhink">Terms</a> & <a href="/guides/privacy/" class="underline-link">Privacy</a>.kj"#,
        ];
        let expected = [4, 2, 2];
        for ((l, r), want) in left.iter().zip(right.iter()).zip(expected) {
            let groups = intraline_groups(l, r);
            assert_eq!(groups.change_groups, want, "groups for {l}");
        }
    }

    #[test]
    fn identical_text_has_no_changes() {
        let view = build_text_diff("a\nb\nc\n", "a\nb\nc\n", &opts());
        assert_eq!(view.rows.len(), 3);
        assert!(view.change_rows.is_empty());
        assert_eq!(view.stats.unchanged, 3);
        assert_eq!(view.left_line_ending, LineEnding::Lf);
    }

    #[test]
    fn pairs_modified_lines_and_produces_spans() {
        let view = build_text_diff("alpha\nbeta\ngamma\n", "alpha\nBETA\ngamma\n", &opts());
        assert_eq!(view.rows.len(), 3);
        assert_eq!(view.rows[1].kind, RowKind::Modified);
        assert_eq!(view.stats.modified, 1);
        assert_eq!(view.stats.hunks, 1);
        assert!(!view.rows[1].left_spans.is_empty());
        assert!(!view.rows[1].right_spans.is_empty());
    }

    #[test]
    fn detects_one_sided_lines() {
        let view = build_text_diff("a\nb\n", "a\nx\nb\n", &opts());
        assert_eq!(view.stats.added, 1);
        assert_eq!(view.stats.removed, 0);
        assert_eq!(view.stats.changes(), 1);
        assert_eq!(view.rows[1].kind, RowKind::RightOnly);
    }

    #[test]
    fn ignores_whitespace_and_case() {
        let mut options = opts();
        options.ignore_whitespace = true;
        options.ignore_case = true;
        let view = build_text_diff("Hello   World\n", "  hello world  \n", &options);
        assert_eq!(view.stats.modified, 0);
        assert_eq!(view.stats.unchanged, 1);
        assert!(view.change_rows.is_empty());
    }

    #[test]
    fn trims_trailing_whitespace() {
        let mut options = opts();
        options.trim_trailing = true;
        let view = build_text_diff("hello   \n", "hello\n", &options);
        assert_eq!(view.stats.modified, 0);
    }

    #[test]
    fn crlf_is_normalized_for_display_and_diff() {
        let view = build_text_diff("a\r\nb\r\n", "a\nb\n", &opts());
        assert_eq!(view.stats.modified, 0);
        assert_eq!(view.left_line_ending, LineEnding::Crlf);
        assert_eq!(view.right_line_ending, LineEnding::Lf);
        assert_eq!(view.left_lines[0], "a");
    }

    #[test]
    fn trailing_newline_does_not_create_a_phantom_line() {
        assert_eq!(split_lines("a\nb\n"), vec!["a", "b"]);
        assert_eq!(split_lines("a\nb"), vec!["a", "b"]);
        assert_eq!(split_lines("a\rb"), vec!["a", "b"]);
        assert!(split_lines("").is_empty());
    }

    #[test]
    fn decodes_utf16_with_bom() {
        let text = "hello\n";
        let mut bytes = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let decoded = decode_text(&bytes);
        assert_eq!(decoded.text, text);
        assert_eq!(decoded.encoding, "UTF-16 LE");
    }

    #[test]
    fn finds_ranges_case_insensitively() {
        let ranges = find_ranges("Hello hello", "HELLO", false);
        assert_eq!(ranges, vec![0..5, 6..11]);
        let ranges = find_ranges("Hello hello", "hello", true);
        assert_eq!(ranges, vec![6..11]);
    }

    #[test]
    fn converts_char_ranges_to_byte_ranges() {
        let text = "aé中z";
        let range = char_range_to_bytes(text, 1..3).unwrap();
        assert_eq!(&text[range], "é中");
    }

    #[test]
    fn change_navigation_wraps_in_order() {
        let view = build_text_diff("a\nb\nc\n", "a\nx\nc\ny\n", &opts());
        assert_eq!(view.next_change(None), Some(1));
        assert_eq!(view.next_change(Some(1)), Some(3));
        assert_eq!(view.previous_change(Some(3)), Some(1));
        assert_eq!(view.previous_change(None), Some(3));
    }
}
