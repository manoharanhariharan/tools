//! Word document text extraction.
//!
//! `.docx` files are parsed directly from `word/document.xml`. Legacy binary
//! `.doc` files use an installed converter such as LibreOffice, pandoc,
//! antiword or catdoc, because the old Word binary format is not practical to
//! parse safely in-process.

use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct WordExtraction {
    pub text: String,
    pub method: String,
}

/// Extract comparable plain text from a `.docx` or legacy `.doc` file.
pub fn extract_word_text(path: Option<&Path>, bytes: &[u8]) -> Result<WordExtraction, String> {
    if bytes.starts_with(b"PK\x03\x04") {
        match extract_docx_text(bytes) {
            Ok(text) => {
                return Ok(WordExtraction {
                    text,
                    method: "built-in DOCX reader".to_string(),
                })
            }
            Err(error) if path.is_none() => return Err(error),
            Err(error) => {
                if let Some(text) = external_word_text(path.expect("path checked above"))? {
                    return Ok(text);
                }
                return Err(error);
            }
        }
    }

    let Some(path) = path else {
        return Err(
            "Legacy .doc extraction needs a file path and an installed converter.".to_string(),
        );
    };
    external_word_text(path)?.ok_or_else(|| {
        "Legacy .doc files need LibreOffice (soffice), pandoc, antiword or catdoc in PATH. \
Save the document as .docx for built-in support."
            .to_string()
    })
}

pub fn extract_docx_text(bytes: &[u8]) -> Result<String, String> {
    let reader = Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|error| format!("DOCX archive error: {error}"))?;
    let file_name = archive
        .file_names()
        .find(|name| name.replace('\\', "/") == "word/document.xml")
        .map(str::to_string)
        .ok_or_else(|| "DOCX is missing word/document.xml".to_string())?;
    let mut document = archive
        .by_name(&file_name)
        .map_err(|error| format!("Could not open word/document.xml: {error}"))?;
    let mut xml = String::new();
    document
        .read_to_string(&mut xml)
        .map_err(|error| format!("Could not read word/document.xml: {error}"))?;
    docx_xml_to_text(&xml)
}

fn docx_xml_to_text(xml: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut out = String::new();
    let mut in_text = false;
    let mut table_depth = 0usize;
    let mut cell_depth = 0usize;
    loop {
        let event = reader
            .read_event()
            .map_err(|error| format!("DOCX XML parse error: {error}"))?;
        match event {
            Event::Start(event) => match local_name(event.name().as_ref()) {
                b"t" => in_text = true,
                b"tab" => out.push('\t'),
                b"br" | b"cr" => out.push('\n'),
                b"tbl" => table_depth += 1,
                b"tc" => cell_depth += 1,
                _ => {}
            },
            Event::Empty(event) => match local_name(event.name().as_ref()) {
                b"tab" => out.push('\t'),
                b"br" | b"cr" => out.push('\n'),
                b"tbl" => table_depth += 1,
                b"tc" => cell_depth += 1,
                _ => {}
            },
            Event::End(event) => match local_name(event.name().as_ref()) {
                b"t" => in_text = false,
                b"p" => {
                    if cell_depth > 0 {
                        out.push(' ');
                    } else {
                        out.push('\n');
                    }
                }
                b"tc" => {
                    cell_depth = cell_depth.saturating_sub(1);
                    while out.ends_with(' ') {
                        out.pop();
                    }
                    out.push('\t');
                }
                b"tr" => {
                    if out.ends_with('\t') {
                        out.pop();
                    }
                    out.push('\n');
                }
                b"tbl" => {
                    table_depth = table_depth.saturating_sub(1);
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                }
                _ => {}
            },
            Event::Text(event) if in_text => {
                let value = event
                    .decode()
                    .map_err(|error| format!("DOCX text encoding error: {error}"))?;
                out.push_str(&value);
            }
            Event::GeneralRef(event) if in_text => {
                if let Some(ch) = event
                    .resolve_char_ref()
                    .map_err(|error| format!("DOCX character reference error: {error}"))?
                {
                    out.push(ch);
                } else {
                    let name = event
                        .decode()
                        .map_err(|error| format!("DOCX entity error: {error}"))?;
                    match name.as_ref() {
                        "amp" => out.push('&'),
                        "lt" => out.push('<'),
                        "gt" => out.push('>'),
                        "quot" => out.push('"'),
                        "apos" => out.push('\''),
                        _ => {}
                    }
                }
            }
            Event::CData(event) if in_text => {
                let value = event
                    .decode()
                    .map_err(|error| format!("DOCX CDATA encoding error: {error}"))?;
                out.push_str(&value);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let _ = table_depth;
    let mut text = out.trim_end().to_string();
    if !text.is_empty() {
        text.push('\n');
    }
    Ok(text)
}

fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn external_word_text(path: &Path) -> Result<Option<WordExtraction>, String> {
    let path_text = path.display().to_string();

    if let Some(text) = run_stdout("pandoc", &["-t", "plain", "--wrap=none", &path_text])? {
        if !text.trim().is_empty() {
            return Ok(Some(WordExtraction {
                text: normalize_external_text(text),
                method: "pandoc".to_string(),
            }));
        }
    }
    if let Some(text) = run_stdout("antiword", &["-w", "0", &path_text])? {
        if !text.trim().is_empty() {
            return Ok(Some(WordExtraction {
                text: normalize_external_text(text),
                method: "antiword".to_string(),
            }));
        }
    }
    if let Some(text) = run_stdout("catdoc", &[&path_text])? {
        if !text.trim().is_empty() {
            return Ok(Some(WordExtraction {
                text: normalize_external_text(text),
                method: "catdoc".to_string(),
            }));
        }
    }

    for program in soffice_candidates() {
        if let Some(text) = run_soffice(&program, path)? {
            if !text.trim().is_empty() {
                return Ok(Some(WordExtraction {
                    text: normalize_external_text(text),
                    method: format!("LibreOffice ({program})"),
                }));
            }
        }
    }

    Ok(None)
}

fn soffice_candidates() -> Vec<String> {
    let mut candidates = vec!["soffice".to_string(), "libreoffice".to_string()];
    #[cfg(windows)]
    {
        for path in [
            r"C:\Program Files\LibreOffice\program\soffice.exe",
            r"C:\Program Files (x86)\LibreOffice\program\soffice.exe",
        ] {
            if Path::new(path).is_file() {
                candidates.push(path.to_string());
            }
        }
    }
    candidates
}

fn command(program: &str) -> Command {
    let mut command = Command::new(program);
    command.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

fn run_stdout(program: &str, args: &[&str]) -> Result<Option<String>, String> {
    let output = match command(program).args(args).output() {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Could not run {program}: {error}")),
    };
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()))
}

fn run_soffice(program: &str, path: &Path) -> Result<Option<String>, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("hcompare-word-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Could not create Word temp folder: {error}"))?;
    let profile = dir.join("profile");
    let profile_url = format!(
        "file:///{}",
        profile.display().to_string().replace('\\', "/")
    );
    let output = command(program)
        .args([
            format!("-env:UserInstallation={profile_url}"),
            "--headless".to_string(),
            "--norestore".to_string(),
            "--convert-to".to_string(),
            "txt:Text".to_string(),
            "--outdir".to_string(),
            dir.display().to_string(),
            path.display().to_string(),
        ])
        .output();
    let result = match output {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let _ = fs::remove_dir_all(&dir);
            return Ok(None);
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&dir);
            return Err(format!("Could not run {program}: {error}"));
        }
    };
    let mut text = None;
    if result.status.success() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let candidate = entry.path();
                if candidate
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("txt"))
                {
                    if let Ok(value) = fs::read_to_string(&candidate) {
                        text = Some(value);
                        break;
                    }
                }
            }
        }
    }
    let _ = fs::remove_dir_all(&dir);
    Ok(text)
}

fn normalize_external_text(text: String) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        out.push_str(line.trim_end());
        out.push('\n');
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    fn docx_bytes(document_xml: &str) -> Vec<u8> {
        docx_bytes_named("word/document.xml", document_xml)
    }

    fn docx_bytes_named(name: &str, document_xml: &str) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut writer = ZipWriter::new(&mut buffer);
            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            writer
                .start_file(name, options)
                .expect("start document.xml");
            writer
                .write_all(document_xml.as_bytes())
                .expect("write document.xml");
            writer.finish().expect("finish docx");
        }
        buffer.into_inner()
    }

    fn document(body: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>{body}</w:body></w:document>"#
        )
    }

    #[test]
    fn extracts_paragraphs_and_tables() {
        let xml = document(
            r#"<w:p><w:r><w:t>Hello</w:t></w:r><w:r><w:t xml:space="preserve"> world &amp; all</w:t></w:r></w:p>
<w:p><w:r><w:t>Second line</w:t></w:r><w:r><w:tab/><w:t>tabbed</w:t></w:r></w:p>
<w:tbl>
  <w:tr><w:tc><w:p><w:r><w:t>A1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B1</w:t></w:r></w:p></w:tc></w:tr>
  <w:tr><w:tc><w:p><w:r><w:t>A2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B2</w:t></w:r></w:p></w:tc></w:tr>
</w:tbl>
<w:p><w:r><w:t>After table</w:t></w:r></w:p>"#,
        );
        let bytes = docx_bytes(&xml);
        let text = extract_docx_text(&bytes).expect("extract");
        assert_eq!(
            text,
            "Hello world & all\nSecond line\ttabbed\nA1\tB1\nA2\tB2\nAfter table\n"
        );
    }

    #[test]
    fn accepts_backslash_entry_names_from_zip_tools() {
        let xml = document("<w:p><w:r><w:t>Backslash path</w:t></w:r></w:p>");
        let bytes = docx_bytes_named("word\\document.xml", &xml);
        let text = extract_docx_text(&bytes).expect("extract");
        assert_eq!(text, "Backslash path\n");
    }

    #[test]
    fn skips_deleted_text_and_reads_insertions() {
        let xml = document(
            r#"<w:p><w:r><w:t>Keep </w:t></w:r><w:del><w:r><w:delText>gone</w:delText></w:r></w:del><w:ins><w:r><w:t>new</w:t></w:r></w:ins></w:p>"#,
        );
        let bytes = docx_bytes(&xml);
        let text = extract_docx_text(&bytes).expect("extract");
        assert_eq!(text, "Keep new\n");
    }

    #[test]
    fn rejects_non_docx_archives() {
        let bytes = docx_bytes(&document("<w:p/>"));
        assert!(!extract_docx_text(b"PK\x03\x04broken").is_ok());
        assert!(extract_docx_text(&bytes).is_ok());
    }

    #[test]
    fn recognizes_word_extensions() {
        assert!(crate::load::is_word_name("report.docx"));
        assert!(crate::load::is_word_name("REPORT.DOC"));
        assert!(!crate::load::is_word_name("report.txt"));
    }
}
