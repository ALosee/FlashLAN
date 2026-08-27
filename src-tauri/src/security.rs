use base64::Engine as _;
use rcgen::{generate_simple_self_signed, CertifiedKey};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tokio_rustls::{TlsAcceptor, TlsConnector};

/// Every FlashLAN instance owns a self-signed certificate. The SHA-256 of
/// its DER encoding is the device fingerprint exchanged during pairing.
pub struct Identity {
    certificate: CertificateDer<'static>,
    private_key: PrivateKeyDer<'static>,
    pub fingerprint: String,
}

impl Identity {
    pub fn load_or_create(dir: &Path) -> Result<Self, String> {
        fs::create_dir_all(dir).map_err(|error| format!("无法创建密钥目录：{error}"))?;
        let cert_path = dir.join("device.pem");
        let key_path = dir.join("device.key");

        let (certificate, private_key) = if cert_path.exists() && key_path.exists() {
            let cert_pem =
                fs::read_to_string(&cert_path).map_err(|error| format!("无法读取证书：{error}"))?;
            let key_pem =
                fs::read_to_string(&key_path).map_err(|error| format!("无法读取私钥：{error}"))?;
            let mut certs = rustls_pemfile::certs(&mut cert_pem.as_bytes())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("证书解析失败：{error}"))?;
            let key = rustls_pemfile::private_key(&mut key_pem.as_bytes())
                .map_err(|error| format!("私钥解析失败：{error}"))?
                .ok_or("私钥文件为空")?;
            let certificate = certs.pop().ok_or("证书文件为空")?;
            (certificate, key)
        } else {
            let CertifiedKey { cert, signing_key } =
                generate_simple_self_signed(vec!["flashlan.local".into()])
                    .map_err(|error| format!("生成证书失败：{error}"))?;
            fs::write(&cert_path, cert.pem())
                .and_then(|_| fs::write(&key_path, signing_key.serialize_pem()))
                .map_err(|error| format!("保存密钥失败：{error}"))?;
            (
                cert.into(),
                PrivateKeyDer::Pkcs8(signing_key.serialize_der().into()),
            )
        };

        let fingerprint = sha256_hex(certificate.as_ref());
        Ok(Self {
            certificate,
            private_key,
            fingerprint,
        })
    }

    pub fn server_acceptor(&self) -> Result<TlsAcceptor, String> {
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![self.certificate.clone()], self.private_key.clone_key())
            .map_err(|error| format!("TLS 服务端配置失败：{error}"))?;
        Ok(TlsAcceptor::from(Arc::new(config)))
    }

    /// Client config that accepts any certificate during the handshake; the
    /// actual authentication is out-of-band: after the TLS session is up the
    /// receiver sends its certificate fingerprint and the sender checks it
    /// against the local trust list before transferring any bytes.
    pub fn client_connector(&self) -> Result<TlsConnector, String> {
        struct NoVerify;

        #[allow(unused_qualifications)]
        impl rustls::client::danger::ServerCertVerifier for NoVerify {
            fn verify_server_cert(
                &self,
                _end_entity: &CertificateDer<'_>,
                _intermediates: &[CertificateDer<'_>],
                _server_name: &ServerName<'_>,
                _ocsp_response: &[u8],
                _now: rustls::pki_types::UnixTime,
            ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
                Ok(rustls::client::danger::ServerCertVerified::assertion())
            }
            fn verify_tls12_signature(
                &self,
                _message: &[u8],
                _cert: &CertificateDer<'_>,
                _dss: &rustls::DigitallySignedStruct,
            ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
            {
                Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
            }
            fn verify_tls13_signature(
                &self,
                _message: &[u8],
                _cert: &CertificateDer<'_>,
                _dss: &rustls::DigitallySignedStruct,
            ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
            {
                Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
            }
            fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
                vec![
                    rustls::SignatureScheme::RSA_PKCS1_SHA256,
                    rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
                    rustls::SignatureScheme::ED25519,
                    rustls::SignatureScheme::RSA_PSS_SHA256,
                ]
            }
        }

        impl std::fmt::Debug for NoVerify {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("NoVerify")
            }
        }

        let builder = rustls::ClientConfig::builder().dangerous();
        let config = builder
            .with_custom_certificate_verifier(Arc::new(NoVerify))
            .with_client_auth_cert(vec![self.certificate.clone()], self.private_key.clone_key())
            .map_err(|error| format!("TLS 客户端配置失败：{error}"))?;
        Ok(TlsConnector::from(Arc::new(config)))
    }
}

/// fingerprint hex -> display name of the paired device.
#[derive(Default)]
pub struct TrustStore {
    path: Mutex<Option<PathBuf>>,
    trusted: Mutex<HashMap<String, TrustedDevice>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedDevice {
    pub name: String,
    #[serde(default)]
    pub paired_at: u64,
}

impl TrustStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bind_path(&self, path: PathBuf) {
        if let Ok(contents) = fs::read_to_string(&path) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, TrustedDevice>>(&contents) {
                *self.trusted.lock().unwrap() = map;
            }
        }
        *self.path.lock().unwrap() = Some(path);
    }

    pub fn is_trusted(&self, fingerprint: &str) -> bool {
        self.trusted.lock().unwrap().contains_key(fingerprint)
    }

    pub fn add(&self, fingerprint: String, device: TrustedDevice) -> Result<(), String> {
        {
            let mut map = self.trusted.lock().unwrap();
            map.insert(fingerprint, device);
        }
        self.persist()
    }

    pub fn remove(&self, fingerprint: &str) -> Result<(), String> {
        {
            let mut map = self.trusted.lock().unwrap();
            map.remove(fingerprint);
        }
        self.persist()
    }

    pub fn list(&self) -> Vec<(String, TrustedDevice)> {
        self.trusted
            .lock()
            .unwrap()
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }

    fn persist(&self) -> Result<(), String> {
        let path_guard = self.path.lock().unwrap();
        let Some(path) = path_guard.as_ref() else {
            return Ok(()); // not bound yet (tests / early startup)
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("无法创建配置目录：{error}"))?;
        }
        let map = self.trusted.lock().unwrap();
        let contents = serde_json::to_string_pretty(&*map).map_err(|error| error.to_string())?;
        fs::write(path, contents).map_err(|error| format!("无法保存信任列表：{error}"))
    }
}

pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    hex::encode(digest)
}

pub fn encode_b64(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_roundtrip_and_fingerprint_stable() {
        let dir = std::env::temp_dir().join(format!("flashlan-id-{}", uuid::Uuid::new_v4()));
        let first = Identity::load_or_create(&dir).unwrap();
        let second = Identity::load_or_create(&dir).unwrap();
        assert_eq!(first.fingerprint, second.fingerprint);
        assert_eq!(first.fingerprint.len(), 64);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn trust_store_persists_and_removes() {
        let path =
            std::env::temp_dir().join(format!("flashlan-trust-{}.json", uuid::Uuid::new_v4()));
        let store = TrustStore::new();
        store.bind_path(path.clone());
        store
            .add(
                "abc123".into(),
                TrustedDevice {
                    name: "Test Phone".into(),
                    paired_at: 42,
                },
            )
            .unwrap();
        assert!(store.is_trusted("abc123"));

        // A fresh store reading the same file sees the entry.
        let reloaded = TrustStore::new();
        reloaded.bind_path(path.clone());
        assert!(reloaded.is_trusted("abc123"));
        assert_eq!(reloaded.list()[0].1.name, "Test Phone");

        reloaded.remove("abc123").unwrap();
        assert!(!reloaded.is_trusted("abc123"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn tls_configs_build() {
        let dir = std::env::temp_dir().join(format!("flashlan-tls-{}", uuid::Uuid::new_v4()));
        let identity = Identity::load_or_create(&dir).unwrap();
        assert!(identity.server_acceptor().is_ok());
        assert!(identity.client_connector().is_ok());
        let _ = fs::remove_dir_all(&dir);
    }
}
