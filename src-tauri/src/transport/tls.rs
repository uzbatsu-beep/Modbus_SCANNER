//! TLS configuration for Modbus Security (IEC 61158 / Modbus TCP Security).
//!
//! The specification uses port 802 and wraps the normal MBAP ADU in TLS.
//! Server certificates are validated against webpki roots; an optional client
//! certificate enables mutual TLS. Self-signed devices must have their CA
//! installed in the OS trust store (or use a client cert signed by a trusted CA).

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Arc;

use rustls::{ClientConfig, RootCertStore};
use serde::{Deserialize, Serialize};

/// TLS options; `None` in `TcpConfig` means plain Modbus TCP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TlsParams {
    /// PEM certificate chain for mutual TLS.
    #[serde(default)]
    pub client_cert: Option<PathBuf>,
    /// PEM private key for mutual TLS.
    #[serde(default)]
    pub client_key: Option<PathBuf>,
}

impl Default for TlsParams {
    fn default() -> Self {
        Self {
            client_cert: None,
            client_key: None,
        }
    }
}

/// Build a rustls client config with system/webpki roots and optional client auth.
pub fn build_client_config(params: &TlsParams) -> Result<Arc<ClientConfig>, String> {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let builder = ClientConfig::builder().with_root_certificates(roots);

    match (&params.client_cert, &params.client_key) {
        (Some(cert_path), Some(key_path)) => {
            let certs = {
                let file = File::open(cert_path).map_err(|e| format!("open client cert: {e}"))?;
                let mut reader = BufReader::new(file);
                rustls_pemfile::certs(&mut reader)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| format!("parse client cert: {e}"))?
            };
            if certs.is_empty() {
                return Err("client certificate file contains no certificates".into());
            }
            let key = {
                let file = File::open(key_path).map_err(|e| format!("open client key: {e}"))?;
                let mut reader = BufReader::new(file);
                rustls_pemfile::private_key(&mut reader)
                    .map_err(|e| format!("parse client key: {e}"))?
                    .ok_or("client key file contains no private key")?
            };
            Ok(Arc::new(
                builder
                    .with_client_auth_cert(certs, key)
                    .map_err(|e| format!("client auth config: {e}"))?,
            ))
        }
        _ => Ok(Arc::new(builder.with_no_client_auth())),
    }
}
