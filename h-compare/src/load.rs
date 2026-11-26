//! Loads a file pair and selects the right comparison view (text, CSV, PDF or binary).

use crate::csvdiff::{compare_csv, delimiter_label, CsvCompare, CsvOptions};
use crate::image_compare::{self, ImageCompareData};
use crate::model::CompareOptions;
use crate::textdiff::{build_text_diff, decode_text, looks_binary, TextDiffView};
use crate::word;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

pub const MAX_TEXT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenKind {
    Text,
    Web,
    Csv,
    Pdf,
    Word,
    Image,
    Binary,
    Error,
}

#[derive(Debug, Clone)]
pub struct BinaryBytes {
    pub left: Vec<u8>,
    pub right: Vec<u8>,
    pub left_truncated: bool,
    pub right_truncated: bool,
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
    pub binary: Option<BinaryBytes>,
    pub image: Option<ImageCompareData>,
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
        binary: None,
        image: None,
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
    let is_word = is_word_name(rel)
        || data
            .left_path
            .as_deref()
            .map(is_word_name_path)
            .unwrap_or(false)
        || data
            .right_path
            .as_deref()
            .map(is_word_name_path)
            .unwrap_or(false);
    if is_word && !oversized {
        let mut methods = Vec::new();
        let mut errors = Vec::new();
        let left_text = if left_side.bytes.is_some() {
            match word::extract_word_text(data.left_path.as_deref(), left_bytes) {
                Ok(extraction) => {
                    methods.push(format!("left: {}", extraction.method));
                    extraction.text
                }
                Err(error) => {
                    errors.push(format!("left: {error}"));
                    String::new()
                }
            }
        } else {
            String::new()
        };
        let right_text = if right_side.bytes.is_some() {
            match word::extract_word_text(data.right_path.as_deref(), right_bytes) {
                Ok(extraction) => {
                    methods.push(format!("right: {}", extraction.method));
                    extraction.text
                }
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
                "Could not extract text from the Word document: {}",
                errors.join("; ")
            ));
            return data;
        }
        if !errors.is_empty() {
            data.messages
                .push(format!("Word extraction warning: {}", errors.join("; ")));
        }
        if !methods.is_empty() {
            data.messages.push(format!(
                "Word text extracted for comparison ({}). Layout, images, headers/footers, comments and tracked deletions are not shown.",
                methods.join("; ")
            ));
        }
        let mut view = build_text_diff(&left_text, &right_text, compare_options);
        view.left_encoding = "Word";
        view.right_encoding = "Word";
        view.left_bytes = left_bytes.len();
        view.right_bytes = right_bytes.len();
        data.kind = OpenKind::Word;
        data.text = Some(view);
        return data;
    }
    let is_image = is_image_name(rel)
        || data
            .left_path
            .as_deref()
            .map(is_image_name_path)
            .unwrap_or(false)
        || data
            .right_path
            .as_deref()
            .map(is_image_name_path)
            .unwrap_or(false);
    if is_image && !oversized {
        match image_compare::compare_images(left_bytes, right_bytes) {
            Ok(images) => {
                data.messages.push(
                    "Images compared by decoded pixels. Difference mode highlights changed pixels; animated images use the first frame."
                        .to_string(),
                );
                data.image = Some(images);
                data.kind = OpenKind::Image;
                return data;
            }
            Err(error) => data.messages.push(format!(
                "Could not decode this pair as images ({error}); showing the binary hex view instead."
            )),
        }
    }
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
        data.binary = Some(BinaryBytes {
            left: left_bytes.to_vec(),
            right: right_bytes.to_vec(),
            left_truncated: left_side.oversized,
            right_truncated: right_side.oversized,
        });
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
    let is_web = is_web_name(rel)
        || data
            .left_path
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .map(is_web_name)
            .unwrap_or(false)
        || data
            .right_path
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .map(is_web_name)
            .unwrap_or(false);
    if is_web {
        data.messages.push(
            "HTML is compared as source text. Browser rendering, scripts and network resources are not executed."
                .to_string(),
        );
    }
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
    data.kind = if is_web {
        OpenKind::Web
    } else {
        OpenKind::Text
    };
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

pub(crate) fn is_word_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".doc", ".docx", ".docm"]
        .iter()
        .any(|extension| lower.ends_with(extension))
}

fn is_word_name_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(is_word_name)
        .unwrap_or(false)
}

fn is_image_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".webp", ".tif", ".tiff", ".ico",
    ]
    .iter()
    .any(|extension| lower.ends_with(extension))
}

fn is_image_name_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(is_image_name)
        .unwrap_or(false)
}

fn is_web_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".html", ".htm", ".xhtml"]
        .iter()
        .any(|extension| lower.ends_with(extension))
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
    fn detects_word_extensions() {
        assert!(is_word_name("report.docx"));
        assert!(is_word_name("REPORT.DOC"));
        assert!(is_word_name("macro.docm"));
        assert!(!is_word_name("report.txt"));
    }

    #[test]
    fn detects_image_and_web_extensions() {
        assert!(is_image_name("photo.PNG"));
        assert!(is_image_name("scan.jpeg"));
        assert!(!is_image_name("photo.svg"));
        assert!(is_web_name("index.html"));
        assert!(is_web_name("page.XHTML"));
        assert!(!is_web_name("page.css"));
    }

    #[test]
    fn loads_html_as_web_source_comparison() {
        let stamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("hcompare-web-load-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let left = dir.join("left.html");
        let right = dir.join("right.html");
        fs::write(&left, "<html><body><h1>Old</h1></body></html>\n").unwrap();
        fs::write(&right, "<html><body><h1>New</h1></body></html>\n").unwrap();
        let data = load_compare(
            "page.html",
            Some(left),
            Some(right),
            &CompareOptions::default(),
            &CsvOptions::default(),
        );
        assert_eq!(data.kind, OpenKind::Web);
        let view = data.text.expect("web text view");
        assert_eq!(view.stats.modified, 1);
        assert!(data
            .messages
            .iter()
            .any(|message| message.contains("HTML is compared as source text")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn loads_binary_files_with_hex_bytes() {
        let stamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("hcompare-hex-load-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let left = dir.join("left.bin");
        let right = dir.join("right.bin");
        fs::write(&left, [0u8, 1, 2, 3, 4]).unwrap();
        fs::write(&right, [0u8, 9, 2, 3, 8]).unwrap();
        let data = load_compare(
            "data.bin",
            Some(left),
            Some(right),
            &CompareOptions::default(),
            &CsvOptions::default(),
        );
        assert_eq!(data.kind, OpenKind::Binary);
        let bytes = data.binary.expect("binary bytes");
        assert_eq!(bytes.left, vec![0, 1, 2, 3, 4]);
        assert_eq!(bytes.right, vec![0, 9, 2, 3, 8]);
        assert_eq!(crate::ui::hex_view::HexUiState::new(&bytes).diff_count, 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn loads_docx_as_word_comparison() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        use zip::{CompressionMethod, ZipWriter};

        fn write_docx(path: &Path, body: &str) {
            let file = fs::File::create(path).expect("create docx");
            let mut writer = ZipWriter::new(file);
            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            writer
                .start_file("word/document.xml", options)
                .expect("start document.xml");
            let xml = format!(
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document>"#
            );
            writer
                .write_all(xml.as_bytes())
                .expect("write document.xml");
            writer.finish().expect("finish docx");
        }

        let stamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("hcompare-word-load-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let left = dir.join("left.docx");
        let right = dir.join("right.docx");
        write_docx(&left, "<w:p><w:r><w:t>Alpha</w:t></w:r></w:p>");
        write_docx(&right, "<w:p><w:r><w:t>Beta</w:t></w:r></w:p>");

        let data = load_compare(
            "report.docx",
            Some(left),
            Some(right),
            &CompareOptions::default(),
            &CsvOptions::default(),
        );
        assert_eq!(data.kind, OpenKind::Word);
        let view = data.text.expect("word text view");
        assert_eq!(view.stats.modified, 1);
        assert!(data
            .messages
            .iter()
            .any(|message| message.contains("Word text extracted")));
        assert!(dir.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn first_binary_difference_is_reported() {
        assert_eq!(first_diff_offset(b"abc", b"abd"), Some(2));
        assert_eq!(first_diff_offset(b"abc", b"abc"), None);
        assert_eq!(first_diff_offset(b"abc", b"abcdef"), Some(3));
    }
}
