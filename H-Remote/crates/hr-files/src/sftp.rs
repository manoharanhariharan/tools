use std::path::Path;
use std::sync::Arc;

use hr_core::{AppError, AppResult};
use russh::client::AuthResult;
use russh::keys::PrivateKeyWithHashAlg;
use russh_sftp::client::SftpSession;
use tokio::io::AsyncWriteExt;

use crate::{FileConnectRequest, FileEntry, copy_with_progress, join_path};

pub struct SftpFs {
    handle: russh::client::Handle<Client>,
    sftp: SftpSession,
    home: String,
}

struct Client;

impl russh::client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

impl SftpFs {
    pub async fn connect(request: &FileConnectRequest) -> AppResult<Self> {
        let config = Arc::new(russh::client::Config::default());
        let mut handle =
            russh::client::connect(config, (request.address.as_str(), request.port), Client)
                .await
                .map_err(|error| {
                    AppError::platform(format!("cannot reach the SFTP server: {error}"))
                })?;

        let auth = if let Some(path) = request
            .private_key
            .as_deref()
            .filter(|path| !path.trim().is_empty())
        {
            let key = russh::keys::load_secret_key(Path::new(path), None).map_err(|error| {
                AppError::validation(format!("cannot read the private key: {error}"))
            })?;
            handle
                .authenticate_publickey(
                    request.username.clone(),
                    PrivateKeyWithHashAlg::new(Arc::new(key), None),
                )
                .await
                .map_err(|error| {
                    AppError::platform(format!("SSH key authentication failed: {error}"))
                })?
        } else {
            handle
                .authenticate_password(request.username.clone(), request.password.clone())
                .await
                .map_err(|error| {
                    AppError::platform(format!("SSH authentication failed: {error}"))
                })?
        };
        if !matches!(auth, AuthResult::Success) {
            return Err(AppError::platform(
                "SSH authentication was rejected by the server",
            ));
        }

        let channel = handle.channel_open_session().await.map_err(|error| {
            AppError::platform(format!("cannot open the SFTP channel: {error}"))
        })?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|error| {
                AppError::platform(format!("cannot start the SFTP subsystem: {error}"))
            })?;
        let sftp = SftpSession::new(channel.into_stream())
            .await
            .map_err(|error| AppError::platform(format!("SFTP handshake failed: {error}")))?;
        let home = if request.startup_path.trim().is_empty() {
            sftp.canonicalize(".")
                .await
                .map(|path| path.to_string())
                .unwrap_or_else(|_| "/".to_string())
        } else {
            request.startup_path.clone()
        };
        Ok(Self { handle, sftp, home })
    }

    pub fn home_path(&self) -> Option<String> {
        Some(self.home.clone())
    }

    pub async fn list(&mut self, path: &str) -> AppResult<Vec<FileEntry>> {
        let entries = self
            .sftp
            .read_dir(path)
            .await
            .map_err(|error| AppError::platform(format!("cannot list '{path}': {error}")))?;
        let mut result = Vec::new();
        for entry in entries {
            let name = entry.file_name().to_string();
            if name == "." || name == ".." {
                continue;
            }
            let metadata = entry.metadata();
            result.push(FileEntry {
                name: name.clone(),
                path: join_path(path, &name),
                is_dir: metadata.is_dir(),
                size: metadata.size.unwrap_or(0),
                modified: metadata.mtime.map(i64::from),
            });
        }
        result.sort_by(|left, right| {
            right
                .is_dir
                .cmp(&left.is_dir)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });
        Ok(result)
    }

    pub async fn rename(&mut self, from: &str, to: &str) -> AppResult<()> {
        self.sftp
            .rename(from, to)
            .await
            .map_err(|error| AppError::platform(format!("cannot rename: {error}")))
    }

    pub async fn delete(&mut self, path: &str, is_dir: bool) -> AppResult<()> {
        if is_dir {
            self.sftp
                .remove_dir(path)
                .await
                .map_err(|error| AppError::platform(format!("cannot remove the folder: {error}")))
        } else {
            self.sftp
                .remove_file(path)
                .await
                .map_err(|error| AppError::platform(format!("cannot delete: {error}")))
        }
    }

    pub async fn mkdir(&mut self, path: &str) -> AppResult<()> {
        self.sftp
            .create_dir(path)
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
        let mut target =
            self.sftp.create(remote).await.map_err(|error| {
                AppError::platform(format!("cannot create '{remote}': {error}"))
            })?;
        copy_with_progress(&mut source, &mut target, total, cancel, progress).await?;
        target
            .shutdown()
            .await
            .map_err(|error| AppError::platform(format!("cannot finish upload: {error}")))?;
        Ok(())
    }

    pub async fn download(
        &mut self,
        remote: &str,
        local: &Path,
        cancel: &std::sync::atomic::AtomicBool,
        progress: &mut (dyn FnMut(u64) + Send),
    ) -> AppResult<()> {
        let total = self
            .sftp
            .metadata(remote)
            .await
            .ok()
            .and_then(|metadata| metadata.size)
            .unwrap_or(0);
        let mut source = self
            .sftp
            .open(remote)
            .await
            .map_err(|error| AppError::platform(format!("cannot open '{remote}': {error}")))?;
        if let Some(parent) = local.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        let mut target = tokio::fs::File::create(local).await.map_err(|error| {
            AppError::platform(format!("cannot create {}: {error}", local.display()))
        })?;
        copy_with_progress(&mut source, &mut target, total, cancel, progress).await?;
        Ok(())
    }

    pub async fn close(self) -> AppResult<()> {
        let _ = self
            .handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
        Ok(())
    }
}
