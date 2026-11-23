//! Explicit rustls crypto provider selection.
//!
//! Dependencies enable both `ring` (russh) and `aws-lc-rs` (via reqwest/sspi).
//! rustls refuses to guess a process default when both are present, which used
//! to panic on the first HTTPS request in release builds. H-Remote installs the
//! ring provider once at start-up so every TLS client agrees.

use std::sync::Once;

static INSTALL: Once = Once::new();

pub fn install_crypto_provider() {
    INSTALL.call_once(|| {
        // Err only means another provider was installed first, which is fine.
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installing_twice_is_safe_and_keeps_a_default() {
        install_crypto_provider();
        install_crypto_provider();
        assert!(rustls::crypto::CryptoProvider::get_default().is_some());
    }
}
