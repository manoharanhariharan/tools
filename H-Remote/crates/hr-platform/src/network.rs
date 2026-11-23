//! Address/port reachability used before connecting (P1-20/P7-10).

use std::time::Duration;

/// Attempts a TCP connection with a hard timeout. Any failure (DNS, refused,
/// timeout) is reported as `false`; callers decide whether to show a dialog.
pub async fn tcp_reachable(host: &str, port: u16, timeout: Duration) -> bool {
    tcp_latency(host, port, timeout).await.is_some()
}

/// Returns the connect latency in milliseconds when the TCP handshake succeeds.
pub async fn tcp_latency(host: &str, port: u16, timeout: Duration) -> Option<u128> {
    if host.trim().is_empty() || port == 0 {
        return None;
    }
    let started = std::time::Instant::now();
    match tokio::time::timeout(timeout, tokio::net::TcpStream::connect((host, port))).await {
        Ok(Ok(stream)) => {
            let _ = stream.set_nodelay(true);
            Some(started.elapsed().as_millis())
        }
        Ok(Err(error)) => {
            tracing::debug!(host, port, %error, "address is not reachable");
            None
        }
        Err(_) => {
            tracing::debug!(host, port, "address reachability timed out");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn reachable_listener_returns_true() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(tcp_reachable("127.0.0.1", port, Duration::from_secs(2)).await);
        assert!(
            tcp_latency("127.0.0.1", port, Duration::from_secs(2))
                .await
                .is_some()
        );
    }

    #[tokio::test]
    async fn invalid_inputs_return_false() {
        assert!(!tcp_reachable("", 80, Duration::from_millis(100)).await);
        assert!(!tcp_reachable("127.0.0.1", 0, Duration::from_millis(100)).await);
    }

    #[tokio::test]
    async fn refused_connection_returns_false_quickly() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        assert!(!tcp_reachable("127.0.0.1", port, Duration::from_millis(500)).await);
    }
}
