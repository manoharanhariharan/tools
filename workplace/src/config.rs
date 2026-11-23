use std::env;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsConfig {
    pub cert: PathBuf,
    pub key: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub data_dir: PathBuf,
    pub tls: Option<TlsConfig>,
    pub session_ttl_secs: i64,
    pub pdf_font: Option<PathBuf>,
    /// Development override: serve UI assets from disk instead of the binary.
    pub web_dir: Option<PathBuf>,
}

impl Config {
    /// Resolve configuration from CLI flags first, then environment, then
    /// defaults. Fails loudly on malformed values instead of guessing.
    pub fn resolve(bind: Option<String>, data_dir: Option<PathBuf>) -> anyhow::Result<Self> {
        let bind_raw = bind
            .or_else(|| env::var("DAYBOOK_BIND").ok())
            .unwrap_or_else(|| "0.0.0.0:8787".to_string());
        let bind: SocketAddr = bind_raw
            .parse()
            .with_context(|| format!("invalid listen address '{bind_raw}' (expected IP:PORT)"))?;

        let data_dir = data_dir
            .or_else(|| env::var_os("DAYBOOK_DATA_DIR").map(PathBuf::from))
            .unwrap_or_else(default_data_dir);

        let cert = env::var_os("DAYBOOK_TLS_CERT").map(PathBuf::from);
        let key = env::var_os("DAYBOOK_TLS_KEY").map(PathBuf::from);
        let tls = match (cert, key) {
            (Some(cert), Some(key)) => Some(TlsConfig { cert, key }),
            (None, None) => None,
            _ => bail!("DAYBOOK_TLS_CERT and DAYBOOK_TLS_KEY must be set together"),
        };

        let session_ttl_secs = match env::var("DAYBOOK_SESSION_TTL_DAYS") {
            Ok(raw) => {
                let days: i64 = raw
                    .parse()
                    .with_context(|| format!("invalid DAYBOOK_SESSION_TTL_DAYS '{raw}'"))?;
                if !(1..=365).contains(&days) {
                    bail!("DAYBOOK_SESSION_TTL_DAYS must be between 1 and 365");
                }
                days * 86_400
            }
            Err(_) => 30 * 86_400,
        };

        Ok(Self {
            bind,
            data_dir,
            tls,
            session_ttl_secs,
            pdf_font: env::var_os("DAYBOOK_PDF_FONT").map(PathBuf::from),
            web_dir: env::var_os("DAYBOOK_WEB_DIR").map(PathBuf::from),
        })
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("daybook.db")
    }

    pub fn is_tls(&self) -> bool {
        self.tls.is_some()
    }

    pub fn secure_cookies(&self) -> bool {
        self.tls.is_some()
    }

    pub fn ensure_data_dir(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.data_dir)
            .with_context(|| format!("cannot create data directory {}", self.data_dir.display()))
    }

    /// Authentication password supplied at bootstrap. Only used when no
    /// password hash exists yet.
    pub fn bootstrap_password() -> Option<String> {
        env::var("DAYBOOK_ADMIN_PASSWORD")
            .ok()
            .filter(|p| !p.is_empty())
    }
}

pub fn default_data_dir() -> PathBuf {
    if cfg!(windows) {
        if let Some(local) = env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local).join("Daybook");
        }
    } else if let Some(data_home) = env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(data_home).join("daybook");
    } else if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".local/share/daybook");
    }
    Path::new("daybook-data").to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_malformed_bind() {
        let result = Config::resolve(Some("not-an-address".into()), None);
        assert!(result.is_err());
    }

    #[test]
    fn accepts_explicit_bind() {
        let config = Config::resolve(Some("127.0.0.1:9999".into()), Some(PathBuf::from("x")))
            .expect("valid");
        assert_eq!(config.bind.port(), 9999);
        assert_eq!(config.db_path(), PathBuf::from("x").join("daybook.db"));
    }
}
