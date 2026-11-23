//! SFTP/FTP browsing and transfer engine (Phase 6b).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use hr_core::{AppError, AppResult, new_id};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Mutex, broadcast};

mod ftp;
mod sftp;

pub const EVENT_CAPACITY: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    /// Unix seconds when the server reports it.
    pub modified: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileListing {
    pub path: String,
    pub home: String,
    pub entries: Vec<FileEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSessionInfo {
    pub id: String,
    pub server_id: String,
    pub server_name: String,
    pub protocol: String,
    pub address: String,
    pub current_path: String,
    pub home_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransferDirection {
    Upload,
    Download,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransferState {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferSnapshot {
    pub id: String,
    pub session_id: String,
    pub direction: TransferDirection,
    pub remote_path: String,
    pub local_path: String,
    pub bytes: u64,
    pub total: u64,
    pub state: TransferState,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum FileEvent {
    Transfer { transfer: TransferSnapshot },
    Closed { session_id: String },
}

#[derive(Debug, Clone)]
pub struct FileConnectRequest {
    pub server_id: String,
    pub server_name: String,
    pub protocol: String,
    pub address: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub private_key: Option<String>,
    pub startup_path: String,
}

struct FileSession {
    info: FileSessionInfo,
    backend: Backend,
}

enum Backend {
    Sftp(sftp::SftpFs),
    Ftp(ftp::FtpFs),
}

impl Backend {
    fn home_path(&self) -> Option<String> {
        match self {
            Self::Sftp(client) => client.home_path(),
            Self::Ftp(client) => client.home_path(),
        }
    }

    async fn list(&mut self, path: &str) -> AppResult<Vec<FileEntry>> {
        match self {
            Self::Sftp(client) => client.list(path).await,
            Self::Ftp(client) => client.list(path).await,
        }
    }

    async fn rename(&mut self, from: &str, to: &str) -> AppResult<()> {
        match self {
            Self::Sftp(client) => client.rename(from, to).await,
            Self::Ftp(client) => client.rename(from, to).await,
        }
    }

    async fn delete(&mut self, path: &str, is_dir: bool) -> AppResult<()> {
        match self {
            Self::Sftp(client) => client.delete(path, is_dir).await,
            Self::Ftp(client) => client.delete(path, is_dir).await,
        }
    }

    async fn mkdir(&mut self, path: &str) -> AppResult<()> {
        match self {
            Self::Sftp(client) => client.mkdir(path).await,
            Self::Ftp(client) => client.mkdir(path).await,
        }
    }

    async fn upload(
        &mut self,
        local: &Path,
        remote: &str,
        cancel: &AtomicBool,
        progress: &mut (dyn FnMut(u64) + Send),
    ) -> AppResult<()> {
        match self {
            Self::Sftp(client) => client.upload(local, remote, cancel, progress).await,
            Self::Ftp(client) => client.upload(local, remote, cancel, progress).await,
        }
    }

    async fn download(
        &mut self,
        remote: &str,
        local: &Path,
        cancel: &AtomicBool,
        progress: &mut (dyn FnMut(u64) + Send),
    ) -> AppResult<()> {
        match self {
            Self::Sftp(client) => client.download(remote, local, cancel, progress).await,
            Self::Ftp(client) => client.download(remote, local, cancel, progress).await,
        }
    }

    async fn close(self) -> AppResult<()> {
        match self {
            Self::Sftp(client) => client.close().await,
            Self::Ftp(client) => client.close().await,
        }
    }
}

pub struct FileManager {
    sessions: Mutex<HashMap<String, Arc<Mutex<FileSession>>>>,
    transfers: Mutex<HashMap<String, TransferSnapshot>>,
    cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    events: broadcast::Sender<FileEvent>,
}

impl Default for FileManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FileManager {
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        Self {
            sessions: Mutex::new(HashMap::new()),
            transfers: Mutex::new(HashMap::new()),
            cancels: Mutex::new(HashMap::new()),
            events,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<FileEvent> {
        self.events.subscribe()
    }

    pub async fn connect(&self, request: FileConnectRequest) -> AppResult<FileSessionInfo> {
        let protocol = request.protocol.to_ascii_uppercase();
        let backend = match protocol.as_str() {
            "SFTP" => Backend::Sftp(sftp::SftpFs::connect(&request).await?),
            "FTP" => Backend::Ftp(ftp::FtpFs::connect(&request).await?),
            other => {
                return Err(AppError::unsupported(format!(
                    "{other} file transfer is not available"
                )));
            }
        };
        let id = new_id();
        let home = backend
            .home_path()
            .unwrap_or_else(|| request.startup_path.clone());
        let current_path = if request.startup_path.trim().is_empty() {
            home.clone()
        } else {
            request.startup_path.clone()
        };
        let info = FileSessionInfo {
            id: id.clone(),
            server_id: request.server_id,
            server_name: request.server_name,
            protocol,
            address: format!("{}:{}", request.address, request.port),
            current_path,
            home_path: home,
        };
        self.sessions.lock().await.insert(
            id.clone(),
            Arc::new(Mutex::new(FileSession {
                info: info.clone(),
                backend,
            })),
        );
        Ok(info)
    }

    pub async fn infos(&self) -> Vec<FileSessionInfo> {
        let sessions = self.sessions.lock().await;
        sessions
            .values()
            .filter_map(|session| session.try_lock().ok().map(|guard| guard.info.clone()))
            .collect()
    }

    pub async fn info(&self, id: &str) -> AppResult<FileSessionInfo> {
        let session = self.session(id).await?;
        Ok(session.lock().await.info.clone())
    }

    pub async fn list(&self, id: &str, path: Option<String>) -> AppResult<FileListing> {
        let session = self.session(id).await?;
        let mut guard = session.lock().await;
        let path = match path {
            Some(path) if !path.trim().is_empty() => path,
            _ => match guard.info.current_path.trim() {
                "" => guard.info.home_path.clone(),
                current => current.to_string(),
            },
        };
        let entries = guard.backend.list(&path).await?;
        guard.info.current_path = path.clone();
        Ok(FileListing {
            path,
            home: guard.info.home_path.clone(),
            entries,
        })
    }

    pub async fn rename(&self, id: &str, from: &str, to: &str) -> AppResult<()> {
        self.session(id)
            .await?
            .lock()
            .await
            .backend
            .rename(from, to)
            .await
    }

    pub async fn delete(&self, id: &str, path: &str, is_dir: bool) -> AppResult<()> {
        self.session(id)
            .await?
            .lock()
            .await
            .backend
            .delete(path, is_dir)
            .await
    }

    pub async fn mkdir(&self, id: &str, path: &str) -> AppResult<()> {
        self.session(id)
            .await?
            .lock()
            .await
            .backend
            .mkdir(path)
            .await
    }

    pub async fn shutdown(&self) {
        let sessions: Vec<_> = self.sessions.lock().await.drain().map(|(_, s)| s).collect();
        for session in sessions {
            if let Ok(session) = Arc::try_unwrap(session) {
                let _ = session.into_inner().backend.close().await;
            }
        }
        for cancel in self.cancels.lock().await.values() {
            cancel.store(true, Ordering::Release);
        }
    }

    pub async fn close(&self, id: &str) -> AppResult<()> {
        if let Some(session) = self.sessions.lock().await.remove(id) {
            if let Ok(session) = Arc::try_unwrap(session) {
                let _ = session.into_inner().backend.close().await;
            }
            let _ = self.events.send(FileEvent::Closed {
                session_id: id.to_string(),
            });
        }
        Ok(())
    }

    pub async fn start_upload(
        self: &Arc<Self>,
        id: &str,
        local_paths: Vec<String>,
        remote_dir: String,
    ) -> AppResult<Vec<TransferSnapshot>> {
        self.start(
            id,
            local_paths.into_iter().map(PathBuf::from).collect(),
            remote_dir,
            TransferDirection::Upload,
        )
        .await
    }

    pub async fn start_download(
        self: &Arc<Self>,
        id: &str,
        remote_paths: Vec<String>,
        local_dir: String,
    ) -> AppResult<Vec<TransferSnapshot>> {
        self.start(
            id,
            remote_paths.into_iter().map(PathBuf::from).collect(),
            local_dir,
            TransferDirection::Download,
        )
        .await
    }

    async fn start(
        self: &Arc<Self>,
        id: &str,
        paths: Vec<PathBuf>,
        target: String,
        direction: TransferDirection,
    ) -> AppResult<Vec<TransferSnapshot>> {
        let session = self.session(id).await?;
        let mut snapshots = Vec::with_capacity(paths.len());
        for path in paths {
            let transfer_id = new_id();
            let cancel = Arc::new(AtomicBool::new(false));
            let (remote, local) = match direction {
                TransferDirection::Upload => {
                    let name = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .ok_or_else(|| AppError::validation("the local file has no name"))?;
                    (join_path(&target, name), path.display().to_string())
                }
                TransferDirection::Download => {
                    let name = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .ok_or_else(|| AppError::validation("the remote file has no name"))?;
                    (
                        path.display().to_string(),
                        Path::new(&target).join(name).display().to_string(),
                    )
                }
            };
            let total = match direction {
                TransferDirection::Upload => tokio::fs::metadata(&path)
                    .await
                    .map(|metadata| metadata.len())
                    .unwrap_or(0),
                TransferDirection::Download => 0,
            };
            let snapshot = TransferSnapshot {
                id: transfer_id.clone(),
                session_id: id.to_string(),
                direction,
                remote_path: remote.clone(),
                local_path: local.clone(),
                bytes: 0,
                total,
                state: TransferState::Running,
                message: None,
            };
            self.transfers
                .lock()
                .await
                .insert(transfer_id.clone(), snapshot.clone());
            self.cancels
                .lock()
                .await
                .insert(transfer_id.clone(), Arc::clone(&cancel));
            let _ = self.events.send(FileEvent::Transfer {
                transfer: snapshot.clone(),
            });
            snapshots.push(snapshot);

            let manager = Arc::clone(self);
            let session = Arc::clone(&session);
            let path = path.clone();
            tokio::spawn(async move {
                manager
                    .run_transfer(transfer_id, session, path, remote, local, direction, cancel)
                    .await;
            });
        }
        Ok(snapshots)
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_transfer(
        self: Arc<Self>,
        id: String,
        session: Arc<Mutex<FileSession>>,
        source: PathBuf,
        remote: String,
        local: String,
        direction: TransferDirection,
        cancel: Arc<AtomicBool>,
    ) {
        let outcome = {
            let mut guard = session.lock().await;
            let progress_manager = Arc::clone(&self);
            let progress_id = id.clone();
            let mut progress = move |bytes: u64| {
                let manager = Arc::clone(&progress_manager);
                let transfer_id = progress_id.clone();
                tokio::spawn(async move {
                    manager.update_progress(&transfer_id, bytes).await;
                });
            };
            match direction {
                TransferDirection::Upload => {
                    guard
                        .backend
                        .upload(&source, &remote, &cancel, &mut progress)
                        .await
                }
                TransferDirection::Download => {
                    guard
                        .backend
                        .download(&remote, Path::new(&local), &cancel, &mut progress)
                        .await
                }
            }
        };
        let cancelled = cancel.load(Ordering::Acquire);
        let snapshot = {
            let mut transfers = self.transfers.lock().await;
            let snapshot = transfers.get_mut(&id);
            match snapshot {
                Some(snapshot) => {
                    snapshot.state = if cancelled {
                        TransferState::Cancelled
                    } else if outcome.is_ok() {
                        TransferState::Completed
                    } else {
                        TransferState::Failed
                    };
                    snapshot.message = outcome.err().map(|error| error.to_string());
                    if snapshot.total > 0 {
                        snapshot.bytes = snapshot.bytes.min(snapshot.total);
                    }
                    snapshot.clone()
                }
                None => return,
            }
        };
        self.cancels.lock().await.remove(&id);
        let _ = self.events.send(FileEvent::Transfer { transfer: snapshot });
    }

    async fn update_progress(&self, id: &str, bytes: u64) {
        let snapshot = {
            let mut transfers = self.transfers.lock().await;
            let snapshot = transfers.get_mut(id);
            match snapshot {
                Some(snapshot) if snapshot.state == TransferState::Running => {
                    snapshot.bytes = bytes;
                    Some(snapshot.clone())
                }
                _ => None,
            }
        };
        if let Some(transfer) = snapshot {
            let _ = self.events.send(FileEvent::Transfer { transfer });
        }
    }

    pub async fn transfers(&self) -> Vec<TransferSnapshot> {
        self.transfers.lock().await.values().cloned().collect()
    }

    pub async fn cancel_transfer(&self, id: &str) -> bool {
        if let Some(cancel) = self.cancels.lock().await.get(id) {
            cancel.store(true, Ordering::Release);
            return true;
        }
        false
    }

    async fn session(&self, id: &str) -> AppResult<Arc<Mutex<FileSession>>> {
        self.sessions
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::not_found("file session"))
    }
}

fn join_path(directory: &str, name: &str) -> String {
    if directory.ends_with('/') {
        format!("{directory}{name}")
    } else {
        format!("{directory}/{name}")
    }
}

/// Reads a file in chunks while reporting progress and checking cancellation.
pub(crate) async fn copy_with_progress<R, W>(
    reader: &mut R,
    writer: &mut W,
    total: u64,
    cancel: &AtomicBool,
    progress: &mut (dyn FnMut(u64) + Send),
) -> AppResult<u64>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut buffer = vec![0u8; 64 * 1024];
    let mut written = 0u64;
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err(AppError::unavailable("transfer cancelled"));
        }
        let read = reader
            .read(&mut buffer)
            .await
            .map_err(|error| AppError::platform(format!("cannot read: {error}")))?;
        if read == 0 {
            break;
        }
        writer
            .write_all(&buffer[..read])
            .await
            .map_err(|error| AppError::platform(format!("cannot write: {error}")))?;
        written += read as u64;
        progress(written.min(total.max(written)));
    }
    writer
        .flush()
        .await
        .map_err(|error| AppError::platform(format!("cannot flush: {error}")))?;
    Ok(written)
}

pub(crate) fn epoch_seconds(time: std::time::SystemTime) -> Option<i64> {
    time.duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_path_handles_trailing_slashes() {
        assert_eq!(join_path("/home/user", "file"), "/home/user/file");
        assert_eq!(join_path("/home/user/", "file"), "/home/user/file");
        assert_eq!(join_path("C:/data", "a.txt"), "C:/data/a.txt");
    }

    #[test]
    fn transfer_snapshot_serialises_camel_case() {
        let snapshot = TransferSnapshot {
            id: "t".into(),
            session_id: "s".into(),
            direction: TransferDirection::Upload,
            remote_path: "/a".into(),
            local_path: "C:/a".into(),
            bytes: 4,
            total: 8,
            state: TransferState::Running,
            message: None,
        };
        let json = serde_json::to_value(snapshot).unwrap();
        assert_eq!(json["direction"], "upload");
        assert_eq!(json["state"], "running");
        assert_eq!(json["remotePath"], "/a");
    }

    #[test]
    fn copy_progress_reports_and_cancels() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let mut source = &b"abcdefgh"[..];
            let mut target = Vec::new();
            let cancel = AtomicBool::new(false);
            let mut seen = 0;
            let mut progress = |bytes| seen = bytes;
            let written = copy_with_progress(&mut source, &mut target, 8, &cancel, &mut progress)
                .await
                .unwrap();
            assert_eq!(written, 8);
            assert_eq!(target, b"abcdefgh");
            assert_eq!(seen, 8);

            let cancel = AtomicBool::new(true);
            let mut source = &b"abc"[..];
            let mut target = Vec::new();
            let mut progress = |_| {};
            assert!(
                copy_with_progress(&mut source, &mut target, 3, &cancel, &mut progress)
                    .await
                    .is_err()
            );
        });
    }
}
