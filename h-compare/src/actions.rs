//! Commands emitted by the UI and executed by the application layer.
//!
//! Keeping these types outside `app` avoids a dependency cycle: presentation
//! modules depend on this shared command vocabulary, and `app` depends on the
//! presentation modules.

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExportKind {
    CsvDifferences,
    CsvHtml,
    TextHtml,
    TextUnified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UiAction {
    OpenNode(String),
    Copy { rel: String, from_left: bool },
    Delete { rel: String, left: bool },
    Reveal { rel: String, left: bool },
    OpenDefault { rel: String, left: bool },
    Sync { from_left: bool },
    Export(ExportKind),
    Scan,
    ReloadOpen,
    NextFileDiff { forward: bool },
    RecomputeCsv { hidden: Vec<usize> },
    DeleteCsvColumn { left: bool, column: usize },
    DeleteCsvColumnBoth { column: usize },
    RestoreCsvBackup { path: PathBuf, backup: PathBuf },
    UndoCsvColumnDelete,
}
