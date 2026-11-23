use std::path::Path;

use hr_core::{AppError, AppResult};
use suppaftp::AsyncFtpStream;

use crate::{FileConnectRequest, FileEntry, copy_with_progress, epoch_seconds, join_path};

pub struct FtpFs {
    stream: AsyncFtpStream,
    home: String,
}

impl FtpFs {
    pub async fn connect(request: &FileConnectRequest) -> AppResult<Self> {
        let mut stream = AsyncFtpStream::connect((request.address.as_str(), request.port))
            .await
            .map_err(|error| AppError::platform(format!("cannot reach the FTP server: {error}")))?;
        let username = if request.username.trim().is_empty() {
            "anonymous"
        } else {
            request.username.as_str()
        };
        let password = if request.password.is_empty() {
            "anonymous@"
        } else {
            request.password.as_str()
        };
        stream
            .login(username, password)
            .await
            .map_err(|error| AppError::platform(format!("FTP login failed: {error}")))?;
        let path = if request.startup_path.trim().is_empty() {
            "/"
        } else {
            request.startup_path.as_str()
        };
        stream
            .cwd(path)
            .await
            .map_err(|error| AppError::platform(format!("cannot enter '{path}': {error}")))?;
        let home = stream.pwd().await.unwrap_or_else(|_| path.to_string());
        Ok(Self { stream, home })
    }

    pub fn home_path(&self) -> Option<String> {
        Some(self.home.clone())
    }

    pub async fn list(&mut self, path: &str) -> AppResult<Vec<FileEntry>> {
        self.stream
            .cwd(path)
            .await
            .map_err(|error| AppError::platform(format!("cannot enter '{path}': {error}")))?;
        let lines = self
            .stream
            .list(None)
            .await
            .map_err(|error| AppError::platform(format!("cannot list '{path}': {error}")))?;
        let mut entries = Vec::new();
        for line in lines {
            let Ok(file) = suppaftp::list::File::try_from(line.as_str()) else {
                continue;
            };
            let name = file.name().to_string();
            if name == "." || name == ".." {
                continue;
            }
            entries.push(FileEntry {
                name: name.clone(),
                path: join_path(path, &name),
                is_dir: file.is_directory(),
                size: file.size() as u64,
                modified: epoch_seconds(file.modified()),
            });
        }
        entries.sort_by(|left, right| {
            right
                .is_dir
                .cmp(&left.is_dir)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });
        Ok(entries)
    }

    pub async fn rename(&mut self, from: &str, to: &str) -> AppResult<()> {
        self.stream
            .rename(from, to)
            .await
            .map_err(|error| AppError::platform(format!("cannot rename: {error}")))
    }

    pub async fn delete(&mut self, path: &str, is_dir: bool) -> AppResult<()> {
        if is_dir {
            self.stream
                .rmdir(path)
                .await
                .map_err(|error| AppError::platform(format!("cannot remove the folder: {error}")))
        } else {
            self.stream
                .rm(path)
                .await
                .map_err(|error| AppError::platform(format!("cannot delete: {error}")))
        }
    }

    pub async fn mkdir(&mut self, path: &str) -> AppResult<()> {
        self.stream
            .mkdir(path)
            .await
            .map_err(|error| AppError::platform(format!("cannot create the folder: {error}")))
    }

    pub async fn upload(
        &mut self,
        local: &Path,
        remote: &str,
        cancel: &std::sync::atomic::AtomicBool,
        progress: &mut (dyn FnMut(u64) + Send),
    ) -> AppResult<()> {
        let mut source = tokio::fs::File::open(local).await.map_err(|error| {
            AppError::platform(format!("cannot open {}: {error}", local.display()))
        })?;
        let total = source.metadata().await.map(|meta| meta.len()).unwrap_or(0);
        let mut stream =
            self.stream.put_with_stream(remote).await.map_err(|error| {
                AppError::platform(format!("cannot create '{remote}': {error}"))
            })?;
        {
            use tokio::io::AsyncWriteExt;
            use tokio_util::compat::FuturesAsyncWriteCompatExt;
            let mut compat = (&mut stream).compat_write();
            copy_with_progress(&mut source, &mut compat, total, cancel, progress).await?;
            compat
                .flush()
                .await
                .map_err(|error| AppError::platform(format!("cannot finish upload: {error}")))?;
        }
        self.stream
            .finalize_put_stream(stream)
            .await
            .map_err(|error| AppError::platform(format!("upload failed: {error}")))?;
        Ok(())
    }

    pub async fn download(
        &mut self,
        remote: &str,
        local: &Path,
        cancel: &std::sync::atomic::AtomicBool,
        progress: &mut (dyn FnMut(u64) + Send),
    ) -> AppResult<()> {
        let total = self.stream.size(remote).await.unwrap_or(0) as u64;
        let mut stream = self
            .stream
            .retr_as_stream(remote)
            .await
            .map_err(|error| AppError::platform(format!("cannot open '{remote}': {error}")))?;
        if let Some(parent) = local.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        let mut target = tokio::fs::File::create(local).await.map_err(|error| {
            AppError::platform(format!("cannot create {}: {error}", local.display()))
        })?;
        {
            use tokio_util::compat::FuturesAsyncReadCompatExt;
            let mut compat = (&mut stream).compat();
            copy_with_progress(&mut compat, &mut target, total, cancel, progress).await?;
        }
        self.stream
            .finalize_retr_stream(stream)
            .await
            .map_err(|error| AppError::platform(format!("download failed: {error}")))?;
        Ok(())
    }

    pub async fn close(mut self) -> AppResult<()> {
        let _ = self.stream.quit().await;
        Ok(())
    }
}
