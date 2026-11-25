//! Encryption-at-rest codec for JSON columns.
//!
//! Only secret-bearing fields are encrypted; the surrounding JSON remains
//! readable so later migrations can operate on non-secret metadata.

use std::sync::Arc;

use hr_core::{AppError, AppResult};
use hr_crypto::Vault;
use hr_domain::{Credential, Server};
use serde_json::{Map, Value};

pub const ENC_KEY: &str = "$enc";
pub const PURPOSE_SERVER_PASSWORD: &str = "server-password";
pub const PURPOSE_SERVER_PRIVATE_KEY: &str = "server-private-key";
pub const PURPOSE_GATEWAY_PASSWORD: &str = "gateway-password";
pub const PURPOSE_ALTERNATE_PASSWORD: &str = "alternate-password";
pub const PURPOSE_ALTERNATE_PRIVATE_KEY: &str = "alternate-private-key";
pub const PURPOSE_ARGUMENT_SECRET: &str = "argument-secret";
pub const PURPOSE_CREDENTIAL_PASSWORD: &str = "credential-password";
pub const PURPOSE_CREDENTIAL_PRIVATE_KEY: &str = "credential-private-key";

/// Abstraction so tests and future key providers share the repository path.
pub trait SecretCodec: Send + Sync {
    fn encrypt(&self, purpose: &str, plaintext: &str) -> AppResult<String>;
    fn decrypt(&self, purpose: &str, encoded: &str) -> AppResult<String>;
    fn create_sentinel(&self) -> AppResult<String>;
    fn verify_sentinel(&self, encoded: &str) -> AppResult<()>;
}

pub struct VaultCodec {
    vault: Arc<Vault>,
}

impl VaultCodec {
    pub fn new(vault: Arc<Vault>) -> Self {
        Self { vault }
    }
}

impl SecretCodec for VaultCodec {
    fn encrypt(&self, purpose: &str, plaintext: &str) -> AppResult<String> {
        self.vault.encrypt_string(purpose, plaintext)
    }

    fn decrypt(&self, purpose: &str, encoded: &str) -> AppResult<String> {
        self.vault
            .decrypt_string(purpose, encoded)
            .map(|value| value.to_string())
    }

    fn create_sentinel(&self) -> AppResult<String> {
        self.vault.create_sentinel()
    }

    fn verify_sentinel(&self, encoded: &str) -> AppResult<()> {
        self.vault.verify_sentinel(encoded)
    }
}

pub fn seal_server(server: &Server, codec: &dyn SecretCodec) -> AppResult<String> {
    let mut value = serde_json::to_value(server)?;
    if let Value::Object(object) = &mut value {
        seal_field(object, "password", PURPOSE_SERVER_PASSWORD, codec)?;
        seal_field(object, "privateKey", PURPOSE_SERVER_PRIVATE_KEY, codec)?;
        seal_field(object, "gatewayPassword", PURPOSE_GATEWAY_PASSWORD, codec)?;
    }
    if let Some(alternates) = value
        .get_mut("alternateCredentials")
        .and_then(Value::as_array_mut)
    {
        for alternate in alternates {
            if let Value::Object(object) = alternate {
                seal_field(object, "password", PURPOSE_ALTERNATE_PASSWORD, codec)?;
                seal_field(object, "privateKey", PURPOSE_ALTERNATE_PRIVATE_KEY, codec)?;
            }
        }
    }
    if let Some(arguments) = value.get_mut("argumentList").and_then(Value::as_array_mut) {
        for argument in arguments {
            let Value::Object(object) = argument else {
                continue;
            };
            let is_secret = object
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.eq_ignore_ascii_case("secret"));
            if is_secret {
                seal_field(object, "value", PURPOSE_ARGUMENT_SECRET, codec)?;
            }
        }
    }
    Ok(serde_json::to_string(&value)?)
}

pub fn unseal_server(
    stored_protocol: &str,
    class_version: u32,
    json: &str,
    codec: &dyn SecretCodec,
) -> AppResult<Server> {
    unseal_server_mode(stored_protocol, class_version, json, codec, false).map(|(server, _)| server)
}

/// Reads a server even when one or more secrets were encrypted with another
/// key. Those fields become empty and the returned count lets the UI/recovery
/// path report what was dropped.
pub fn unseal_server_lenient(
    stored_protocol: &str,
    class_version: u32,
    json: &str,
    codec: &dyn SecretCodec,
) -> AppResult<(Server, u32)> {
    unseal_server_mode(stored_protocol, class_version, json, codec, true)
}

fn unseal_server_mode(
    stored_protocol: &str,
    class_version: u32,
    json: &str,
    codec: &dyn SecretCodec,
    lenient: bool,
) -> AppResult<(Server, u32)> {
    let mut value: Value = serde_json::from_str(json)?;
    let mut cleared = 0;
    if let Value::Object(object) = &mut value {
        cleared += usize::from(unseal_field(
            object,
            "password",
            PURPOSE_SERVER_PASSWORD,
            codec,
            lenient,
        )?);
        cleared += usize::from(unseal_field(
            object,
            "privateKey",
            PURPOSE_SERVER_PRIVATE_KEY,
            codec,
            lenient,
        )?);
        cleared += usize::from(unseal_field(
            object,
            "gatewayPassword",
            PURPOSE_GATEWAY_PASSWORD,
            codec,
            lenient,
        )?);
    }
    if let Some(alternates) = value
        .get_mut("alternateCredentials")
        .and_then(Value::as_array_mut)
    {
        for alternate in alternates {
            if let Value::Object(object) = alternate {
                cleared += usize::from(unseal_field(
                    object,
                    "password",
                    PURPOSE_ALTERNATE_PASSWORD,
                    codec,
                    lenient,
                )?);
                cleared += usize::from(unseal_field(
                    object,
                    "privateKey",
                    PURPOSE_ALTERNATE_PRIVATE_KEY,
                    codec,
                    lenient,
                )?);
            }
        }
    }
    if let Some(arguments) = value.get_mut("argumentList").and_then(Value::as_array_mut) {
        for argument in arguments {
            let Value::Object(object) = argument else {
                continue;
            };
            let is_secret = object
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.eq_ignore_ascii_case("secret"));
            if is_secret {
                cleared += usize::from(unseal_field(
                    object,
                    "value",
                    PURPOSE_ARGUMENT_SECRET,
                    codec,
                    lenient,
                )?);
            }
        }
    }
    let server: Server = serde_json::from_value(value)?;
    if !server.protocol().id().eq_ignore_ascii_case(stored_protocol) {
        return Err(AppError::storage(format!(
            "server protocol mismatch: column '{stored_protocol}', JSON '{}'",
            server.protocol().id()
        )));
    }
    if server.base().class_version != class_version {
        return Err(AppError::storage(format!(
            "server class version mismatch: column {class_version}, JSON {}",
            server.base().class_version
        )));
    }
    Ok((server, cleared as u32))
}

pub fn seal_credential(credential: &Credential, codec: &dyn SecretCodec) -> AppResult<String> {
    let mut value = serde_json::to_value(credential)?;
    if let Value::Object(object) = &mut value {
        seal_field(object, "password", PURPOSE_CREDENTIAL_PASSWORD, codec)?;
        seal_field(
            object,
            "privateKeyPath",
            PURPOSE_CREDENTIAL_PRIVATE_KEY,
            codec,
        )?;
    }
    Ok(serde_json::to_string(&value)?)
}

pub fn unseal_credential(json: &str, codec: &dyn SecretCodec) -> AppResult<Credential> {
    unseal_credential_mode(json, codec, false).map(|(credential, _)| credential)
}

/// Credential counterpart to [`unseal_server_lenient`].
pub fn unseal_credential_lenient(
    json: &str,
    codec: &dyn SecretCodec,
) -> AppResult<(Credential, u32)> {
    unseal_credential_mode(json, codec, true)
}

fn unseal_credential_mode(
    json: &str,
    codec: &dyn SecretCodec,
    lenient: bool,
) -> AppResult<(Credential, u32)> {
    let mut value: Value = serde_json::from_str(json)?;
    let mut cleared = 0;
    if let Value::Object(object) = &mut value {
        cleared += usize::from(unseal_field(
            object,
            "password",
            PURPOSE_CREDENTIAL_PASSWORD,
            codec,
            lenient,
        )?);
        cleared += usize::from(unseal_field(
            object,
            "privateKeyPath",
            PURPOSE_CREDENTIAL_PRIVATE_KEY,
            codec,
            lenient,
        )?);
    }
    let credential: Credential = serde_json::from_value(value)?;
    Ok((credential, cleared as u32))
}

fn seal_field(
    object: &mut Map<String, Value>,
    key: &str,
    purpose: &str,
    codec: &dyn SecretCodec,
) -> AppResult<()> {
    let Some(value) = object.get(key) else {
        return Ok(());
    };
    let plaintext = match value {
        Value::String(value) => value,
        Value::Null => return Ok(()),
        _ => {
            return Err(AppError::storage(format!(
                "secret field '{key}' must be a string"
            )));
        }
    };
    if plaintext.is_empty() {
        return Ok(());
    }
    let encoded = codec.encrypt(purpose, plaintext)?;
    let mut sealed = Map::new();
    sealed.insert(ENC_KEY.to_string(), Value::String(encoded));
    object.insert(key.to_string(), Value::Object(sealed));
    Ok(())
}

fn unseal_field(
    object: &mut Map<String, Value>,
    key: &str,
    purpose: &str,
    codec: &dyn SecretCodec,
    lenient: bool,
) -> AppResult<bool> {
    let Some(value) = object.get(key) else {
        return Ok(false);
    };
    let plaintext = match value {
        Value::String(plaintext) => {
            object.insert(key.to_string(), Value::String(plaintext.clone()));
            return Ok(false);
        }
        Value::Null => return Ok(false),
        Value::Object(sealed) => {
            let Some(encoded) = sealed.get(ENC_KEY).and_then(Value::as_str) else {
                if lenient {
                    object.insert(key.to_string(), Value::String(String::new()));
                    return Ok(true);
                }
                return Err(AppError::storage(format!(
                    "secret field '{key}' is malformed"
                )));
            };
            match codec.decrypt(purpose, encoded) {
                Ok(plaintext) => plaintext,
                Err(_) if lenient => {
                    object.insert(key.to_string(), Value::String(String::new()));
                    return Ok(true);
                }
                Err(error) => return Err(error),
            }
        }
        _ if lenient => {
            object.insert(key.to_string(), Value::String(String::new()));
            return Ok(true);
        }
        _ => {
            return Err(AppError::storage(format!(
                "secret field '{key}' has an unsupported shape"
            )));
        }
    };
    object.insert(key.to_string(), Value::String(plaintext));
    Ok(false)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use hr_crypto::{InstallationSalt, KeyStoreKind, Vault};
    use std::path::Path;
    use tempfile::TempDir;

    pub(crate) fn test_vault(dir: &Path) -> Arc<Vault> {
        let salt = InstallationSalt::load_or_create(dir.join("salt")).unwrap();
        Arc::new(Vault::from_parts([7u8; 32], &salt, KeyStoreKind::Memory))
    }

    #[test]
    fn server_secrets_are_encrypted_and_round_trip() {
        let dir = TempDir::new().unwrap();
        let vault = test_vault(dir.path());
        let codec = VaultCodec::new(Arc::clone(&vault));

        for server in hr_domain::sample_servers() {
            let json = seal_server(&server, &codec).unwrap();
            assert!(!json.contains("ssh-secret"));
            assert!(!json.contains("vnc-secret"));
            assert!(!json.contains("gw-secret"));
            assert!(!json.contains("sftp-secret"));
            assert!(!json.contains("ftp-secret"));
            let decoded = unseal_server(
                server.protocol().id(),
                server.base().class_version,
                &json,
                &codec,
            )
            .unwrap();
            assert_eq!(decoded, server);
        }
    }

    #[test]
    fn wrong_vault_fails_to_unseal() {
        let first = TempDir::new().unwrap();
        let second = TempDir::new().unwrap();
        let codec = VaultCodec::new(test_vault(first.path()));
        let other = VaultCodec::new(test_vault(second.path()));
        let server = hr_domain::Server::new_ssh("SSH", "host", "root");
        let server = match server {
            hr_domain::Server::Ssh(mut inner) => {
                inner.credentials.password = "secret".into();
                hr_domain::Server::Ssh(inner)
            }
            _ => unreachable!(),
        };
        let json = seal_server(&server, &codec).unwrap();
        assert!(unseal_server("SSH", server.base().class_version, &json, &other).is_err());

        let (lenient, cleared) =
            unseal_server_lenient("SSH", server.base().class_version, &json, &other).unwrap();
        assert!(cleared > 0);
        assert_eq!(lenient.base().display_name, server.base().display_name);
        assert!(
            lenient
                .credentials()
                .is_some_and(|value| value.password.is_empty())
        );
    }

    #[test]
    fn legacy_plaintext_is_readable_and_resealed() {
        let dir = TempDir::new().unwrap();
        let codec = VaultCodec::new(test_vault(dir.path()));
        let server = hr_domain::Server::new_ssh("SSH", "host", "root");
        let mut value = serde_json::to_value(&server).unwrap();
        value["password"] = Value::String("legacy".into());
        let json = serde_json::to_string(&value).unwrap();
        let decoded = unseal_server("SSH", server.base().class_version, &json, &codec).unwrap();
        assert_eq!(decoded.credentials().unwrap().password.expose(), "legacy");
        let sealed = seal_server(&decoded, &codec).unwrap();
        assert!(!sealed.contains("legacy"));
    }

    #[test]
    fn credential_secrets_are_encrypted() {
        let dir = TempDir::new().unwrap();
        let codec = VaultCodec::new(test_vault(dir.path()));
        let mut credential = Credential::new("prod", "root");
        credential.password = "password".into();
        credential.private_key_path = None;
        let json = seal_credential(&credential, &codec).unwrap();
        assert!(!json.contains("\"password\":\"password\""));
        assert_eq!(unseal_credential(&json, &codec).unwrap(), credential);
    }

    #[test]
    fn argument_secrets_are_encrypted() {
        let dir = TempDir::new().unwrap();
        let codec = VaultCodec::new(test_vault(dir.path()));
        let mut server = hr_domain::Server::new_local_app("App", "C:\\app.exe");
        if let hr_domain::Server::LocalApp(inner) = &mut server {
            inner.argument_list = vec![hr_domain::AppArgument {
                argument_type: hr_domain::AppArgumentType::Secret,
                name: "token".into(),
                key: "--token".into(),
                value: "do-not-leak".into(),
                ..hr_domain::AppArgument::default()
            }];
        }
        let json = seal_server(&server, &codec).unwrap();
        assert!(!json.contains("do-not-leak"));
        assert_eq!(
            unseal_server("LocalApp", server.base().class_version, &json, &codec).unwrap(),
            server
        );
    }

    #[test]
    fn sentinel_detects_a_different_key() {
        let first = TempDir::new().unwrap();
        let second = TempDir::new().unwrap();
        let codec = VaultCodec::new(test_vault(first.path()));
        let other = VaultCodec::new(test_vault(second.path()));
        let sentinel = codec.create_sentinel().unwrap();
        assert!(codec.verify_sentinel(&sentinel).is_ok());
        assert!(other.verify_sentinel(&sentinel).is_err());
    }
}
