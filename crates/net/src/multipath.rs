use crate::quic::{make_client_endpoint, QuicError};
use quinn::Connection;
use rustls::pki_types::CertificateDer;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::task::JoinSet;

/// Application-level multipath race: dial several paths, first completed 1-RTT wins.
/// Does not enable 0-RTT PAY; callers still use admit_frame on early data.
pub async fn race_connect(
    paths: Vec<(SocketAddr, CertificateDer<'static>)>,
    server_name: &str,
    per_path_timeout: Duration,
) -> Result<(Connection, SocketAddr), QuicError> {
    if paths.is_empty() {
        return Err(QuicError::Io("no paths".into()));
    }
    let mut set = JoinSet::new();
    for (addr, cert) in paths {
        let name = server_name.to_string();
        set.spawn(async move {
            let endpoint = make_client_endpoint(cert).map_err(|e| e.to_string())?;
            let connecting = endpoint
                .connect(addr, &name)
                .map_err(|e| e.to_string())?;
            let conn = tokio::time::timeout(per_path_timeout, connecting)
                .await
                .map_err(|_| "path timeout".to_string())?
                .map_err(|e| e.to_string())?;
            Ok::<_, String>((conn, addr))
        });
    }

    let mut last_err = QuicError::Io("all paths failed".into());
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok(Ok((conn, addr))) => {
                set.abort_all();
                return Ok((conn, addr));
            }
            Ok(Err(e)) => last_err = QuicError::Io(e),
            Err(e) => last_err = QuicError::Io(e.to_string()),
        }
    }
    Err(last_err)
}
