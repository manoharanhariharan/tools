//! Self-signed local CA + server certificate generation.
//!
//! Browsers only allow service workers (and therefore installable PWAs with
//! offline sync) on secure origins. A LAN IP over plain HTTP is not secure, so
//! in-house deployments install this generated CA once per phone and serve the
//! app over HTTPS.

use std::net::{IpAddr, UdpSocket};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose, SanType,
};
use time::{Duration, OffsetDateTime};

pub const CA_FILENAME: &str = "daybook-ca.crt";
pub const CERT_FILENAME: &str = "daybook-cert.pem";
pub const KEY_FILENAME: &str = "daybook-key.pem";

pub struct GeneratedCerts {
    pub ca_cert: PathBuf,
    pub cert: PathBuf,
    pub key: PathBuf,
    pub ca_pem: String,
    /// SANs actually embedded in the leaf certificate.
    pub hosts: Vec<String>,
}

/// Generate a 10-year local CA and a 397-day server certificate (Apple rejects
/// longer-lived TLS leaf certificates).
pub fn generate(out_dir: &Path, hosts: &[String]) -> anyhow::Result<GeneratedCerts> {
    let mut sans: Vec<SanType> = Vec::new();
    let mut host_labels: Vec<String> = Vec::new();
    for raw in hosts {
        let host = raw.trim();
        if host.is_empty() {
            continue;
        }
        if host_labels.iter().any(|existing| existing.eq_ignore_ascii_case(host)) {
            continue;
        }
        let san = san_for(host)?;
        host_labels.push(host.to_string());
        sans.push(san);
    }
    if sans.is_empty() {
        bail!("at least one host name or IP address is required");
    }

    let now = OffsetDateTime::now_utc();

    let mut ca_params = CertificateParams::new(Vec::<String>::new())?;
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    ca_params
        .distinguished_name
        .push(DnType::CommonName, "Daybook Local CA");
    ca_params
        .distinguished_name
        .push(DnType::OrganizationName, "Daybook self-hosted");
    ca_params.not_before = now - Duration::days(1);
    ca_params.not_after = now + Duration::days(3_650);

    let ca_key = KeyPair::generate().context("cannot generate CA key")?;
    let ca_cert = ca_params
        .self_signed(&ca_key)
        .context("cannot sign CA certificate")?;
    let ca_pem = ca_cert.pem();

    let issuer = Issuer::from_params(&ca_params, &ca_key);

    let mut leaf_params = CertificateParams::new(Vec::<String>::new())?;
    leaf_params.subject_alt_names = sans;
    leaf_params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    leaf_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    leaf_params.use_authority_key_identifier_extension = true;
    leaf_params
        .distinguished_name
        .push(DnType::CommonName, host_labels[0].clone());
    leaf_params
        .distinguished_name
        .push(DnType::OrganizationName, "Daybook self-hosted");
    leaf_params.not_before = now - Duration::days(1);
    leaf_params.not_after = now + Duration::days(397);

    let leaf_key = KeyPair::generate().context("cannot generate server key")?;
    let leaf_cert = leaf_params
        .signed_by(&leaf_key, &issuer)
        .context("cannot sign server certificate")?;

    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("cannot create {}", out_dir.display()))?;

    let ca_cert_path = out_dir.join(CA_FILENAME);
    let cert_path = out_dir.join(CERT_FILENAME);
    let key_path = out_dir.join(KEY_FILENAME);

    std::fs::write(&ca_cert_path, &ca_pem)
        .with_context(|| format!("cannot write {}", ca_cert_path.display()))?;
    // Leaf first, then the issuing CA: rustls accepts this as a chain.
    std::fs::write(&cert_path, format!("{}{}", leaf_cert.pem(), ca_pem))
        .with_context(|| format!("cannot write {}", cert_path.display()))?;
    std::fs::write(&key_path, leaf_key.serialize_pem())
        .with_context(|| format!("cannot write {}", key_path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600));
    }

    Ok(GeneratedCerts {
        ca_cert: ca_cert_path,
        cert: cert_path,
        key: key_path,
        ca_pem,
        hosts: host_labels,
    })
}

fn san_for(host: &str) -> anyhow::Result<SanType> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(SanType::IpAddress(ip));
    }
    if host.len() > 253 {
        bail!("host '{host}' is too long");
    }
    if !host
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
    {
        bail!("host '{host}' contains invalid characters");
    }
    for label in host.split('.') {
        if label.is_empty() || label.len() > 63 || label.starts_with('-') || label.ends_with('-') {
            bail!("host '{host}' is not a valid DNS name");
        }
    }
    let name = host
        .try_into()
        .map_err(|_| anyhow::anyhow!("host '{host}' is not a valid DNS name"))?;
    Ok(SanType::DnsName(name))
}

/// Best-effort primary LAN IPv4 address (no packets are sent; the UDP connect
/// only selects a route).
pub fn detect_lan_ipv4() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let addr = socket.local_addr().ok()?;
    Some(addr.ip())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_names_are_validated() {
        assert!(san_for("192.168.1.20").is_ok());
        assert!(san_for("daybook.local").is_ok());
        assert!(san_for("bad host").is_err());
        assert!(san_for("-leading.example").is_err());
        assert!(san_for("too..many..dots").is_err());
    }

    #[test]
    fn generates_ca_and_chained_server_certificate() {
        let dir = tempfile::tempdir().unwrap();
        let generated = generate(
            dir.path(),
            &[
                "192.168.1.20".to_string(),
                "daybook.local".to_string(),
                "localhost".to_string(),
            ],
        )
        .unwrap();

        assert!(generated.ca_pem.contains("BEGIN CERTIFICATE"));
        assert_eq!(generated.hosts.len(), 3);

        let ca = std::fs::read_to_string(&generated.ca_cert).unwrap();
        let cert = std::fs::read_to_string(&generated.cert).unwrap();
        let key = std::fs::read_to_string(&generated.key).unwrap();

        assert!(ca.starts_with("-----BEGIN CERTIFICATE-----"));
        assert_eq!(cert.matches("BEGIN CERTIFICATE").count(), 2);
        assert!(key.contains("BEGIN PRIVATE KEY"));
        assert!(!ca.contains("PRIVATE KEY"));
        assert_ne!(ca, cert);
    }

    #[test]
    fn rejects_empty_host_set() {
        let dir = tempfile::tempdir().unwrap();
        assert!(generate(dir.path(), &[]).is_err());
        assert!(generate(dir.path(), &["".to_string()]).is_err());
    }

    #[tokio::test]
    async fn rustls_can_load_the_generated_pair() {
        let dir = tempfile::tempdir().unwrap();
        let generated = generate(dir.path(), &["localhost".to_string()]).unwrap();
        let config = axum_server::tls_rustls::RustlsConfig::from_pem_file(
            &generated.cert,
            &generated.key,
        )
        .await;
        assert!(config.is_ok(), "rustls rejected the generated certificate");
    }
}
