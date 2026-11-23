# Phase 6b — SFTP/FTP file transfer

**Status: complete (2026-11-22).** Remote browsing, navigation, rename/delete,
upload/download with progress and cancellation are implemented for SFTP and FTP,
integrated as file-transfer tabs in the workspace.

## What shipped

### `hr-files`

- `FileManager` owns connected file sessions and transfer tasks, with a bounded
  event broadcast and deterministic shutdown.
- **SFTP** (`russh` + `russh-sftp`): password or private-key authentication,
  SFTP subsystem, home canonicalisation, directory listing, rename, delete,
  mkdir, streaming upload/download with progress.
- **FTP** (`suppaftp` async): anonymous or user login, startup path (`CWD`),
  LIST parsing, rename/delete/mkdir, streaming upload/download with progress.
- Bounded 64 KiB copy loop checks a cancellation flag between chunks and reports
  bytes transferred.
- Temporary/runtime data is never written except to the chosen local/download
  destination; connection state is dropped on close/shutdown.

### Tauri

- `file_list`, `file_rename`, `file_delete`, `file_mkdir`, `file_upload`,
  `file_download`, `file_upload_dialog`, `file_download_dialog`,
  `file_transfers`, `file_cancel_transfer`.
- `connect_server`/`quick_connect` route SFTP/FTP to the file manager and return
  a normal `SessionSnapshot` (`backend: fileTransfer`), so file sessions appear
  as workspace tabs. `session_list` merges file sessions; cancel/remove/reconnect
  dispatch to the file manager.
- `file:event` forwards transfer progress to every window.
- Native file/folder pickers are used for upload/download dialogs.

### UI

- `FileBrowser.svelte` inside a session tab:
  - path bar (home/up/go/refresh), folder double-click navigation;
  - columns for name, size, modified, type; sort asc/desc; folders first;
  - six persisted date formats;
  - multi-select, upload, download, rename, delete with confirmation, new folder;
  - transfer panel with direction, percentage/state and cancel;
  - empty/loading/error states and a fixed-height scroll area.
- Catalog/launcher connect all supported protocols instead of RDP only.

## Defaults and decisions

- Download uses the folder picker; upload uses the file picker. The browser
  never guesses a destination path.
- Directory download is not implemented yet; the UI requires files for
  download and reports this clearly.
- SFTP private keys are loaded for the connection only; no temporary key files
  are written (unlike the reference PuTTY key-copy flow).
- FTP uses plain FTP (no FTPS) in this phase; SFTP is preferred.

## Deviations

1. **P6-13** back/forward history is not implemented; home/up/path navigation is.
2. **P6-14** clipboard-file upload is not implemented; files come from the
   native picker. Download directories are not recursive yet.
3. **P6-15** rename uses a dialog rather than inline cell editing.
4. **P6-16** native OS drag-and-drop paths are not available in the webview
   without a Tauri drag event bridge; drag upload/download remains deferred.
5. **P6-17** transfer list/progress/cancel work; “open destination folder” is
   not yet wired.
6. FTPS is deferred; plain FTP and SFTP only.

## Verification

- `hr-files` unit tests: path joining, transfer serialisation, progress and
  cancellation.
- UI: `FileBrowser.test.ts` listing/navigation/sorting/selection.
- E2E: SFTP tab opens, lists the startup path, navigates into a folder and
  closes.
- `cargo test --workspace`: 300 tests.
- UI: 73 Vitest tests; E2E: 13 Playwright tests.
