//! Credentials, alternate credentials and inheritance.

use std::fmt;
use std::ops::{Deref, DerefMut};

use hr_core::new_id;
use serde::{Deserialize, Serialize};

use crate::protocol::AddressPortBase;

/// A string whose `Debug` representation never reveals the value.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            formatter.write_str("SecretString(<empty>)")
        } else {
            formatter.write_str("SecretString(***)")
        }
    }
}

impl Deref for SecretString {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SecretString {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl AsRef<str> for SecretString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct AlternateCredential {
    pub name: String,
    pub address: Option<String>,
    pub port: Option<u16>,
    pub username: String,
    pub password: SecretString,
    pub private_key: Option<String>,
}

impl AlternateCredential {
    pub fn normalize(&mut self) {
        self.name = self.name.trim().to_string();
        self.address = self
            .address
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        self.username = self.username.trim().to_string();
        self.private_key = self
            .private_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
    }

    /// Fills a missing address/port from the server so launching an alternate
    /// credential never produces an unreachable target (P7-15).
    pub fn normalize_against(&mut self, main: &AddressPortBase) {
        self.normalize();
        if self.address.is_none() {
            self.address = Some(main.address.clone());
        }
        if self.port.is_none() {
            self.port = Some(main.port);
        }
    }

    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();
        if self.name.trim().is_empty() {
            issues.push("alternate credential name is required".to_string());
        }
        if !self.password.is_empty() && self.private_key.is_some() {
            issues.push(format!(
                "alternate credential '{}' cannot use both password and private key",
                self.name
            ));
        }
        issues
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Credential {
    pub id: String,
    pub name: String,
    pub address: Option<String>,
    pub port: Option<u16>,
    pub username: String,
    pub password: SecretString,
    pub private_key_path: Option<String>,
    /// Source/database that owns the credential, if any.
    pub database_id: Option<String>,
    /// MD5 of the decrypted tuple, used for duplicate detection only.
    pub hash: String,
    pub is_editable: bool,
}

impl Credential {
    pub fn new(name: impl Into<String>, username: impl Into<String>) -> Self {
        let mut credential = Self {
            id: new_id(),
            name: name.into(),
            username: username.into(),
            is_editable: true,
            ..Self::default()
        };
        credential.rehash();
        credential
    }

    pub fn normalize(&mut self) {
        self.name = self.name.trim().to_string();
        self.address = self
            .address
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        self.username = self.username.trim().to_string();
        self.private_key_path = self
            .private_key_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
    }

    pub fn rehash(&mut self) -> String {
        let mut hash = md5::Context::new();
        hash.consume(self.username.as_bytes());
        hash.consume(b"\0");
        hash.consume(self.password.expose().as_bytes());
        hash.consume(b"\0");
        hash.consume(self.address.as_deref().unwrap_or("").as_bytes());
        hash.consume(b"\0");
        hash.consume(self.port.unwrap_or(0).to_string().as_bytes());
        hash.consume(b"\0");
        hash.consume(self.private_key_path.as_deref().unwrap_or("").as_bytes());
        let digest = hash.compute();
        self.hash = digest.0.iter().map(|byte| format!("{byte:02x}")).collect();
        self.hash.clone()
    }

    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();
        let name = self.name.trim();
        if name.is_empty() {
            issues.push("credential name is required".to_string());
        } else if name.chars().count() > 100 {
            issues.push("credential name must be at most 100 characters".to_string());
        }
        if self.username.trim().is_empty() {
            issues.push(format!("credential '{name}' needs a username"));
        }
        if !self.password.is_empty() && self.private_key_path.is_some() {
            issues.push(format!(
                "credential '{name}' cannot use both password and private key"
            ));
        }
        issues
    }

    pub fn duplicate(&self) -> Self {
        let mut duplicate = self.clone();
        duplicate.id = new_id();
        duplicate.name = format!("{} (copy)", self.name);
        duplicate.rehash();
        duplicate
    }

    /// Fills missing address/port from a primary credential for alternate use.
    pub fn inherit_address_port_from(&mut self, primary: &Credential) {
        if self.address.is_none() {
            self.address = primary.address.clone();
        }
        if self.port.is_none() {
            self.port = primary.port;
        }
        self.rehash();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CredentialSource {
    Inline,
    Inherited,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveCredentials {
    pub username: String,
    pub password: SecretString,
    pub private_key: Option<String>,
    pub source: CredentialSource,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_string_never_debugs_the_value() {
        let secret = SecretString::new("hunter2");
        let debug = format!("{secret:?}");
        assert!(!debug.contains("hunter2"));
        assert!(debug.contains("***"));
        assert_eq!(
            format!("{:?}", SecretString::default()),
            "SecretString(<empty>)"
        );
    }

    #[test]
    fn credential_hash_changes_when_any_secret_tuple_field_changes() {
        let mut credential = Credential::new("prod", "root");
        credential.password = "a".into();
        let first = credential.rehash();
        credential.password = "b".into();
        let second = credential.rehash();
        assert_ne!(first, second);
        credential.password = "a".into();
        assert_eq!(credential.rehash(), first);
        credential.username = "admin".into();
        assert_ne!(credential.rehash(), first);
    }

    #[test]
    fn credentials_validate_names_and_exclusivity() {
        let mut credential = Credential::new("prod", "root");
        assert!(credential.validate().is_empty());
        credential.name = "x".repeat(101);
        assert!(!credential.validate().is_empty());

        let mut both = Credential::new("prod", "root");
        both.password = "secret".into();
        both.private_key_path = Some("C:\\key".into());
        assert!(both.validate().iter().any(|issue| issue.contains("both")));
    }

    #[test]
    fn duplicate_gets_new_id_and_name() {
        let credential = Credential::new("prod", "root");
        let duplicate = credential.duplicate();
        assert_ne!(duplicate.id, credential.id);
        assert_eq!(duplicate.name, "prod (copy)");
    }

    #[test]
    fn alternate_credentials_inherit_address_and_port() {
        let mut main = AddressPortBase::new(22);
        main.address = "host".into();
        let mut alternate = AlternateCredential {
            name: "backup".into(),
            username: "root".into(),
            ..AlternateCredential::default()
        };
        alternate.normalize_against(&main);
        assert_eq!(alternate.address.as_deref(), Some("host"));
        assert_eq!(alternate.port, Some(22));
    }

    #[test]
    fn inherited_credentials_fill_only_missing_fields() {
        let mut vault = Credential::new("vault", "vault-user");
        vault.password = "vault-pass".into();
        vault.private_key_path = None;

        let mut inline = crate::protocol::UserPasswordBase::with_username("inline-user");
        inline.inherited_credential_name = Some("vault".into());
        let effective = inline.resolve_effective(Some(&vault));
        assert_eq!(effective.username, "inline-user");
        assert_eq!(effective.password.as_ref(), "vault-pass");
        assert_eq!(effective.source, CredentialSource::Inherited);

        inline.password = "inline-pass".into();
        let effective = inline.resolve_effective(Some(&vault));
        assert_eq!(effective.password.as_ref(), "inline-pass");
    }
}
