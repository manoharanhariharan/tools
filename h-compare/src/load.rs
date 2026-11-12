//! Loads a file pair and selects the right comparison view (text, CSV, PDF or binary).

use crate::csvdiff::{compare_csv, delimiter_label, CsvCompare, CsvOptions};
use crate::model::CompareOptions;
use crate::textdiff::{build_text_diff, decode_text, looks_binary, TextDiffView};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

pub const MAX_TEXT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenKind {
    Text,
    Csv,
    Pdf,
    Binary,
    Error,
}

#[derive(Debug, Clone)]
pub struct OpenData {
    pub rel: String,
    pub kind: OpenKind,
    pub left_path: Option<PathBuf>,
    pub right_path: Option<PathBuf>,
    pub left_len: Option<u64>,
    pub right_len: Option<u64>,
    pub left_modified: Option<SystemTime>,
    pub right_modified: Option<SystemTime>,
    pub text: Option<TextDiffView>,
    pub csv: Option<CsvCompare>,
    pub messages: Vec<String>,
    pub error: Option<String>,
    pub first_binary_diff: Option<u64>,
}

#[derive(Default)]
struct SideData {
    path: Option<PathBuf>,
    bytes: Option<Vec<u8>>,
    len: Option<u64>,
    modified: Option<SystemTime>,
    oversized: bool,
    messages: Vec<String>,
}

/// Load and compare a single pair of files. Intended for a worker thread.
pub fn load_compare(
    rel: &str,
    left: Option<PathBuf>,
    right: Option<PathBuf>,
    compare_options: &CompareOptions,
    csv_options: &CsvOptions,
) -> OpenData {
    let left_side = read_side(left, "left");
    let right_side = read_side(right, "right");
    let mut messages = left_side.messages;
    messages.extend(right_side.messages);

    let mut data = OpenData {
        rel: rel.to_string(),
        kind: OpenKind::Error,
        left_path: left_side.path.clone(),
        right_path: right_side.path.clone(),
        left_len: left_side.len,
        right_len: right_side.len,
        left_modified: left_side.modified,
        right_modified: right_side.modified,
        text: None,
        csv: None,
        messages,
        error: None,
        first_binary_diff: None,
    };

    let left_bytes = left_side.bytes.as_deref().unwrap_or(&[]);
    let right_bytes = right_side.bytes.as_deref().unwrap_or(&[]);
    if left_side.bytes.is_none() && right_side.bytes.is_none() {
        data.error = Some("Neither side of this entry could be read.".to_string());
        return data;
    }

    let binary = looks_binary(left_bytes) || looks_binary(right_bytes);
    let oversized = left_side.oversized || right_side.oversized;
    let is_pdf = is_pdf_name(rel)
        || data.left_path.as_deref().map(is_pdf_path).unwrap_or(false)
        || data.right_path.as_deref().map(is_pdf_path).unwrap_or(false);
    if is_pdf && !oversized {
        let mut errors = Vec::new();
        let left_text = if left_side.bytes.is_some() {
            match extract_pdf_text(left_side.path.as_deref(), left_bytes) {
                Ok(text) => text,
                Err(error) => {
                    errors.push(format!("left: {error}"));
                    String::new()
                }
            }
        } else {
            String::new()
        };
        let right_text = if right_side.bytes.is_some() {
            match extract_pdf_text(right_side.path.as_deref(), right_bytes) {
                Ok(text) => text,
                Err(error) => {
                    errors.push(format!("right: {error}"));
                    String::new()
                }
            }
        } else {
            String::new()
        };
        if left_text.is_empty() && right_text.is_empty() && !errors.is_empty() {
            data.error = Some(format!(
                "Could not extract text from the PDF: {}",
                errors.join("; ")
            ));
            return data;
        }
        if !errors.is_empty() {
            data.messages
                .push(format!("PDF extraction warning: {}", errors.join("; ")));
        }
        data.messages.push(
            "PDF text extracted for comparison. Scanned/image-only pages have no text layer."
                .to_string(),
        );
        let mut view = build_text_diff(&left_text, &right_text, compare_options);
        view.left_encoding = "PDF";
        view.right_encoding = "PDF";
        view.left_bytes = left_bytes.len();
        view.right_bytes = right_bytes.len();
        data.kind = OpenKind::Pdf;
        data.text = Some(view);
        return data;
    }
    if binary {
        data.kind = OpenKind::Binary;
        if !oversized && !left_bytes.is_empty() && !right_bytes.is_empty() {
            data.first_binary_diff = first_diff_offset(left_bytes, right_bytes);
        }
        if oversized {
            data.messages.push(format!(
                "File is larger than {} MB; showing metadata only.",
                MAX_TEXT_BYTES / (1024 * 1024)
            ));
        }
        return data;
    }

    if oversized {
        data.error = Some(format!(
            "File is larger than the {} MB text limit. Use a streaming diff tool for this file.",
            MAX_TEXT_BYTES / (1024 * 1024)
        ));
        return data;
    }

    let is_csv = is_csv_name(rel)
        || data
            .left_path
            .as_deref()
            .map(is_csv_name_path)
            .unwrap_or(false)
        || data
            .right_path
            .as_deref()
            .map(is_csv_name_path)
            .unwrap_or(false);

    if is_csv {
        match compare_csv(left_bytes, right_bytes, csv_options) {
            Ok(compare) => {
                if csv_options.delimiter.byte().is_none() {
                    data.messages.push(format!(
                        "Detected {} delimiter.",
                        delimiter_label(compare.left.delimiter)
                    ));
                }
                if compare.left.encoding != compare.right.encoding {
                    data.messages.push(format!(
                        "Encodings differ: {} / {}.",
                        compare.left.encoding, compare.right.encoding
                    ));
                }
                data.messages.extend(compare.warnings.iter().cloned());
                data.kind = OpenKind::Csv;
                data.csv = Some(compare);
                return data;
            }
            Err(err) => {
                data.messages.push(format!(
                    "Could not parse this pair as CSV ({err}); showing a text diff instead."
                ));
            }
        }
    }

    let left_decoded = decode_text(left_bytes);
    let right_decoded = decode_text(right_bytes);
    if left_decoded.encoding != "UTF-8" || right_decoded.encoding != "UTF-8" {
        data.messages.push(format!(
            "Encoding: {} / {}.",
            left_decoded.encoding, right_decoded.encoding
        ));
    }
    if left_decoded.had_bom != right_decoded.had_bom {
        data.messages
            .push("Only one side has a byte-order mark.".to_string());
    }
    let mut view = build_text_diff(&left_decoded.text, &right_decoded.text, compare_options);
    view.left_encoding = left_decoded.encoding;
    view.right_encoding = right_decoded.encoding;
    view.left_bom = left_decoded.had_bom;
    view.right_bom = right_decoded.had_bom;
    view.left_bytes = left_bytes.len();
    view.right_bytes = right_bytes.len();
    if view.left_line_ending != view.right_line_ending {
        data.messages.push(format!(
            "Line endings differ: {} / {}.",
            view.left_line_ending.label(),
            view.right_line_ending.label()
        ));
    }
    data.kind = OpenKind::Text;
    data.text = Some(view);
    data
}

fn read_side(path: Option<PathBuf>, label: &str) -> SideData {
    let Some(path) = path else {
        return SideData::default();
    };
    let mut side = SideData {
        path: Some(path.clone()),
        ..Default::default()
    };
    match fs::metadata(&path) {
        Ok(meta) => {
            side.len = Some(meta.len());
            side.modified = meta.modified().ok();
        }
        Err(err) => {
            side.messages
                .push(format!("Could not read {label} file metadata: {err}"));
        }
    }
    if side.len.unwrap_or(0) > MAX_TEXT_BYTES {
        side.oversized = true;
        if let Ok(mut file) = fs::File::open(&path) {
            let mut prefix = vec![0u8; 8192];
            if let Ok(read) = file.read(&mut prefix) {
                prefix.truncate(read);
                side.bytes = Some(prefix);
            }
        }
        side.messages.push(format!(
            "{label} file is larger than {} MB; only a small prefix was inspected.",
            MAX_TEXT_BYTES / (1024 * 1024)
        ));
        return side;
    }
    match fs::read(&path) {
        Ok(bytes) => side.bytes = Some(bytes),
        Err(err) => side
            .messages
            .push(format!("Could not read {label} file: {err}")),
    }
    side
}

fn first_diff_offset(left: &[u8], right: &[u8]) -> Option<u64> {
    let common = left.len().min(right.len());
    let mut offset = 0usize;
    while offset < common {
        let end = (offset + 64 * 1024).min(common);
        if left[offset..end] != right[offset..end] {
            for idx in offset..end {
                if left[idx] != right[idx] {
                    return Some(idx as u64);
                }
            }
        }
        offset = end;
    }
    if left.len() != right.len() {
        Some(common as u64)
    } else {
        None
    }
}

fn is_csv_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".csv", ".tsv", ".tab", ".psv"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

fn is_csv_name_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(is_csv_name)
        .unwrap_or(false)
}

fn is_pdf_name(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".pdf")
}

fn is_pdf_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

/// Extract the text layer from a PDF. Poppler's `pdftotext` is used when it is
/// available because it is substantially faster on large documents; otherwise
/// the pure-Rust `pdf-extract` parser is used as a self-contained fallback.
fn extract_pdf_text(path: Option<&Path>, bytes: &[u8]) -> Result<String, String> {
    if let Some(path) = path {
        if let Ok(output) = Command::new("pdftotext")
            .arg("-layout")
            .arg("-enc")
            .arg("UTF-8")
            .arg(path)
            .arg("-")
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout).into_owned();
                if !text.trim().is_empty() {
                    return Ok(text);
                }
            }
        }
    }
    let extracted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pdf_extract::extract_text_from_mem(bytes)
    }))
    .map_err(|_| "the PDF parser panicked".to_string())?;
    extracted.map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_csv_extensions() {
        assert!(is_csv_name("a.csv"));
        assert!(is_csv_name("A.CSV"));
        assert!(is_csv_name("a.tsv"));
        assert!(!is_csv_name("a.txt"));
    }

    #[test]
    fn first_binary_difference_is_reported() {
        assert_eq!(first_diff_offset(b"abc", b"abd"), Some(2));
        assert_eq!(first_diff_offset(b"abc", b"abc"), None);
        assert_eq!(first_diff_offset(b"abc", b"abcdef"), Some(3));
    }
}
