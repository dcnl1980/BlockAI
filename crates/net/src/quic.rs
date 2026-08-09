use crate::frame::{decode_frame, encode_frame, AppFrame, FrameError};
use crate::policy::{admit_frame, AdmitError};
use quinn::{ClientConfig, Endpoint, RecvStream, SendStream, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum QuicError {
    #[error(transparent)]
    Frame(#[from] FrameError),
    #[error(transparent)]
    Admit(#[from] AdmitError),
    #[error("quic io: {0}")]
    Io(String),
}

fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// Generate a self-signed TLS identity for lab/sim only.
/// These keys are transport keys and must never sign PAY / capabilities.
pub fn generate_transport_tls() -> Result<(CertificateDer<'static>, PrivateKeyDer<'static>), Box<dyn Error + Send + Sync>> {
    install_crypto_provider();
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let cert_der = CertificateDer::from(cert.cert);
    let key_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()));
    Ok((cert_der, key_der))
}

pub fn make_server_endpoint(addr: SocketAddr) -> Result<(Endpoint, CertificateDer<'static>), Box<dyn Error + Send + Sync>> {
    let (cert, key) = generate_transport_tls()?;
    let mut server_crypto = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert.clone()], key)?;
    // Quinn/rustls require 0 or 2^32-1. Allow early data for idempotent reads;
    // application policy still rejects PAY on 0-RTT.
    server_crypto.max_early_data_size = u32::MAX;
    server_crypto.alpn_protocols = vec![b"blockai-seef/1".to_vec()];
    let mut server_config = ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto)?,
    ));
    server_config.transport = Arc::new({
        let mut t = quinn::TransportConfig::default();
        t.max_concurrent_bidi_streams(16u32.into());
        t
    });
    let endpoint = Endpoint::server(server_config, addr)?;
    Ok((endpoint, cert))
}

pub fn make_client_endpoint(
    server_cert: CertificateDer<'static>,
) -> Result<Endpoint, Box<dyn Error + Send + Sync>> {
    let mut certs = rustls::RootCertStore::empty();
    certs.add(server_cert)?;
    let mut client_crypto = rustls::ClientConfig::builder()
        .with_root_certificates(certs)
        .with_no_client_auth();
    client_crypto.alpn_protocols = vec![b"blockai-seef/1".to_vec()];
    client_crypto.enable_early_data = true;
    let client_config = ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(client_crypto)?,
    ));
    let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
    endpoint.set_default_client_config(client_config);
    Ok(endpoint)
}

pub async fn send_frame(send: &mut SendStream, frame: &AppFrame) -> Result<(), QuicError> {
    let bytes = encode_frame(frame)?;
    send.write_all(&bytes)
        .await
        .map_err(|e| QuicError::Io(e.to_string()))?;
    Ok(())
}

pub async fn recv_frame(recv: &mut RecvStream) -> Result<AppFrame, QuicError> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf)
        .await
        .map_err(|e| QuicError::Io(e.to_string()))?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut body = vec![0u8; len];
    recv.read_exact(&mut body)
        .await
        .map_err(|e| QuicError::Io(e.to_string()))?;
    let mut bytes = Vec::with_capacity(4 + len);
    bytes.extend_from_slice(&len_buf);
    bytes.extend_from_slice(&body);
    Ok(decode_frame(&bytes)?)
}

/// Read one frame and enforce early-data admission policy.
pub async fn recv_admitted_frame(
    recv: &mut RecvStream,
    is_early_data: bool,
) -> Result<AppFrame, QuicError> {
    let frame = recv_frame(recv).await?;
    admit_frame(is_early_data, &frame)?;
    Ok(frame)
}
