//! CSV parsing, alignment, cell diffing and column rewrite helpers.

use crate::diff_common::BlockSpan;
use crate::textdiff::{decode_text, intraline_spans, looks_binary};
use serde::{Deserialize, Serialize};
use similar::{capture_diff_slices, Algorithm, DiffOp};
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DelimiterChoice {
    #[default]
    Auto,
    Comma,
    Semicolon,
    Tab,
    Pipe,
}

impl DelimiterChoice {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto detect",
            Self::Comma => "Comma (,)",
            Self::Semicolon => "Semicolon (;)",
            Self::Tab => "Tab",
            Self::Pipe => "Pipe (|)",
        }
    }

    pub fn byte(self) -> Option<u8> {
        match self {
            Self::Auto => None,
            Self::Comma => Some(b','),
            Self::Semicolon => Some(b';'),
            Self::Tab => Some(b'\t'),
            Self::Pipe => Some(b'|'),
        }
    }

    pub const ALL: [Self; 5] = [
        Self::Auto,
        Self::Comma,
        Self::Semicolon,
        Self::Tab,
        Self::Pipe,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CsvAlign {
    /// Use the first column as a key when it is unique in both files,
    /// otherwise fall back to row order.
    #[default]
    Auto,
    RowOrder,
    Key(usize),
}

impl CsvAlign {
    pub fn short_label(self) -> String {
        match self {
            Self::Auto => "Auto key".to_string(),
            Self::RowOrder => "Row order".to_string(),
            Self::Key(key) => format!("Key col {}", key + 1),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsvOptions {
    pub delimiter: DelimiterChoice,
    pub has_headers: bool,
    pub align: CsvAlign,
    pub ignore_case: bool,
    pub trim_whitespace: bool,
    pub ignore_empty_rows: bool,
}

impl Default for CsvOptions {
    fn default() -> Self {
        Self {
            delimiter: DelimiterChoice::Auto,
            has_headers: true,
            align: CsvAlign::Auto,
            ignore_case: false,
            trim_whitespace: true,
            ignore_empty_rows: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CsvTable {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub source_rows: Vec<usize>,
    pub has_headers: bool,
    pub delimiter: u8,
    pub column_count: usize,
    pub encoding: &'static str,
}

impl CsvTable {
    pub fn cell(&self, row: usize, col: usize) -> &str {
        self.rows
            .get(row)
            .and_then(|row| row.get(col))
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn header(&self, col: usize) -> &str {
        self.headers.get(col).map(String::as_str).unwrap_or("")
    }

    pub fn display_header(&self, col: usize) -> String {
        let header = self.header(col).trim();
        if header.is_empty() {
            format!("Column {}", col + 1)
        } else {
            header.to_string()
        }
    }

    pub fn row_number(&self, row: usize) -> usize {
        self.source_rows.get(row).copied().unwrap_or(row + 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsvRowKind {
    Equal,
    Modified,
    LeftOnly,
    RightOnly,
}

impl CsvRowKind {
    pub fn is_change(self) -> bool {
        !matches!(self, Self::Equal)
    }
}

#[derive(Debug, Clone)]
pub struct CellDiff {
    pub col: usize,
    pub left: String,
    pub right: String,
    pub left_spans: Vec<Range<usize>>,
    pub right_spans: Vec<Range<usize>>,
}

#[derive(Debug, Clone)]
pub struct CsvRow {
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub kind: CsvRowKind,
    pub changed_cols: Vec<usize>,
    pub cells: Vec<CellDiff>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CsvStats {
    pub left_rows: usize,
    pub right_rows: usize,
    pub equal: usize,
    pub modified: usize,
    pub added: usize,
    pub removed: usize,
    pub changed_cells: usize,
    pub header_changes: usize,
}

impl CsvStats {
    #[allow(dead_code)]
    pub fn changes(&self) -> usize {
        self.modified + self.added + self.removed + self.header_changes
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CsvIgnore {
    pub left: BTreeSet<usize>,
    pub right: BTreeSet<usize>,
}

impl CsvIgnore {
    pub fn ignores_column(&self, col: usize) -> bool {
        self.left.contains(&col) || self.right.contains(&col)
    }
}

#[derive(Debug, Clone)]
pub struct CsvCompare {
    pub left: CsvTable,
    pub right: CsvTable,
    pub rows: Vec<CsvRow>,
    pub stats: CsvStats,
    pub changed_headers: Vec<usize>,
    pub column_count: usize,
    pub align: CsvAlign,
    pub warnings: Vec<String>,
}

impl CsvCompare {
    pub fn column_name(&self, col: usize) -> String {
        let left = self.left.display_header(col);
        let right = self.right.display_header(col);
        if normalize_text(&left, false, true) == normalize_text(&right, false, true) {
            left
        } else {
            format!("{left}  /  {right}")
        }
    }

    pub fn row_change_count(&self) -> usize {
        self.stats.modified + self.stats.added + self.stats.removed
    }

    pub fn next_change(&self, current: Option<usize>) -> Option<usize> {
        let start = current.map(|row| row + 1).unwrap_or(0);
        (start..self.rows.len()).find(|idx| self.rows[*idx].kind.is_change())
    }

    pub fn previous_change(&self, current: Option<usize>) -> Option<usize> {
        let mut idx = current.unwrap_or(self.rows.len()).min(self.rows.len());
        while idx > 0 {
            idx -= 1;
            if self.rows[idx].kind.is_change() {
                return Some(idx);
            }
        }
        None
    }

    pub fn cell_diff(&self, row: usize, col: usize) -> Option<&CellDiff> {
        self.rows
            .get(row)?
            .cells
            .iter()
            .find(|cell| cell.col == col)
    }
}

pub fn parse_csv(bytes: &[u8], options: &CsvOptions) -> Result<CsvTable, String> {
    if looks_binary(bytes) {
        return Err("File appears to be binary, not CSV".to_string());
    }
    let decoded = decode_text(bytes);
    let delimiter = options
        .delimiter
        .byte()
        .unwrap_or_else(|| detect_delimiter(&decoded.text));
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(decoded.text.as_bytes());

    let mut records: Vec<Vec<String>> = Vec::new();
    for (idx, result) in reader.records().enumerate() {
        let record =
            result.map_err(|err| format!("CSV parse error near record {}: {err}", idx + 1))?;
        let mut fields: Vec<String> = record.iter().map(str::to_string).collect();
        if idx == 0 {
            if let Some(first) = fields.first_mut() {
                if let Some(stripped) = first.strip_prefix('\u{feff}') {
                    *first = stripped.to_string();
                }
            }
        }
        if options.ignore_empty_rows && fields.iter().all(|field| field.trim().is_empty()) {
            continue;
        }
        records.push(fields);
    }

    let has_headers = options.has_headers && !records.is_empty();
    let headers = if has_headers {
        records.remove(0)
    } else {
        Vec::new()
    };
    let column_count = records
        .iter()
        .map(Vec::len)
        .chain(std::iter::once(headers.len()))
        .max()
        .unwrap_or(0);
    let mut padded_headers = headers;
    padded_headers.resize(column_count, String::new());
    for row in &mut records {
        row.resize(column_count, String::new());
    }
    let source_rows = (0..records.len())
        .map(|idx| idx + 1 + usize::from(has_headers))
        .collect();

    Ok(CsvTable {
        headers: padded_headers,
        rows: records,
        source_rows,
        has_headers,
        delimiter,
        column_count,
        encoding: decoded.encoding,
    })
}

pub fn compare_csv(
    left_bytes: &[u8],
    right_bytes: &[u8],
    options: &CsvOptions,
) -> Result<CsvCompare, String> {
    let left = parse_csv(left_bytes, options)?;
    let right = parse_csv(right_bytes, options)?;
    Ok(compare_tables(left, right, options))
}

#[allow(dead_code)]
pub fn compare_csv_with_ignore(
    left_bytes: &[u8],
    right_bytes: &[u8],
    options: &CsvOptions,
    ignore: &CsvIgnore,
) -> Result<CsvCompare, String> {
    let left = parse_csv(left_bytes, options)?;
    let right = parse_csv(right_bytes, options)?;
    Ok(compare_tables_with_ignore(left, right, options, ignore))
}

pub fn compare_tables(left: CsvTable, right: CsvTable, options: &CsvOptions) -> CsvCompare {
    compare_tables_with_ignore(left, right, options, &CsvIgnore::default())
}

pub fn compare_tables_with_ignore(
    left: CsvTable,
    right: CsvTable,
    options: &CsvOptions,
    ignore: &CsvIgnore,
) -> CsvCompare {
    let column_count = left.column_count.max(right.column_count);
    let mut warnings = Vec::new();
    let left_active = (0..left.column_count)
        .filter(|col| !ignore.ignores_column(*col))
        .count();
    let right_active = (0..right.column_count)
        .filter(|col| !ignore.ignores_column(*col))
        .count();
    if left_active != right_active {
        warnings.push(format!(
            "Active column counts differ: left has {left_active}, right has {right_active}."
        ));
    }

    let mut changed_headers = Vec::new();
    if (left.has_headers || right.has_headers) && left.column_count > 0 && right.column_count > 0 {
        for col in 0..column_count {
            if ignore.ignores_column(col) {
                continue;
            }
            let left_header = normalize_text(
                left.header(col),
                options.ignore_case,
                options.trim_whitespace,
            );
            let right_header = normalize_text(
                right.header(col),
                options.ignore_case,
                options.trim_whitespace,
            );
            if left_header != right_header {
                changed_headers.push(col);
            }
        }
    }

    let mut chosen_rows: Option<Vec<CsvRow>> = None;
    let align = match options.align {
        CsvAlign::Auto => {
            if let Some(key) = auto_key_column(&left, &right, column_count, options, ignore) {
                let key_rows = align_by_key(&left, &right, column_count, key, options, ignore);
                let key_one_sided = one_sided_count(&key_rows);
                if key_one_sided == 0 {
                    chosen_rows = Some(key_rows);
                    CsvAlign::Key(key)
                } else {
                    // Key alignment can turn a changed key into an added + removed
                    // pair. When the files have the same row count and only a small
                    // minority of rows fail to match, positional alignment is
                    // usually the truthful view (one modified row, not two).
                    let same_len = left.rows.len() == right.rows.len();
                    let limited = key_one_sided * 2 < left.rows.len();
                    if same_len && limited {
                        let row_rows =
                            align_by_row_order(&left, &right, column_count, options, ignore);
                        let row_one_sided = one_sided_count(&row_rows);
                        if row_one_sided < key_one_sided {
                            chosen_rows = Some(row_rows);
                            CsvAlign::RowOrder
                        } else {
                            chosen_rows = Some(key_rows);
                            CsvAlign::Key(key)
                        }
                    } else {
                        chosen_rows = Some(key_rows);
                        CsvAlign::Key(key)
                    }
                }
            } else {
                CsvAlign::RowOrder
            }
        }
        CsvAlign::Key(key) if key < column_count => CsvAlign::Key(key),
        CsvAlign::Key(key) => {
            warnings.push(format!(
                "Key column {} is out of range; falling back to row order.",
                key + 1
            ));
            CsvAlign::RowOrder
        }
        other => other,
    };

    let rows = chosen_rows.unwrap_or_else(|| match align {
        CsvAlign::RowOrder | CsvAlign::Auto => {
            align_by_row_order(&left, &right, column_count, options, ignore)
        }
        CsvAlign::Key(key) => align_by_key(&left, &right, column_count, key, options, ignore),
    });

    let mut stats = CsvStats {
        left_rows: left.rows.len(),
        right_rows: right.rows.len(),
        header_changes: changed_headers.len(),
        ..Default::default()
    };
    for row in &rows {
        match row.kind {
            CsvRowKind::Equal => stats.equal += 1,
            CsvRowKind::Modified => {
                stats.modified += 1;
                stats.changed_cells += row.changed_cols.len();
            }
            CsvRowKind::LeftOnly => stats.removed += 1,
            CsvRowKind::RightOnly => stats.added += 1,
        }
    }

    CsvCompare {
        left,
        right,
        rows,
        stats,
        changed_headers,
        column_count,
        align,
        warnings,
    }
}

fn align_by_row_order(
    left: &CsvTable,
    right: &CsvTable,
    column_count: usize,
    options: &CsvOptions,
    ignore: &CsvIgnore,
) -> Vec<CsvRow> {
    let left_serialized = serialize_rows(left, column_count, options, ignore);
    let right_serialized = serialize_rows(right, column_count, options, ignore);
    let ops = capture_diff_slices(Algorithm::Myers, &left_serialized, &right_serialized);
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
                    rows.push(compare_row(
                        left,
                        right,
                        old_index + offset,
                        new_index + offset,
                        column_count,
                        options,
                        ignore,
                    ));
                }
                i += 1;
            }
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                push_row_block(
                    &mut rows,
                    RowBlock {
                        left,
                        right,
                        column_count,
                        options,
                        ignore,
                        span: BlockSpan {
                            old_index,
                            old_len,
                            new_index,
                            new_len,
                        },
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
                    push_row_block(
                        &mut rows,
                        RowBlock {
                            left,
                            right,
                            column_count,
                            options,
                            ignore,
                            span: BlockSpan {
                                old_index,
                                old_len,
                                new_index: insert_index,
                                new_len,
                            },
                        },
                    );
                    i += 2;
                } else {
                    for offset in 0..old_len {
                        rows.push(one_sided_row(Some(old_index + offset), None));
                    }
                    let _ = new_index;
                    i += 1;
                }
            }
            DiffOp::Insert {
                new_index, new_len, ..
            } => {
                for offset in 0..new_len {
                    rows.push(one_sided_row(None, Some(new_index + offset)));
                }
                i += 1;
            }
        }
    }
    rows
}

struct RowBlock<'a> {
    left: &'a CsvTable,
    right: &'a CsvTable,
    column_count: usize,
    options: &'a CsvOptions,
    ignore: &'a CsvIgnore,
    span: BlockSpan,
}

fn push_row_block(rows: &mut Vec<CsvRow>, block: RowBlock<'_>) {
    let left = block.left;
    let right = block.right;
    let column_count = block.column_count;
    let options = block.options;
    let ignore = block.ignore;
    let BlockSpan {
        old_index,
        old_len,
        new_index,
        new_len,
    } = block.span;
    let common = old_len.min(new_len);
    for offset in 0..common {
        rows.push(compare_row(
            left,
            right,
            old_index + offset,
            new_index + offset,
            column_count,
            options,
            ignore,
        ));
    }
    for offset in common..old_len {
        rows.push(one_sided_row(Some(old_index + offset), None));
    }
    for offset in common..new_len {
        rows.push(one_sided_row(None, Some(new_index + offset)));
    }
}

fn align_by_key(
    left: &CsvTable,
    right: &CsvTable,
    column_count: usize,
    key: usize,
    options: &CsvOptions,
    ignore: &CsvIgnore,
) -> Vec<CsvRow> {
    let mut right_by_key: HashMap<String, VecDeque<usize>> = HashMap::new();
    for (idx, row) in right.rows.iter().enumerate() {
        let key_value = normalize_text(
            row.get(key).map(String::as_str).unwrap_or(""),
            options.ignore_case,
            options.trim_whitespace,
        );
        right_by_key.entry(key_value).or_default().push_back(idx);
    }

    let mut used_right = vec![false; right.rows.len()];
    let mut rows = Vec::new();
    for (left_idx, left_row) in left.rows.iter().enumerate() {
        let key_value = normalize_text(
            left_row.get(key).map(String::as_str).unwrap_or(""),
            options.ignore_case,
            options.trim_whitespace,
        );
        let matched = right_by_key
            .get_mut(&key_value)
            .and_then(|queue| queue.pop_front());
        if let Some(right_idx) = matched {
            used_right[right_idx] = true;
            rows.push(compare_row(
                left,
                right,
                left_idx,
                right_idx,
                column_count,
                options,
                ignore,
            ));
        } else {
            rows.push(one_sided_row(Some(left_idx), None));
        }
    }
    for (right_idx, used) in used_right.iter().copied().enumerate() {
        if !used {
            rows.push(one_sided_row(None, Some(right_idx)));
        }
    }
    rows
}

fn auto_key_column(
    left: &CsvTable,
    right: &CsvTable,
    column_count: usize,
    options: &CsvOptions,
    ignore: &CsvIgnore,
) -> Option<usize> {
    let col = (0..column_count).find(|col| !ignore.ignores_column(*col))?;
    let left_keys = column_key_set(left, col, options)?;
    let right_keys = column_key_set(right, col, options)?;
    let shared = left_keys.intersection(&right_keys).count();
    let max_rows = left.rows.len().max(right.rows.len());
    if shared > 0 && shared * 2 >= max_rows {
        Some(col)
    } else {
        None
    }
}

fn column_key_set(
    table: &CsvTable,
    col: usize,
    options: &CsvOptions,
) -> Option<std::collections::HashSet<String>> {
    if table.rows.is_empty() {
        return None;
    }
    let mut seen = std::collections::HashSet::with_capacity(table.rows.len());
    for row in &table.rows {
        let value = normalize_text(
            row.get(col).map(String::as_str).unwrap_or(""),
            options.ignore_case,
            options.trim_whitespace,
        );
        if value.is_empty() || !seen.insert(value) {
            return None;
        }
    }
    Some(seen)
}

fn serialize_rows(
    table: &CsvTable,
    column_count: usize,
    options: &CsvOptions,
    ignore: &CsvIgnore,
) -> Vec<String> {
    table
        .rows
        .iter()
        .map(|row| {
            let mut serialized = String::new();
            let mut first = true;
            for col in 0..column_count {
                if ignore.ignores_column(col) {
                    continue;
                }
                if !first {
                    serialized.push('\u{1f}');
                }
                first = false;
                serialized.push_str(&normalize_text(
                    row.get(col).map(String::as_str).unwrap_or(""),
                    options.ignore_case,
                    options.trim_whitespace,
                ));
            }
            serialized
        })
        .collect()
}

fn compare_row(
    left: &CsvTable,
    right: &CsvTable,
    left_idx: usize,
    right_idx: usize,
    column_count: usize,
    options: &CsvOptions,
    ignore: &CsvIgnore,
) -> CsvRow {
    let mut changed_cols = Vec::new();
    let mut cells = Vec::new();
    for col in 0..column_count {
        if ignore.ignores_column(col) {
            continue;
        }
        let left_value = left.cell(left_idx, col);
        let right_value = right.cell(right_idx, col);
        if normalize_text(left_value, options.ignore_case, options.trim_whitespace)
            == normalize_text(right_value, options.ignore_case, options.trim_whitespace)
        {
            continue;
        }
        changed_cols.push(col);
        let (left_spans, right_spans) = intraline_spans(left_value, right_value);
        cells.push(CellDiff {
            col,
            left: left_value.to_string(),
            right: right_value.to_string(),
            left_spans,
            right_spans,
        });
    }
    CsvRow {
        left: Some(left_idx),
        right: Some(right_idx),
        kind: if changed_cols.is_empty() {
            CsvRowKind::Equal
        } else {
            CsvRowKind::Modified
        },
        changed_cols,
        cells,
    }
}

fn one_sided_count(rows: &[CsvRow]) -> usize {
    rows.iter()
        .filter(|row| matches!(row.kind, CsvRowKind::LeftOnly | CsvRowKind::RightOnly))
        .count()
}

fn one_sided_row(left: Option<usize>, right: Option<usize>) -> CsvRow {
    CsvRow {
        left,
        right,
        kind: if left.is_some() {
            CsvRowKind::LeftOnly
        } else {
            CsvRowKind::RightOnly
        },
        changed_cols: Vec::new(),
        cells: Vec::new(),
    }
}

pub fn normalize_text(value: &str, ignore_case: bool, trim: bool) -> String {
    let mut out = if trim {
        value.trim().to_string()
    } else {
        value.to_string()
    };
    if ignore_case {
        out = out.to_lowercase();
    }
    out
}

pub fn detect_delimiter(text: &str) -> u8 {
    let candidates = *b",;\t|";
    let mut scores = [0usize; 4];
    for line in text.lines().filter(|line| !line.trim().is_empty()).take(10) {
        let mut in_quotes = false;
        for ch in line.chars() {
            if ch == '"' {
                in_quotes = !in_quotes;
                continue;
            }
            if in_quotes {
                continue;
            }
            if let Some(pos) = candidates
                .iter()
                .position(|candidate| *candidate as char == ch)
            {
                scores[pos] += 1;
            }
        }
    }
    let mut best = 0;
    for idx in 1..scores.len() {
        if scores[idx] > scores[best] {
            best = idx;
        }
    }
    if scores[best] == 0 {
        b','
    } else {
        candidates[best]
    }
}

pub fn delimiter_label(byte: u8) -> &'static str {
    match byte {
        b',' => "comma",
        b';' => "semicolon",
        b'\t' => "tab",
        b'|' => "pipe",
        _ => "custom",
    }
}

/// Remove one column from a CSV file. The original file is backed up first.
/// Returns the backup path so the UI can mention it in the confirmation/done
/// message.
pub fn delete_column_from_file(
    path: &Path,
    column: usize,
    delimiter: u8,
) -> Result<PathBuf, String> {
    let bytes = fs::read(path).map_err(|error| format!("Could not read the file: {error}"))?;
    let decoded = decode_text(&bytes);
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(decoded.text.as_bytes());
    let mut records: Vec<Vec<String>> = Vec::new();
    let mut removed_any = false;
    for (index, record) in reader.records().enumerate() {
        let record = record
            .map_err(|error| format!("CSV parse error near record {}: {error}", index + 1))?;
        let mut fields: Vec<String> = record.iter().map(str::to_string).collect();
        if column < fields.len() {
            fields.remove(column);
            removed_any = true;
        }
        records.push(fields);
    }
    if !removed_any {
        return Err(format!(
            "Column {} does not exist in this file.",
            column + 1
        ));
    }

    let terminator = if decoded.text.contains("\r\n") {
        csv::Terminator::CRLF
    } else {
        csv::Terminator::Any(b'\n')
    };
    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .terminator(terminator)
        .from_writer(Vec::new());
    for record in &records {
        writer
            .write_record(record)
            .map_err(|error| format!("Could not rewrite the CSV: {error}"))?;
    }
    let csv_bytes = writer
        .into_inner()
        .map_err(|error| format!("Could not finish the CSV: {error}"))?;
    let text = String::from_utf8(csv_bytes)
        .map_err(|error| format!("Rewritten CSV was not valid UTF-8: {error}"))?;
    let output = encode_csv_text(&text, decoded.encoding, decoded.had_bom);

    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file.csv".to_string());
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let backup = path.with_file_name(format!("{file_name}.{stamp}.hcompare.bak"));
    fs::copy(path, &backup).map_err(|error| format!("Could not create backup: {error}"))?;
    fs::write(path, output).map_err(|error| format!("Could not write the CSV: {error}"))?;
    Ok(backup)
}

/// Find backups that `delete_column_from_file` left beside `path`, newest first.
pub fn column_backups(path: &Path) -> Vec<PathBuf> {
    let Some(dir) = path.parent() else {
        return Vec::new();
    };
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    let prefix = format!("{name}.");
    let mut backups: Vec<(u128, PathBuf)> = fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if !file_name.starts_with(&prefix) || !file_name.ends_with(".hcompare.bak") {
                return None;
            }
            let stamp = file_name[prefix.len()..file_name.len() - ".hcompare.bak".len()]
                .parse::<u128>()
                .unwrap_or(0);
            Some((stamp, entry.path()))
        })
        .collect();
    backups.sort_by_key(|left| std::cmp::Reverse(left.0));
    backups.into_iter().map(|(_, path)| path).collect()
}

/// Restore `path` from a backup created by `delete_column_from_file`.
pub fn restore_column_backup(path: &Path, backup: &Path) -> Result<(), String> {
    let bytes = fs::read(backup).map_err(|error| format!("Could not read the backup: {error}"))?;
    fs::write(path, bytes).map_err(|error| format!("Could not restore the file: {error}"))?;
    Ok(())
}

fn encode_csv_text(text: &str, encoding: &str, had_bom: bool) -> Vec<u8> {
    match encoding {
        "UTF-16 LE" => {
            let mut out = Vec::with_capacity(text.len() * 2 + 2);
            out.extend_from_slice(&[0xFF, 0xFE]);
            for unit in text.encode_utf16() {
                out.extend_from_slice(&unit.to_le_bytes());
            }
            out
        }
        "UTF-16 BE" => {
            let mut out = Vec::with_capacity(text.len() * 2 + 2);
            out.extend_from_slice(&[0xFE, 0xFF]);
            for unit in text.encode_utf16() {
                out.extend_from_slice(&unit.to_be_bytes());
            }
            out
        }
        _ => {
            if had_bom {
                let mut out = Vec::with_capacity(text.len() + 3);
                out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
                out.extend_from_slice(text.as_bytes());
                out
            } else {
                text.as_bytes().to_vec()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> CsvOptions {
        CsvOptions::default()
    }

    #[test]
    fn parses_quoted_fields_and_newlines() {
        let input = b"name,note\nalice,\"hello, world\"\nbob,\"line 1\nline 2\"\n";
        let table = parse_csv(input, &options()).unwrap();
        assert_eq!(table.column_count, 2);
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0][1], "hello, world");
        assert_eq!(table.rows[1][1], "line 1\nline 2");
        assert_eq!(table.source_rows, vec![2, 3]);
    }

    #[test]
    fn detects_semicolon_delimiter() {
        assert_eq!(detect_delimiter("a;b;c\n1;2;3\n"), b';');
        assert_eq!(detect_delimiter("a\tb\n1\t2\n"), b'\t');
        assert_eq!(detect_delimiter("plain text\n"), b',');
    }

    #[test]
    fn aligns_rows_in_order_and_finds_changed_cells() {
        let left = b"id,name,value\n1,alpha,10\n2,beta,20\n3,gamma,30\n";
        let right = b"id,name,value\n1,alpha,10\n2,BETA,21\n4,delta,40\n";
        let compare = compare_csv(left, right, &options()).unwrap();
        assert_eq!(compare.stats.equal, 1);
        assert_eq!(compare.stats.modified, 1);
        assert_eq!(compare.stats.removed, 1);
        assert_eq!(compare.stats.added, 1);
        assert_eq!(compare.stats.changed_cells, 2);
        let modified = compare
            .rows
            .iter()
            .find(|row| row.kind == CsvRowKind::Modified)
            .unwrap();
        assert_eq!(modified.changed_cols, vec![1, 2]);
    }

    #[test]
    fn key_alignment_makes_reordered_rows_equal() {
        let left = b"id,name\n2,beta\n1,alpha\n";
        let right = b"id,name\n1,alpha\n2,beta\n";
        let mut opts = options();
        opts.align = CsvAlign::Key(0);
        let compare = compare_csv(left, right, &opts).unwrap();
        assert_eq!(compare.stats.equal, 2);
        assert_eq!(compare.stats.changes(), 0);
        assert_eq!(compare.rows[0].left, Some(0));
        assert_eq!(compare.rows[0].right, Some(1));
    }

    #[test]
    fn key_alignment_handles_added_and_removed_keys() {
        let left = b"id,name\n1,alpha\n2,beta\n";
        let right = b"id,name\n1,alpha\n3,gamma\n";
        let mut opts = options();
        opts.align = CsvAlign::Key(0);
        let compare = compare_csv(left, right, &opts).unwrap();
        assert_eq!(compare.stats.equal, 1);
        assert_eq!(compare.stats.removed, 1);
        assert_eq!(compare.stats.added, 1);
    }

    #[test]
    fn auto_key_falls_back_when_first_column_has_duplicates() {
        let left = b"group,value\na,1\na,2\nb,3\n";
        let right = b"group,value\na,1\na,3\nb,3\n";
        let compare = compare_csv(left, right, &options()).unwrap();
        assert_eq!(compare.align, CsvAlign::RowOrder);
        assert_eq!(compare.stats.modified, 1);
    }

    #[test]
    fn auto_key_falls_back_when_first_column_does_not_overlap() {
        let left = b"name,value\nalpha,1\nbeta,2\ngamma,3\n";
        let right = b"name,value\nalpha,1\nBETA,3\ndelta,4\n";
        let compare = compare_csv(left, right, &options()).unwrap();
        assert_eq!(compare.align, CsvAlign::RowOrder);
        assert_eq!(compare.stats.modified, 2);
    }

    #[test]
    fn auto_prefers_row_order_when_a_changed_key_would_split_a_row() {
        let mut left = String::from("id,value\n");
        let mut right = String::from("id,value\n");
        for id in 1..=10 {
            left.push_str(&format!("{id},v{id}\n"));
            if id == 5 {
                right.push_str(&format!("FIVE,v{id}\n"));
            } else {
                right.push_str(&format!("{id},v{id}\n"));
            }
        }
        let compare = compare_csv(left.as_bytes(), right.as_bytes(), &options()).unwrap();
        assert_eq!(compare.align, CsvAlign::RowOrder);
        assert_eq!(compare.stats.modified, 1);
        assert_eq!(compare.stats.added, 0);
        assert_eq!(compare.stats.removed, 0);
        assert_eq!(compare.stats.changed_cells, 1);
    }

    #[test]
    fn ignoring_a_column_on_one_side_removes_its_cell_diff() {
        let mut ignore = CsvIgnore::default();
        ignore.right.insert(1);
        let compare = compare_csv_with_ignore(
            b"id,name\n1,alpha\n2,beta\n",
            b"id,name\n1,ALPHA\n2,BETA\n",
            &options(),
            &ignore,
        )
        .unwrap();
        assert_eq!(compare.stats.modified, 0);
        assert_eq!(compare.stats.equal, 2);
        assert_eq!(compare.stats.changed_cells, 0);
    }

    #[test]
    fn ignoring_an_extra_column_removes_the_column_count_warning() {
        let mut ignore = CsvIgnore::default();
        ignore.right.insert(2);
        let compare = compare_csv_with_ignore(
            b"id,name\n1,alpha\n",
            b"id,name,debug\n1,alpha,true\n",
            &options(),
            &ignore,
        )
        .unwrap();
        assert_eq!(compare.stats.equal, 1);
        assert!(compare
            .warnings
            .iter()
            .all(|warning| !warning.contains("Active column counts")));
    }

    #[test]
    fn auto_key_skips_an_ignored_first_column() {
        let mut ignore = CsvIgnore::default();
        ignore.left.insert(0);
        let compare = compare_csv_with_ignore(
            b"noise,id,value\nx,1,alpha\ny,2,beta\n",
            b"noise,id,value\nz,1,alpha\nw,2,beta\n",
            &options(),
            &ignore,
        )
        .unwrap();
        assert_eq!(compare.align, CsvAlign::Key(1));
        assert_eq!(compare.stats.equal, 2);
    }

    #[test]
    fn detects_changed_headers() {
        let left = b"id,name\n1,alpha\n";
        let right = b"id,title\n1,alpha\n";
        let compare = compare_csv(left, right, &options()).unwrap();
        assert_eq!(compare.changed_headers, vec![1]);
        assert_eq!(compare.stats.header_changes, 1);
    }

    #[test]
    fn can_treat_first_row_as_data() {
        let mut opts = options();
        opts.has_headers = false;
        let table = parse_csv(b"a,b\n1,2\n", &opts).unwrap();
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0], vec!["a", "b"]);
        assert_eq!(table.column_count, 2);
    }

    #[test]
    fn handles_missing_columns() {
        let left = b"a,b,c\n1,2,3\n";
        let right = b"a,b\n1,2\n";
        let compare = compare_csv(left, right, &options()).unwrap();
        assert_eq!(compare.column_count, 3);
        assert_eq!(compare.stats.modified, 1);
        assert_eq!(compare.rows[0].changed_cols, vec![2]);
        assert_eq!(compare.column_name(2), "c  /  Column 3");
        assert!(compare
            .warnings
            .iter()
            .any(|w| w.contains("column counts differ")));
    }

    #[test]
    fn parses_utf16_csv() {
        let text = "id,name\n1,alpha\n";
        let mut bytes = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let table = parse_csv(&bytes, &options()).unwrap();
        assert_eq!(table.encoding, "UTF-16 LE");
        assert_eq!(table.rows[0][1], "alpha");
    }

    #[test]
    fn ignores_empty_rows() {
        let table = parse_csv(b"a,b\n\n1,2\n,\n", &options()).unwrap();
        assert_eq!(table.rows.len(), 1);
    }

    #[test]
    fn out_of_range_key_falls_back() {
        let mut opts = options();
        opts.align = CsvAlign::Key(99);
        let compare = compare_csv(b"a,b\n1,2\n", b"a,b\n1,2\n", &opts).unwrap();
        assert_eq!(compare.align, CsvAlign::RowOrder);
        assert!(compare.warnings.iter().any(|w| w.contains("out of range")));
    }

    #[test]
    fn change_navigation() {
        let compare = compare_csv(
            b"id,name\n1,a\n2,b\n",
            b"id,name\n1,a\n2,B\n3,c\n",
            &options(),
        )
        .unwrap();
        assert_eq!(compare.next_change(None), Some(1));
        assert_eq!(compare.next_change(Some(1)), Some(2));
        assert_eq!(compare.previous_change(Some(2)), Some(1));
    }

    #[test]
    fn deleting_a_column_rewrites_the_file_and_creates_a_backup() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "hcompare-csv-delete-{}-{stamp}.csv",
            std::process::id()
        ));
        fs::write(&path, "id,name,value\n1,alpha,10\n2,beta,20\n").unwrap();
        let backup = delete_column_from_file(&path, 1, b',').unwrap();
        let rewritten = fs::read_to_string(&path).unwrap();
        assert_eq!(rewritten, "id,value\n1,10\n2,20\n");
        assert!(backup.exists());
        assert!(fs::read_to_string(&backup).unwrap().contains("alpha"));
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&backup);
    }

    #[test]
    fn column_backups_lists_newest_first_and_restore_round_trips() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "hcompare-csv-restore-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("data.csv");
        fs::write(&path, "id,name,value\n1,alpha,10\n").unwrap();
        let first = delete_column_from_file(&path, 1, b',').unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let second = delete_column_from_file(&path, 1, b',').unwrap();

        let backups = column_backups(&path);
        assert_eq!(backups.len(), 2);
        assert_eq!(backups[0], second);
        assert_eq!(backups[1], first);

        restore_column_backup(&path, &second).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "id,value\n1,10\n");

        restore_column_backup(&path, &first).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "id,name,value\n1,alpha,10\n"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
