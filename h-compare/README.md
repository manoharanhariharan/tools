# H-Compare

A fast, native folder / text / CSV / PDF / Word comparison tool written in Rust. It is built for the
"open two folders and find what changed" workflow:

- recursive folder tree with same / different / left-only / right-only statuses
- side-by-side text diff with line and character-level highlights, plus merged view
- table-aware CSV diff with cell-level changes, merged or side-by-side layouts
- Word (.docx / .doc) text comparison with paragraph and table extraction
- PDF text diff (Poppler fast path with a bundled pure-Rust fallback)
- Ten polished theme presets, search, change navigation, copy/sync actions and HTML/CSV exports

## Features

### Appearance

- Ten theme presets: **Dark**, **Light**, **PRemoteM**, **Secret Key**, **Greystone**,
  **Asphalt**, **Wine**, **Forest**, **Soil** and **High Contrast**.
- Pick a preset from the top-bar **Settings** menu; the choice is persisted in the settings
  file and applies immediately.
- Crisp, icon + text buttons throughout the toolbar, comparison views, dialogs and
  context menus, so each action is self-describing.
- One **Settings** menu consolidates theme selection, the full comparison options dialog and
  app-level actions that were previously split across separate Theme/Options/Quick options
  menus.
- Intent-based buttons: filled **primary** actions, softly outlined **secondary** actions,
  quiet **ghost** actions and muted red **danger** actions for destructive operations.
- Right-click context menus throughout the tree and comparison tables.

### Folder comparison

- Recursively scans both roots in parallel and compares files byte-for-byte.
- Status-aware tree: `=` same, `~` different, `-` left only, `+` right only, `!` type mismatch/error.
- Filter by status, filter by name, expand/collapse all, and jump between differences.
- Collapse the whole tree panel with the `«` button or `Ctrl+B`; a slim FILES strip restores it.
- Recent comparisons are available from the top-bar **Recent** menu and the welcome screen.
- Optional skip list for common build directories (`node_modules`, `target`, `dist`, `.git`, …).
- Binary files are detected and reported with metadata and the first differing byte.
- File copy (A → B / B → A), sync all differences with a confirmation dialog, reveal in
  Explorer, open with the default application, and delete with confirmation.
- Scanning and file loading run on worker threads, so the UI stays responsive.

### Text comparison

- Side-by-side line alignment using Myers diff.
- Two layouts:
  - **Side by side** shows both files with synchronized vertical scrolling.
  - **Merged** shows one unified stream: equal lines once, changed lines stacked as
    left then right, and added/removed lines with a single marker.
- The options grid can be collapsed with the **▲ / ▼** arrow at the right end of the
  header; change navigation stays visible while collapsed, and the collapsed state is
  remembered.
- Intra-line character highlights for changed lines.
- Handles CRLF/LF, UTF-8 BOM, and UTF-16 LE/BE BOM.
- Options: ignore case, ignore whitespace, trim trailing whitespace.
- Search with case sensitivity, match count, and next/previous navigation.
- F7 / Shift+F7 change navigation steps through each **intra-line change group** before
  moving to the next changed line, focusing and scrolling the group into view. The
  counter shows the current group and the total number of change groups.
- Arrow keys move the comparison cursor: Up/Down/PageUp/PageDown/Home/End move by line,
  and Left/Right step through the focused intra-line change groups (moving to the
  neighbouring changed line at a group boundary).
- Side-by-side mode shares vertical and horizontal scrolling between both panes.
- Unified diff and HTML report export.
- System CJK font fallback (when a font such as Yu Gothic or Microsoft YaHei exists) so
  Japanese / Chinese / Korean text displays correctly.

### PDF comparison

- PDFs are compared by their **text layer**, using the same text diff view.
- Fast path: Poppler `pdftotext` is used when it is on `PATH` (recommended for very large
  PDFs). Otherwise the bundled pure-Rust `pdf-extract` parser is used, so no external
  installation is required.
- Extraction runs on the file-loading worker thread, keeping the UI responsive.
- Scanned/image-only PDFs have no text layer and cannot be compared as text; the view
  reports that no text was extracted.
- Both **Side by side** and **Merged** layouts are available for PDF text.

### Word comparison

- `.docx` and `.docm` files are read directly from `word/document.xml` with no Office
  installation required.
- Paragraphs, tabs, line breaks and tables are converted to comparable plain text; table
  cells are tab-separated and rows become separate lines.
- Legacy `.doc` files are converted through an installed `soffice`/`libreoffice`, `pandoc`,
  `antiword` or `catdoc` executable. If none is available, H-Compare explains how to
  install one or to save the document as `.docx`.
- Word comparisons use the same side-by-side/merged view, intra-line highlights, search,
  F7 change navigation, copy and export actions as text and PDF comparisons.
- Word layout, images, headers/footers, comments and tracked deletions are not part of the
  text comparison.

### CSV / TSV comparison

- RFC 4180-style parsing, including quoted delimiters and quoted newlines.
- Delimiter auto-detection (comma, semicolon, tab, pipe) with manual override.
- Header-row detection; optional empty-row skipping and cell whitespace trimming.
- Row alignment:
  - **Auto** uses the first visible column as a key when it is unique in both files
    and the key sets overlap enough; otherwise it falls back to row order.
  - When key alignment would split a few rows into added + removed pairs but the
    files have the same row count, Auto prefers row order so the real cell changes
    are reported and counted.
  - **Row order** aligns by position.
  - Any column can be selected explicitly as a key.
- Cell-level highlighting with character-level highlights in side-by-side mode.
- Column headers are tinted and underlined when the column contains differing cells; the
  tooltip reports the number of differing cells and any header-text change. This makes
  changed columns easy to spot while scrolling wide tables.
- Two layouts:
  - **Merged** shows `left` and `right` values stacked in each changed cell.
  - **Side by side** gives both full tables with a central change gutter.
- Side-by-side mode keeps the splitter fixed at the centre of the viewport; both panes
  share vertical and horizontal scrolling so they stay aligned, with one large shared
  horizontal scrollbar under the panes.
- The chosen layout and differences-only state are retained across rescans, reloads
  and column edits.
- Differences-only filter, search across cells, per-row deltas, and stats.
- **Differences only** also hides columns with no differences. Those columns are grouped at
  the bottom of the Columns menu; check one to override and show it while the filter is on.
- Very large cells are rendered from a bounded preview (the full value remains in the
  tooltip), and width sampling/intra-line spans are bounded so wide or huge columns do not
  stall the table.
- F7 / Shift+F7 change navigation steps through each differing cell in a row before
  moving to the next changed row, focusing and scrolling the cell into view. The
  counter shows the current changed-cell position and the total number of cells.
- Arrow keys move the selected cell through visible rows and columns. Home/End jump to the
  first/last visible column and PageUp/PageDown move ten rows; the selection follows
  filters and hidden columns.
- **Hide any column** from the table and comparison with one context-menu action.
- The **Columns (visible/total)** toolbar menu lists every column with a checkbox. Visible
  columns are checked; unchecking one hides it and moves it into the **Hidden columns**
  group at the bottom of the menu. A **Show all columns** action restores everything. The
  picker uses a larger window and a wide draggable scrollbar for long column lists.
- Hidden columns do not contribute to row alignment, cell differences, stats or exports.
- Hidden-column choices are remembered per open file for the current session.
- Right-click a column header or cell for hide/key/copy/delete actions, and right-click a row
  gutter to copy the left, right, or both rows as TSV.
- Right-click can also **delete the column from the left or right CSV file**, or from
  **both files at once**. This is confirmation-gated and writes a timestamped
  `.hcompare.bak` backup beside each rewritten file. Backup files are skipped by folder
  scans so they do not pollute the comparison.
- **Undo delete** (or `Ctrl+Z`) restores the last deleted column from its backup.
- **Restore deleted columns** lists the `.hcompare.bak` backups beside the open CSVs so any
  earlier state can be restored deliberately. A backup is removed from this list as soon as
  it has been restored, so the same restore is never offered twice in one session.
- Export the differences as CSV or the whole comparison as a standalone HTML report.

### Headless mode

Useful for scripts and CI:

```text
hcompare --report LEFT RIGHT
```

Exit code `1` means differences were found, `0` means identical, `2` means the comparison
could not run. `--all` includes build folders that are skipped by default.

## Build

Requirements:

- Rust 1.85 or newer.
- On Windows, the MSVC toolchain plus Visual Studio Build Tools and a Windows SDK.
  (`rustup default stable-msvc` is the usual setup.)

```powershell
cd tools/h-compare
cargo test
cargo build --release
```

The executable is `target/release/hcompare.exe` (or `target/release/hcompare`).

On Windows the exe embeds `assets/hcompare.ico` plus product/version metadata through
`winres`. Regenerate the icon with:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\make-icon.ps1
```

Legacy `.doc` comparison also needs one of LibreOffice, pandoc, antiword or catdoc
available on `PATH`; `.docx` and `.docm` work without external tools.

### Published builds

Ready-to-run portable executables are kept in `publish/` using the
`hcompare-vYY.MM.DD.N.exe` naming format. Each published build is accompanied by
`BUILD.txt` (source commit, toolchain, build time) and `SHA256SUMS.txt`.

The release task does the full flow in one command:

```powershell
cd tools/h-compare
.\scripts\release.ps1
```

It runs `cargo fmt --check`, `cargo test`, `cargo clippy -D warnings`, builds the
release binary, computes the next `vYY.MM.DD.N` version, updates `publish/`,
commits `h-compare`, and pushes to the current git branch. Use
`-Version v...`, `-Message "..."`, `-SkipTests`, `-IncludeUntracked`, or `-NoPush`
to override parts of the flow. Untracked files must be staged explicitly unless
`-IncludeUntracked` is passed.

Run during development:

```powershell
cargo run -- "C:\path\to\left" "C:\path\to\right"
cargo run -- "C:\path\to\left" "C:\path\to\right" --open=path/to/file.csv
```

CLI summary:

```text
Usage:
  hcompare [LEFT] [RIGHT]        Open the desktop comparison
  hcompare --report LEFT RIGHT   Compare headlessly and print a report

Options:
  -r, --report   Headless report (exit code 1 when differences exist)
      --all      Include common build folders in a report
      --open P   Open relative path P after the initial scan
      --theme T  Start with a theme: dark, light, premote, secretkey,
                 greystone, asphalt, wine, forest, soil, highcontrast
  -h, --help     Show this help
  -V, --version  Show the version and build stamp
```

`--version` prints the app version, build date and short git hash, for example
`hcompare 0.2.0 (build 2026-11-11 603553f)`. The About window shows the same build
stamp. This is the quickest way to confirm which executable is running on a machine.

## Keyboard shortcuts

| Key | Action |
| --- | --- |
| `F5` | Rescan both folders |
| `F7` / `Shift+F7` | Next / previous change (file, text hunk or CSV row) |
| `Arrow keys` | Move the CSV cell selection or text/PDF cursor |
| `Home` / `End` | First / last visible CSV column, or first / last text line |
| `PageUp` / `PageDown` | Move ten CSV rows or text lines |
| `Ctrl+F` | Find in the current text/CSV comparison |
| `Ctrl+B` | Collapse or expand the folder tree |
| `Enter` / `Shift+Enter` | Next / previous search match while searching |
| `Enter` | Compare the two folders when the search bar is closed and both paths are set |
| `Ctrl+C` | Copy the selected line, cell or row |
| `Ctrl+Z` | Undo the last deleted CSV column |
| `Escape` | Close search or dialog |

## Mouse / right-click

- **Folder tree**: open, copy A → B / B → A, reveal, delete, copy paths.
- **CSV header or cell**: copy values, hide the column, use the column as the alignment key,
  copy the column name/number, or delete the column from the left file, the right file, or
  both files.
- The **Columns (visible/total)** toolbar menu lets you hide and restore columns with
  checkboxes; hidden columns are grouped at the bottom of that menu.
- **CSV row gutter**: copy the left row, right row, or both rows as TSV.
- **Text diff line**: copy the line.

## Configuration

Settings are stored in:

```text
%APPDATA%\h-compare\settings.json
```

They include the last folders, text comparison options, CSV options, theme, and the
skip-build-folders preference. Passwords and credentials are never stored.

## Architecture

```text
src/
  main.rs        CLI parsing, window setup and the headless report
  app.rs         application state, background jobs, layout and actions
  model.rs       statuses, counts, comparison tree and formatting helpers
  folder.rs      recursive scan, byte comparison and tree construction
  textdiff.rs    line alignment, intra-line spans, encoding and search helpers
  csvdiff.rs     CSV parsing, key/row alignment and cell diffs
  word.rs        Word (.docx/.doc) text extraction
  load.rs        worker-thread file loading and binary detection
  report.rs      HTML and CSV report generation
  settings.rs    persisted settings
  ui/            theme, folder tree, text view, CSV view and shared widgets
```

## Limits

This is a focused implementation, not a byte-for-byte clone of Beyond Compare:

- Text/CSV diffs are limited to 64 MB per file. Larger binary files show metadata only.
- Binary comparison reports metadata and the first differing byte; it does not show a hex view.
- Folder sync copies files, not directories, and is intentionally confirmation-gated.
- Deleting a CSV column rewrites that file and normalizes CSV quoting; a timestamped
  `.hcompare.bak` backup is created first.
- `.docx`/`.docm` Word files are compared by extracted body text. Legacy `.doc` files need
  LibreOffice, pandoc, antiword or catdoc installed.
- Images, archives, three-way merge, syntax highlighting and plugin support are not implemented.
- CSV key alignment uses unique first-column values by default; duplicate-key files can be
  aligned by row order or an explicit key column.
