//! Import servers from a PRemoteM/1Remote SQLite database (P2-63).

use std::path::Path;

use hr_core::{AppError, AppResult, new_id};
use hr_domain::{ImportResult, ProtocolKind, Server, normalize_tags};
use serde_json::Value;
use sqlx::Row;

use crate::Database;

pub async fn import_database(path: &Path) -> AppResult<ImportResult> {
    let database = Database::open_sqlite_read_only(path).await?;
    let result = import_from_pool(database.pool().clone()).await;
    database.close().await;
    result
}

async fn import_from_pool(pool: sqlx::SqlitePool) -> AppResult<ImportResult> {
    let rows = sqlx::query("SELECT Protocol, ClassVersion, Json FROM Servers")
        .fetch_all(&pool)
        .await
        .map_err(|error| AppError::storage(format!("cannot read legacy Servers table: {error}")))?;
    let mut result = ImportResult::default();
    for (index, row) in rows.iter().enumerate() {
        let protocol: String = row
            .try_get("Protocol")
            .or_else(|_| row.try_get("protocol"))
            .unwrap_or_default();
        let json: String = row
            .try_get("Json")
            .or_else(|_| row.try_get("json"))
            .unwrap_or_default();
        let mut server = if let Ok(mut server) = serde_json::from_str::<Server>(&json) {
            server.base_mut().id = new_id();
            server.base_mut().session_id = None;
            server
        } else {
            match legacy_value_to_server(&protocol, &json) {
                Ok(server) => server,
                Err(error) => {
                    result.warnings.push(format!("row {}: {error}", index + 1));
                    continue;
                }
            }
        };
        server.normalize();
        let issues = server.validate();
        if issues.is_empty() {
            result.servers.push(server);
        } else {
            result.warnings.push(format!(
                "row {} skipped: {}",
                index + 1,
                issues
                    .iter()
                    .map(|issue| issue.message.clone())
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
    }
    Ok(result)
}

fn legacy_value_to_server(protocol: &str, json: &str) -> Result<Server, String> {
    let value: Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
    let kind = ProtocolKind::from_id(protocol)
        .or_else(|| {
            value
                .get("Protocol")
                .and_then(Value::as_str)
                .or_else(|| value.get("protocol").and_then(Value::as_str))
                .and_then(ProtocolKind::from_id)
        })
        .ok_or_else(|| format!("unsupported protocol '{protocol}'"))?;
    let display_name = string_field(&value, &["DisplayName", "displayName", "Name", "name"])
        .unwrap_or_else(|| format!("Imported {}", kind.id()));
    let address = string_field(&value, &["Address", "address"]).unwrap_or_default();
    let port = number_field(&value, &["Port", "port"]).unwrap_or(0) as u16;
    let username =
        string_field(&value, &["UserName", "Username", "username", "UserName"]).unwrap_or_default();
    let mut server = match kind {
        ProtocolKind::Rdp => {
            Server::new_rdp(display_name, address, if port == 0 { 3389 } else { port })
        }
        ProtocolKind::RemoteApp => Server::new_remote_app(
            display_name,
            address,
            string_field(&value, &["RemoteApplicationProgram"]).unwrap_or_default(),
        ),
        ProtocolKind::Vnc => {
            Server::new_vnc(display_name, address, if port == 0 { 5900 } else { port })
        }
        ProtocolKind::Ssh => Server::new_ssh(display_name, address, username.clone()),
        ProtocolKind::Telnet => {
            Server::new_telnet(display_name, address, if port == 0 { 23 } else { port })
        }
        ProtocolKind::Serial => Server::new_serial(
            display_name,
            string_field(&value, &["SerialPort"]).unwrap_or_else(|| "COM1".into()),
        ),
        ProtocolKind::Sftp => Server::new_sftp(display_name, address, username.clone()),
        ProtocolKind::Ftp => Server::new_ftp(display_name, address, username.clone()),
        ProtocolKind::LocalApp => Server::new_local_app(
            display_name,
            string_field(&value, &["ExePath"]).unwrap_or_default(),
        ),
        ProtocolKind::Custom => Server::new_custom(display_name, protocol),
        ProtocolKind::Dummy => Server::new_dummy(display_name),
    };
    if let Some(credentials) = server.credentials_mut() {
        if credentials.username.is_empty() {
            credentials.username = username;
        }
        if let Some(password) = string_field(&value, &["Password", "password"]) {
            // Legacy encrypted values are unreadable; only plain values are kept.
            if !looks_encrypted(&password) {
                credentials.password = password.into();
            }
        }
    }
    if let Some(tags) = value.get("Tags").and_then(Value::as_array).map(|tags| {
        tags.iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>()
    }) {
        server.base_mut().tags = normalize_tags(tags);
    }
    if let Some(tree) = value
        .get("TreeNodes")
        .and_then(Value::as_array)
        .map(|nodes| {
            nodes
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
    {
        server.base_mut().tree_nodes = tree;
    }
    if let Some(note) = string_field(&value, &["Note", "note"]) {
        server.base_mut().note = note;
    }
    server.base_mut().id = new_id();
    Ok(server)
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::to_string)
}

fn number_field(value: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_i64))
}

fn looks_encrypted(value: &str) -> bool {
    value.starts_with("v1:")
        || value.starts_with("u3")
        || value.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Repository;
    use crate::codec::tests::test_vault;
    use hr_domain::Server;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn imports_our_exported_database() {
        let dir = TempDir::new().unwrap();
        let source = dir.path().join("source.db");
        let database = Database::open_sqlite(&source).await.unwrap();
        let vault = test_vault(dir.path());
        let repository = Repository::open(
            database.pool().clone(),
            Arc::new(crate::VaultCodec::new(vault)),
        )
        .await
        .unwrap();
        repository
            .upsert_server(Server::new_ssh("Imported SSH", "10.0.0.1", "root"))
            .await
            .unwrap();
        database.close().await;

        let result = import_database(&source).await.unwrap();
        assert_eq!(result.servers.len(), 1);
        assert_eq!(result.servers[0].base().display_name, "Imported SSH");
        assert!(result.warnings.is_empty());
    }

    #[tokio::test]
    async fn imports_legacy_json_rows() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("legacy.db");
        let database = Database::open_sqlite(&path).await.unwrap();
        sqlx::query("INSERT INTO Servers (Id, Protocol, ClassVersion, Json, UpdatedAt) VALUES ('x','RDP',1,?,0)")
            .bind(r#"{"DisplayName":"Old RDP","Address":"10.0.0.2","Port":3390,"UserName":"admin","Tags":["prod"],"TreeNodes":["Prod"]}"#)
            .execute(database.pool())
            .await
            .unwrap();
        database.close().await;

        let result = import_database(&path).await.unwrap();
        assert_eq!(result.servers.len(), 1);
        match &result.servers[0] {
            Server::Rdp(rdp) => {
                assert_eq!(rdp.address.address, "10.0.0.2");
                assert_eq!(rdp.address.port, 3390);
                assert_eq!(rdp.credentials.username, "admin");
            }
            _ => panic!("expected RDP"),
        }
    }
}
