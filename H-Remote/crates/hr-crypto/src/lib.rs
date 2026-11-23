//! Encryption at rest.
//!
//! The master key is stored in the operating-system keyring. When that is
//! unavailable (headless Linux, CI, locked keychain), an Argon2id-protected
//! key file can be unlocked with `HREMOTE_VAULT_PASSPHRASE`. Plaintext secrets
//! are never written to disk.
//!
//! Ciphertext format: `v1:<nonce base64url>:<ciphertext base64url>`. The
//! associated data binds the ciphertext to the installation salt and a purpose
//! string, so copying a value to a different field or installation fails
//! authentication instead of decrypting.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use getrandom::fill as fill_random;
use hr_core::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};

pub const CIPHERTEXT_VERSION: &str = "v1";
pub const ENCRYPTION_SENTINEL: &str = "HREMOTE-ENCRYPTION-SENTINEL-v1";
pub const SALT_FILE_NAME: &str = ".encryption_salt";
pub const KEY_FILE_NAME: &str = ".vault_key.json";
pub const PASS_PHRASE_ENV: &str = "HREMOTE_VAULT_PASSPHRASE";
const KEYRING_SERVICE: &str = "H-Remote";
const KEYRING_USER: &str = "vault-master-key";
const SALT_LEN: usize = 32;
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyStoreKind {
    OsKeyring,
    PassphraseFile,
    Memory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub ready: bool,
    pub key_store: Option<KeyStoreKind>,
    pub salt_path: PathBuf,
    pub message: Option<String>,
}

/// A 32-byte random installation salt persisted beside the profile.
pub struct InstallationSalt {
    bytes: Zeroizing<[u8; SALT_LEN]>,
    path: PathBuf,
}

impl InstallationSalt {
    pub fn load_or_create(path: PathBuf) -> AppResult<Self> {
        if let Ok(from_env) = std::env::var("HREMOTE_ENCRYPTION_SALT") {
            let decoded = URL_SAFE_NO_PAD.decode(from_env.trim()).map_err(|error| {
                AppError::crypto(format!("invalid HREMOTE_ENCRYPTION_SALT: {error}"))
            })?;
            if decoded.len() != SALT_LEN {
                return Err(AppError::crypto("HREMOTE_ENCRYPTION_SALT must be 32 bytes"));
            }
            let mut bytes = Zeroizing::new([0u8; SALT_LEN]);
            bytes.copy_from_slice(&decoded);
            return Ok(Self { bytes, path });
        }

        if path.is_file() {
            let encoded = std::fs::read_to_string(&path)?;
            let decoded = URL_SAFE_NO_PAD
                .decode(encoded.trim())
                .map_err(|error| AppError::crypto(format!("invalid salt file: {error}")))?;
            if decoded.len() != SALT_LEN {
                return Err(AppError::crypto("salt file must contain 32 bytes"));
            }
            let mut bytes = Zeroizing::new([0u8; SALT_LEN]);
            bytes.copy_from_slice(&decoded);
            return Ok(Self { bytes, path });
        }

        let mut bytes = Zeroizing::new([0u8; SALT_LEN]);
        fill_random(&mut *bytes).map_err(|error| AppError::crypto(format!("random: {error}")))?;
        let salt = Self { bytes, path };
        salt.persist()?;
        Ok(salt)
    }

    fn persist(&self) -> AppResult<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let encoded = URL_SAFE_NO_PAD.encode(&self.bytes[..]);
        std::fs::write(&self.path, encoded.as_bytes())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    pub fn as_bytes(&self) -> &[u8; SALT_LEN] {
        &self.bytes
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// AES-256-GCM vault bound to an installation salt.
pub struct Vault {
    key: Zeroizing<[u8; KEY_LEN]>,
    salt: Zeroizing<[u8; SALT_LEN]>,
    kind: KeyStoreKind,
}

impl Vault {
    pub fn from_parts(key: [u8; KEY_LEN], salt: &InstallationSalt, kind: KeyStoreKind) -> Self {
        let mut salt_bytes = Zeroizing::new([0u8; SALT_LEN]);
        salt_bytes.copy_from_slice(salt.as_bytes());
        Self {
            key: Zeroizing::new(key),
            salt: salt_bytes,
            kind,
        }
    }

    pub fn key_store_kind(&self) -> KeyStoreKind {
        self.kind
    }

    /// Encrypts bytes for a specific purpose (for example `credential-password`).
    pub fn encrypt(&self, purpose: &str, plaintext: &[u8]) -> AppResult<String> {
        let cipher = Aes256Gcm::new_from_slice(&self.key[..])
            .map_err(|error| AppError::crypto(format!("invalid key: {error}")))?;
        let mut nonce_bytes = [0u8; NONCE_LEN];
        fill_random(&mut nonce_bytes)
            .map_err(|error| AppError::crypto(format!("random: {error}")))?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let aad = self.aad(purpose);
        let ciphertext = cipher
            .encrypt(
                nonce,
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| AppError::crypto("encryption failed"))?;
        Ok(format!(
            "{CIPHERTEXT_VERSION}:{}:{}",
            URL_SAFE_NO_PAD.encode(nonce_bytes),
            URL_SAFE_NO_PAD.encode(ciphertext)
        ))
    }

    pub fn decrypt(&self, purpose: &str, encoded: &str) -> AppResult<Zeroizing<Vec<u8>>> {
        let mut parts = encoded.split(':');
        let version = parts
            .next()
            .ok_or_else(|| AppError::crypto("malformed ciphertext"))?;
        if version != CIPHERTEXT_VERSION {
            return Err(AppError::crypto(format!(
                "unsupported ciphertext version '{version}'"
            )));
        }
        let nonce_bytes = URL_SAFE_NO_PAD
            .decode(
                parts
                    .next()
                    .ok_or_else(|| AppError::crypto("ciphertext has no nonce"))?,
            )
            .map_err(|error| AppError::crypto(format!("invalid nonce: {error}")))?;
        if nonce_bytes.len() != NONCE_LEN {
            return Err(AppError::crypto("nonce must be 12 bytes"));
        }
        let ciphertext = URL_SAFE_NO_PAD
            .decode(
                parts
                    .next()
                    .ok_or_else(|| AppError::crypto("ciphertext has no payload"))?,
            )
            .map_err(|error| AppError::crypto(format!("invalid payload: {error}")))?;
        if parts.next().is_some() {
            return Err(AppError::crypto("ciphertext has trailing data"));
        }

        let cipher = Aes256Gcm::new_from_slice(&self.key[..])
            .map_err(|error| AppError::crypto(format!("invalid key: {error}")))?;
        let aad = self.aad(purpose);
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&nonce_bytes),
                Payload {
                    msg: &ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| {
                AppError::crypto("decryption failed: wrong key, purpose or corrupt data")
            })?;
        Ok(Zeroizing::new(plaintext))
    }

    pub fn encrypt_string(&self, purpose: &str, plaintext: &str) -> AppResult<String> {
        self.encrypt(purpose, plaintext.as_bytes())
    }

    pub fn decrypt_string(&self, purpose: &str, encoded: &str) -> AppResult<Zeroizing<String>> {
        let bytes = self.decrypt(purpose, encoded)?;
        let text = String::from_utf8(bytes.to_vec())
            .map_err(|_| AppError::crypto("decrypted value is not UTF-8"))?;
        Ok(Zeroizing::new(text))
    }

    /// Creates the value stored in `Configs.EncryptionTest` (P1-38).
    pub fn create_sentinel(&self) -> AppResult<String> {
        self.encrypt_string("encryption-test", ENCRYPTION_SENTINEL)
    }

    /// Verifies a database sentinel. A wrong key returns an explicit error.
    pub fn verify_sentinel(&self, encoded: &str) -> AppResult<()> {
        let value = self.decrypt_string("encryption-test", encoded)?;
        if value
            .as_bytes()
            .ct_eq(ENCRYPTION_SENTINEL.as_bytes())
            .into()
        {
            Ok(())
        } else {
            Err(AppError::crypto("encryption sentinel mismatch"))
        }
    }

    fn aad(&self, purpose: &str) -> Vec<u8> {
        let mut aad = Vec::with_capacity(64 + purpose.len());
        aad.extend_from_slice(b"hremote:");
        aad.extend_from_slice(CIPHERTEXT_VERSION.as_bytes());
        aad.push(b':');
        aad.extend_from_slice(URL_SAFE_NO_PAD.encode(&self.salt[..]).as_bytes());
        aad.push(b':');
        aad.extend_from_slice(purpose.as_bytes());
        aad
    }
}

/// OS keyring backend. Uses Credential Manager, Keychain or Secret Service.
pub struct OsKeyringStore {
    service: String,
    user: String,
}

impl Default for OsKeyringStore {
    fn default() -> Self {
        Self {
            service: KEYRING_SERVICE.to_string(),
            user: KEYRING_USER.to_string(),
        }
    }
}

impl OsKeyringStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn entry(&self) -> AppResult<keyring::Entry> {
        keyring::Entry::new(&self.service, &self.user)
            .map_err(|error| AppError::crypto(format!("OS keyring unavailable: {error}")))
    }

    pub fn load(&self) -> AppResult<Option<Zeroizing<[u8; KEY_LEN]>>> {
        let entry = self.entry()?;
        match entry.get_password() {
            Ok(encoded) => {
                let decoded = URL_SAFE_NO_PAD.decode(encoded.trim()).map_err(|error| {
                    AppError::crypto(format!("keyring entry is corrupt: {error}"))
                })?;
                if decoded.len() != KEY_LEN {
                    return Err(AppError::crypto("keyring entry has the wrong length"));
                }
                let mut key = Zeroizing::new([0u8; KEY_LEN]);
                key.copy_from_slice(&decoded);
                Ok(Some(key))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(AppError::crypto(format!("cannot read OS keyring: {error}"))),
        }
    }

    pub fn store(&self, key: &[u8; KEY_LEN]) -> AppResult<()> {
        let encoded = URL_SAFE_NO_PAD.encode(key);
        self.entry()?
            .set_password(&encoded)
            .map_err(|error| AppError::crypto(format!("cannot write OS keyring: {error}")))
    }

    pub fn delete(&self) -> AppResult<()> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(AppError::crypto(format!(
                "cannot delete keyring entry: {error}"
            ))),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct KeyFile {
    version: u32,
    salt: String,
    nonce: String,
    ciphertext: String,
}

/// Master key encrypted with an Argon2id-derived passphrase key.
pub struct PassphraseFileStore {
    path: PathBuf,
    passphrase: Zeroizing<String>,
}

impl PassphraseFileStore {
    pub fn new(path: PathBuf, passphrase: Zeroizing<String>) -> AppResult<Self> {
        if passphrase.trim().is_empty() {
            return Err(AppError::crypto("vault passphrase must not be empty"));
        }
        Ok(Self { path, passphrase })
    }

    /// Loads the master key, creating and persisting one on first use.
    pub fn load_or_create(&self) -> AppResult<Zeroizing<[u8; KEY_LEN]>> {
        if self.path.is_file() {
            self.load_existing()
        } else {
            let mut key = Zeroizing::new([0u8; KEY_LEN]);
            fill_random(&mut *key).map_err(|error| AppError::crypto(format!("random: {error}")))?;
            self.persist(&key)?;
            Ok(key)
        }
    }

    fn load_existing(&self) -> AppResult<Zeroizing<[u8; KEY_LEN]>> {
        let bytes = std::fs::read(&self.path)?;
        let file: KeyFile = serde_json::from_slice(&bytes)
            .map_err(|error| AppError::crypto(format!("vault key file is corrupt: {error}")))?;
        if file.version != 1 {
            return Err(AppError::crypto("unsupported vault key file version"));
        }
        let salt = URL_SAFE_NO_PAD
            .decode(&file.salt)
            .map_err(|error| AppError::crypto(format!("invalid key-file salt: {error}")))?;
        let nonce = URL_SAFE_NO_PAD
            .decode(&file.nonce)
            .map_err(|error| AppError::crypto(format!("invalid key-file nonce: {error}")))?;
        let ciphertext = URL_SAFE_NO_PAD
            .decode(&file.ciphertext)
            .map_err(|error| AppError::crypto(format!("invalid key-file payload: {error}")))?;
        if salt.len() < 16 || nonce.len() != NONCE_LEN {
            return Err(AppError::crypto("vault key file has invalid parameters"));
        }

        let derived = self.derive(&salt)?;
        let cipher = Aes256Gcm::new_from_slice(&derived[..])
            .map_err(|error| AppError::crypto(format!("invalid derived key: {error}")))?;
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: b"hremote:vault-key-file:v1",
                },
            )
            .map_err(|_| AppError::crypto("wrong passphrase or corrupt vault key file"))?;
        if plaintext.len() != KEY_LEN {
            return Err(AppError::crypto("vault key file has the wrong key length"));
        }
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        key.copy_from_slice(&plaintext);
        Ok(key)
    }

    fn persist(&self, key: &[u8; KEY_LEN]) -> AppResult<()> {
        let mut salt = [0u8; 32];
        fill_random(&mut salt).map_err(|error| AppError::crypto(format!("random: {error}")))?;
        let mut nonce = [0u8; NONCE_LEN];
        fill_random(&mut nonce).map_err(|error| AppError::crypto(format!("random: {error}")))?;

        let derived = self.derive(&salt)?;
        let cipher = Aes256Gcm::new_from_slice(&derived[..])
            .map_err(|error| AppError::crypto(format!("invalid derived key: {error}")))?;
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &key[..],
                    aad: b"hremote:vault-key-file:v1",
                },
            )
            .map_err(|_| AppError::crypto("cannot encrypt vault key"))?;

        let file = KeyFile {
            version: 1,
            salt: URL_SAFE_NO_PAD.encode(salt),
            nonce: URL_SAFE_NO_PAD.encode(nonce),
            ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
        };
        let bytes = serde_json::to_vec_pretty(&file)?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temp = self.path.with_extension("tmp");
        std::fs::write(&temp, &bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&temp, &self.path)?;
        Ok(())
    }

    fn derive(&self, salt: &[u8]) -> AppResult<Zeroizing<[u8; KEY_LEN]>> {
        // Argon2id with interactive parameters; sufficient for a local key file.
        let params = Params::new(19_456, 2, 1, Some(KEY_LEN))
            .map_err(|error| AppError::crypto(format!("invalid Argon2 parameters: {error}")))?;
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut derived = Zeroizing::new([0u8; KEY_LEN]);
        argon
            .hash_password_into(self.passphrase.as_bytes(), salt, &mut *derived)
            .map_err(|error| AppError::crypto(format!("key derivation failed: {error}")))?;
        Ok(derived)
    }
}

/// In-memory key store used by tests and the UI test harness.
#[derive(Default)]
pub struct MemoryKeyStore {
    slot: Mutex<Option<[u8; KEY_LEN]>>,
}

impl MemoryKeyStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn store(&self, key: &[u8; KEY_LEN]) {
        *self.slot.lock().expect("memory key lock poisoned") = Some(*key);
    }

    pub fn load(&self) -> Option<Zeroizing<[u8; KEY_LEN]>> {
        self.slot
            .lock()
            .expect("memory key lock poisoned")
            .map(Zeroizing::new)
    }
}

/// Result of attempting to initialise the vault at startup.
pub enum VaultInit {
    Ready(Vault),
    Unavailable(VaultStatus),
}

/// Initialises the vault from the OS keyring, then the passphrase file
/// fallback. Never blocks startup: unavailable secrets are reported to the UI.
pub fn initialize_vault(salt_path: PathBuf, key_file_path: PathBuf) -> VaultInit {
    let salt = match InstallationSalt::load_or_create(salt_path.clone()) {
        Ok(salt) => salt,
        Err(error) => {
            return VaultInit::Unavailable(VaultStatus {
                ready: false,
                key_store: None,
                salt_path,
                message: Some(error.to_string()),
            });
        }
    };

    let keyring = OsKeyringStore::new();
    match keyring.load() {
        Ok(Some(key)) => {
            return VaultInit::Ready(Vault::from_parts(*key, &salt, KeyStoreKind::OsKeyring));
        }
        Ok(None) => {
            let mut key = [0u8; KEY_LEN];
            if fill_random(&mut key).is_ok() {
                if keyring.store(&key).is_ok() {
                    return VaultInit::Ready(Vault::from_parts(
                        key,
                        &salt,
                        KeyStoreKind::OsKeyring,
                    ));
                }
                key.zeroize();
            }
        }
        Err(error) => {
            tracing::warn!(%error, "OS keyring unavailable; trying passphrase key file");
        }
    }

    if let Ok(passphrase) = std::env::var(PASS_PHRASE_ENV) {
        let store = match PassphraseFileStore::new(key_file_path, Zeroizing::new(passphrase)) {
            Ok(store) => store,
            Err(error) => {
                return VaultInit::Unavailable(VaultStatus {
                    ready: false,
                    key_store: None,
                    salt_path,
                    message: Some(error.to_string()),
                });
            }
        };
        match store.load_or_create() {
            Ok(key) => {
                return VaultInit::Ready(Vault::from_parts(
                    *key,
                    &salt,
                    KeyStoreKind::PassphraseFile,
                ));
            }
            Err(error) => {
                return VaultInit::Unavailable(VaultStatus {
                    ready: false,
                    key_store: None,
                    salt_path,
                    message: Some(error.to_string()),
                });
            }
        }
    }

    VaultInit::Unavailable(VaultStatus {
        ready: false,
        key_store: None,
        salt_path,
        message: Some(
            "No OS keyring is available. Set HREMOTE_VAULT_PASSPHRASE to enable encrypted storage."
                .to_string(),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_vault(dir: &TempDir) -> Vault {
        let salt = InstallationSalt::load_or_create(dir.path().join("salt")).unwrap();
        Vault::from_parts([7u8; KEY_LEN], &salt, KeyStoreKind::Memory)
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        let dir = TempDir::new().unwrap();
        let vault = test_vault(&dir);
        let encoded = vault.encrypt_string("password", "hunter2").unwrap();
        assert!(!encoded.contains("hunter2"));
        let decoded = vault.decrypt_string("password", &encoded).unwrap();
        assert_eq!(&*decoded, "hunter2");
    }

    #[test]
    fn ciphertext_is_randomised() {
        let dir = TempDir::new().unwrap();
        let vault = test_vault(&dir);
        let a = vault.encrypt_string("password", "same").unwrap();
        let b = vault.encrypt_string("password", "same").unwrap();
        assert_ne!(a, b);
        assert_eq!(&*vault.decrypt_string("password", &a).unwrap(), "same");
        assert_eq!(&*vault.decrypt_string("password", &b).unwrap(), "same");
    }

    #[test]
    fn purpose_and_salt_are_bound_into_the_ciphertext() {
        let dir = TempDir::new().unwrap();
        let vault = test_vault(&dir);
        let encoded = vault.encrypt_string("password", "secret").unwrap();
        assert!(vault.decrypt_string("private-key", &encoded).is_err());

        let other_salt = InstallationSalt::load_or_create(dir.path().join("other-salt")).unwrap();
        let other = Vault::from_parts([7u8; KEY_LEN], &other_salt, KeyStoreKind::Memory);
        assert!(other.decrypt_string("password", &encoded).is_err());
    }

    #[test]
    fn tampering_is_detected() {
        let dir = TempDir::new().unwrap();
        let vault = test_vault(&dir);
        let encoded = vault.encrypt_string("password", "secret").unwrap();
        let mut chars: Vec<char> = encoded.chars().collect();
        let last = chars.len() - 2;
        chars[last] = if chars[last] == 'A' { 'B' } else { 'A' };
        let tampered: String = chars.into_iter().collect();
        assert!(vault.decrypt_string("password", &tampered).is_err());
    }

    #[test]
    fn sentinel_detects_the_wrong_key() {
        let dir = TempDir::new().unwrap();
        let salt = InstallationSalt::load_or_create(dir.path().join("salt")).unwrap();
        let vault = Vault::from_parts([1u8; KEY_LEN], &salt, KeyStoreKind::Memory);
        let sentinel = vault.create_sentinel().unwrap();
        assert!(vault.verify_sentinel(&sentinel).is_ok());

        let wrong = Vault::from_parts([2u8; KEY_LEN], &salt, KeyStoreKind::Memory);
        assert!(wrong.verify_sentinel(&sentinel).is_err());
    }

    #[test]
    fn salt_is_stable_and_persisted() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".locality").join(SALT_FILE_NAME);
        let first = InstallationSalt::load_or_create(path.clone()).unwrap();
        let second = InstallationSalt::load_or_create(path.clone()).unwrap();
        assert_eq!(first.as_bytes(), second.as_bytes());
        assert!(path.is_file());
    }

    #[test]
    fn passphrase_file_store_persists_and_rejects_wrong_passphrase() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(KEY_FILE_NAME);
        let store =
            PassphraseFileStore::new(path.clone(), Zeroizing::new("correct horse".into())).unwrap();
        let key = store.load_or_create().unwrap();
        assert!(path.is_file());

        let encoded = std::fs::read_to_string(&path).unwrap();
        assert!(!encoded.contains(&URL_SAFE_NO_PAD.encode(&key[..])));

        let reopened =
            PassphraseFileStore::new(path.clone(), Zeroizing::new("correct horse".into())).unwrap();
        assert_eq!(&*reopened.load_or_create().unwrap(), &*key);

        let wrong = PassphraseFileStore::new(path, Zeroizing::new("wrong horse".into())).unwrap();
        assert!(wrong.load_or_create().is_err());
    }

    #[test]
    fn empty_passphrase_is_rejected() {
        let dir = TempDir::new().unwrap();
        assert!(
            PassphraseFileStore::new(dir.path().join("key"), Zeroizing::new("  ".into())).is_err()
        );
    }

    #[test]
    fn memory_key_store_round_trips() {
        let store = MemoryKeyStore::new();
        assert!(store.load().is_none());
        store.store(&[9u8; KEY_LEN]);
        assert_eq!(&*store.load().unwrap(), &[9u8; KEY_LEN]);
    }

    #[test]
    fn malformed_ciphertext_is_rejected_without_panicking() {
        let dir = TempDir::new().unwrap();
        let vault = test_vault(&dir);
        for value in [
            "",
            "v2:a:b",
            "v1:%%%:%%",
            "v1:AA:BB",
            "v1:AAAAAAAAAAAAAAAA:AA:extra",
        ] {
            assert!(vault.decrypt_string("password", value).is_err(), "{value}");
        }
    }
}
