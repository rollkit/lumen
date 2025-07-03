use std::{sync::Arc, time::Instant};

use alloy_primitives::B256;
use bytes::Bytes;
use governor::{
    clock::DefaultClock,
    state::{direct::NotKeyed, InMemoryState},
    Quota, RateLimiter,
};
use reqwest::StatusCode;
use serde_json::json;
use thiserror::Error;
use tokio::sync::Semaphore;

use tracing::debug;

/// Initialize metrics
fn init_metrics() {
    metrics::describe_histogram!(
        "tx_forwarder_latency_ms",
        "End-to-end latency to the sequencer (ms)"
    );
    metrics::describe_counter!(
        "tx_forwarder_errors_total",
        "Total errors encountered while forwarding"
    );
}

/// Transaction forwarder for submitting transactions to the sequencer
#[derive(Clone, Debug)]
pub struct TxForwarder {
    client: reqwest::Client,
    endpoint: reqwest::Url,
    limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    queue: Arc<Semaphore>,
    /// Optional HTTP Basic-Auth header value (`"Basic base64(username:password)"`).
    auth_header: Option<String>,
}

impl TxForwarder {
    /// Construct a new forwarder.
    ///
    /// * `endpoint`  – The sequencer endpoint (e.g. <http://localhost:8547>).
    /// * `queue_size` – Maximum number of in-flight requests (mapped onto a semaphore).
    /// * `rate_limit_per_sec` – Maximum POST requests per second sent to the sequencer.
    pub fn new(
        endpoint: reqwest::Url,
        queue_size: usize,
        rate_limit_per_sec: u32,
        auth_header: Option<String>,
        client: Option<reqwest::Client>,
    ) -> Self {
        // Initialize metrics on first creation
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| {
            init_metrics();
        });

        let quota = Quota::per_second(
            core::num::NonZeroU32::new(rate_limit_per_sec)
                .expect("rate_limit_per_sec must be non-zero"),
        );
        Self {
            client: client.unwrap_or_default(),
            endpoint,
            limiter: Arc::new(RateLimiter::direct(quota)),
            queue: Arc::new(Semaphore::new(queue_size)),
            auth_header,
        }
    }

    /// Forward a raw RLP-encoded transaction to the sequencer and return the hash it reports.
    ///
    /// The function:
    /// 1. Waits for a queue permit (bounded concurrency).
    /// 2. Observes rate-limit\n
    /// 3. POSTs `raw_tx` bytes as-is (JSON RPC 2.0 `eth_sendRawTransaction`).\n
    /// 4. Records latency & error metrics.\n
    /// 5. Maps failures into [`ForwardError`].
    pub async fn forward_raw(&self, raw_tx: Bytes) -> Result<B256, ForwardError> {
        // Step 1 – queue bound
        let _permit = self
            .queue
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ForwardError::Shutdown)?;

        // Step 2 – rate-limit (this is a lightweight async wait)
        self.limiter.until_ready().await;

        // Step 3 – POST
        let start = Instant::now();
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "eth_sendRawTransaction",
            "params": [format!("0x{}", hex::encode(&raw_tx))],
            "id": 1u64,
        });

        debug!(endpoint=%self.endpoint, "Forwarding tx to sequencer");
        let mut req = self.client.post(self.endpoint.clone()).json(&payload);
        // <add attach auth header if present>
        if let Some(ref hdr) = self.auth_header {
            req = req.header(reqwest::header::AUTHORIZATION, hdr.clone());
        }
        // </add>
        let resp = req.send().await.map_err(ForwardError::Network)?;

        let latency_ms = start.elapsed().as_millis() as f64;
        metrics::histogram!("tx_forwarder_latency_ms").record(latency_ms);

        // Step 4 – map HTTP status
        if !resp.status().is_success() {
            let class = resp.status().as_u16().to_string();
            metrics::counter!("tx_forwarder_errors_total",  "class" => class);
            return Err(ForwardError::HttpStatus(resp.status()));
        }

        // Step 5 – parse JSON-RPC result { "result": "0x…" }
        let json: serde_json::Value = resp.json().await.map_err(ForwardError::InvalidJson)?;
        if let Some(result) = json.get("result").and_then(|v| v.as_str()) {
            let hash = result.trim_start_matches("0x");
            let mut b256_bytes = [0u8; 32];
            hex::decode_to_slice(hash, &mut b256_bytes).map_err(|_| ForwardError::InvalidHash)?;
            return Ok(B256::from(b256_bytes));
        }

        if json.get("error").is_some() {
            metrics::counter!("tx_forwarder_errors_total", "class" => "upstream");
            return Err(ForwardError::UpstreamError(json));
        }

        metrics::counter!("tx_forwarder_errors_total", "class" => "invalid_body");
        Err(ForwardError::UnexpectedBody(json))
    }
}

/* -------------------------------------------------------------------------- */
/*                                   Error                                    */
/* -------------------------------------------------------------------------- */

/// Errors that can occur during transaction forwarding
#[derive(Debug, Error)]
pub enum ForwardError {
    /// Service is shutting down
    #[error("Service shutting down")]
    Shutdown,
    /// Request was rate-limited
    #[error("Rate-limited")]
    RateLimited,
    /// Network error occurred
    #[error("Network error: {0}")]
    Network(reqwest::Error),
    /// Sequencer returned non-success HTTP status
    #[error("Sequencer returned HTTP status {0}")]
    HttpStatus(StatusCode),
    /// Failed to parse JSON response
    #[error("Invalid JSON body")]
    InvalidJson(reqwest::Error),
    /// Response body was unexpected
    #[error("Unexpected body: {0:?}")]
    UnexpectedBody(serde_json::Value),
    /// Transaction hash in response was invalid
    #[error("Invalid transaction hash")]
    InvalidHash,
    /// Sequencer returned a JSON-RPC error object
    #[error("Upstream JSON-RPC error: {0}")]
    UpstreamError(serde_json::Value),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    #[tokio::test]
    async fn happy_path() {
        // Spin up a mock sequencer.
        let server = MockServer::start().await;

        // Mock 200 OK with a zero tx-hash.
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": format!("0x{}", "00".repeat(32))
        });
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let forwarder = TxForwarder::new(
            server.uri().parse().unwrap(),
            /*queue_size*/ 10,
            /*rate_limit*/ 1_000,
            /*auth*/ None,
            None,
        );

        let hash = forwarder
            .forward_raw(Bytes::from_static(b"\x01\x02"))
            .await
            .expect("forwarding should succeed");

        assert_eq!(hash, B256::ZERO);
    }

    #[tokio::test]
    async fn http_error() {
        let server = MockServer::start().await;

        // Mock 500 Internal Server Error.
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let forwarder = TxForwarder::new(server.uri().parse().unwrap(), 10, 1_000, None, None);

        let err = forwarder
            .forward_raw(Bytes::from_static(b"\x03\x04"))
            .await
            .expect_err("should return error");

        matches!(err, ForwardError::HttpStatus(status) if status == reqwest::StatusCode::INTERNAL_SERVER_ERROR);
    }
}
