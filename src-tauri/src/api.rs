//! Local REST + WebSocket API (E-03).
//!
//! Binds to 127.0.0.1 only. All scan endpoints reuse the same `ScannerHandle`
//! as the GUI/CLI, so behaviour and results cannot drift. WebSocket clients
//! receive the same JSON events as the frontend via `EventSink`.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, oneshot};

use crate::scanner::types::{ScanParams, ScanTarget};
use crate::scanner::engine::ControlMsg;
use crate::scanner::{EventSink, ScannerHandle};
use crate::state::{LoggerHandle, TrafficLogHandle};

/// Handle used by the Tauri command to stop the API server gracefully.
pub struct ApiServerHandle {
    shutdown: Option<oneshot::Sender<()>>,
    pub port: u16,
}

impl ApiServerHandle {
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

#[derive(Clone)]
struct ApiState {
    scanner: Arc<ScannerHandle>,
    traffic: Arc<TrafficLogHandle>,
    loggers: Arc<LoggerHandle>,
    events: broadcast::Sender<Value>,
    auth: Arc<ApiAuth>,
}

#[derive(Debug)]
struct ApiAuth {
    token: String,
    port: u16,
}
impl ApiAuth {
    fn from_env(port: u16) -> Result<Self, String> {
        let token = std::env::var("MODBUS_SCANNER_API_TOKEN").map_err(|_| {
            "local API disabled: set MODBUS_SCANNER_API_TOKEN (>= 24 characters)".to_string()
        })?;
        if token.len() < 24 || token.bytes().any(|c| !c.is_ascii_graphic()) {
            return Err("MODBUS_SCANNER_API_TOKEN must contain >= 24 non-whitespace ASCII characters".into());
        }
        Ok(Self { token, port })
    }
    fn permitted_origin(&self, headers: &HeaderMap) -> bool {
        match headers.get(header::ORIGIN) {
            None => true, // non-browser automation must still authenticate
            Some(origin) => {
                let origin = origin.to_str().unwrap_or("");
                origin == format!("http://127.0.0.1:{}", self.port) ||
                    origin == format!("http://localhost:{}", self.port)
            }
        }
    }
    fn valid_bearer(&self, headers: &HeaderMap) -> bool {
        let provided = headers.get(header::AUTHORIZATION)
            .and_then(|s| s.to_str().ok()).unwrap_or("");
        let expected = format!("Bearer {}", self.token);
        // Equal-length constant-time comparison to avoid leaking token prefix.
        let a = provided.as_bytes();
        let b = expected.as_bytes();
        a.len() == b.len() &&
            a.iter().zip(b.iter()).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
    }
}

async fn authorize(
    State(auth): State<Arc<ApiAuth>>,
    request: Request,
    next: Next,
) -> Response {
    if !auth.permitted_origin(request.headers()) {
        return (StatusCode::FORBIDDEN, Json(json!({"error":"cross-origin API access denied"})))
            .into_response();
    }
    if !auth.valid_bearer(request.headers()) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error":"Bearer API token required"})))
            .into_response();
    }
    next.run(request).await
}

#[derive(Debug, Deserialize)]
struct StartScanBody {
    target: ScanTarget,
    params: ScanParams,
}

struct ApiError(String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, Json(json!({ "error": self.0 }))).into_response()
    }
}

impl From<String> for ApiError {
    fn from(value: String) -> Self {
        Self(value)
    }
}

async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "modbus-scanner-api",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp_ms": crate::scanner::types::now_ms(),
    }))
}

async fn get_status(State(state): State<ApiState>) -> Json<Value> {
    Json(json!({
        "status": state.scanner.status(),
        "progress": state.scanner.progress(),
    }))
}

async fn get_results(State(state): State<ApiState>) -> Json<Value> {
    Json(json!(state.scanner.results()))
}

async fn get_traffic(State(state): State<ApiState>) -> Json<Value> {
    Json(json!(state.traffic.snapshot()))
}

async fn start_scan(
    State(state): State<ApiState>,
    Json(body): Json<StartScanBody>,
) -> Result<Json<Value>, ApiError> {
    let events = state.events.clone();
    let sink = EventSink::new(
        Arc::new(move |name, payload| {
            let _ = events.send(json!({ "event": name, "payload": payload }));
        }),
        Some(state.traffic.clone()),
        Some(state.loggers.clone()),
    );
    state.scanner.start(sink, body.target, body.params).await?;
    Ok(Json(json!({ "status": "started" })))
}

async fn stop_scan(State(state): State<ApiState>) -> Result<Json<Value>, ApiError> {
    state.scanner.control(ControlMsg::Stop).await?;
    // Cancellation is asynchronous; the terminal Stopped state is reported
    // only after the engine has aborted workers and dropped its transport.
    Ok(Json(json!({ "status": "stopping" })))
}

async fn pause_scan(State(state): State<ApiState>) -> Result<Json<Value>, ApiError> {
    state.scanner.control(ControlMsg::Pause).await?;
    Ok(Json(json!({ "status": "paused" })))
}

async fn resume_scan(State(state): State<ApiState>) -> Result<Json<Value>, ApiError> {
    state.scanner.control(ControlMsg::Resume).await?;
    Ok(Json(json!({ "status": "running" })))
}

async fn websocket(State(state): State<ApiState>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| websocket_loop(socket, state.events.subscribe()))
}

async fn websocket_loop(mut socket: WebSocket, mut rx: broadcast::Receiver<Value>) {
    loop {
        match rx.recv().await {
            Ok(value) => {
                let text = value.to_string();
                if socket.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                let _ = socket
                    .send(Message::Text(json!({ "warning": "lagged", "skipped": skipped }).to_string()))
                    .await;
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

fn router(state: ApiState) -> Router {
    let guard = state.auth.clone();
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/status", get(get_status))
        .route("/api/v1/results", get(get_results))
        .route("/api/v1/traffic", get(get_traffic))
        .route("/api/v1/scan", post(start_scan))
        .route("/api/v1/scan/stop", post(stop_scan))
        .route("/api/v1/scan/pause", post(pause_scan))
        .route("/api/v1/scan/resume", post(resume_scan))
        .route("/api/v1/events", get(websocket))
        // Every endpoint is protected, including WebSocket upgrade and health.
        .route_layer(middleware::from_fn_with_state(guard, authorize))
        .with_state(state)
}

/// Start the server; returns a handle that can request graceful shutdown.
pub async fn start(
    port: u16,
    scanner: Arc<ScannerHandle>,
    traffic: Arc<TrafficLogHandle>,
    loggers: Arc<LoggerHandle>,
) -> Result<ApiServerHandle, String> {
    let auth = Arc::new(ApiAuth::from_env(port)?);
    let (tx, _) = broadcast::channel(256);
    let state = ApiState {
        scanner,
        traffic,
        loggers,
        events: tx,
        auth,
    };
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port)))
        .await
        .map_err(|e| format!("failed to bind 127.0.0.1:{port}: {e}"))?;
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, router(state))
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
        {
            tracing::warn!("local API server error: {error}");
        }
    });
    Ok(ApiServerHandle {
        shutdown: Some(shutdown_tx),
        port,
    })
}

/// Headless server used by the `serve` CLI subcommand.
pub async fn serve_headless(port: u16) -> Result<(), String> {
    let handle = start(
        port,
        Arc::new(ScannerHandle::new()),
        Arc::new(TrafficLogHandle::new()),
        Arc::new(LoggerHandle::new()),
    )
    .await?;
    println!("Modbus Scanner API listening on http://127.0.0.1:{port} (Bearer token required)");
    println!("Press Ctrl+C to stop.");
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            let mut handle = handle;
            handle.stop();
        }
        Err(e) => return Err(e.to_string()),
    }
    Ok(())
}

#[cfg(test)]
mod security_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn test_state(port: u16) -> ApiState {
        ApiState {
            scanner: Arc::new(ScannerHandle::new()),
            traffic: Arc::new(TrafficLogHandle::new()),
            loggers: Arc::new(LoggerHandle::new()),
            events: broadcast::channel(16).0,
            auth: Arc::new(ApiAuth {
                token: "0123456789abcdef0123456789abcdef".into(),
                port,
            }),
        }
    }

    async fn request(port: u16, path: &str, headers: &str) -> String {
        let mut socket = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let message = format!(
            "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n{headers}\r\n"
        );
        socket.write_all(message.as_bytes()).await.unwrap();
        let mut body = Vec::new();
        tokio::time::timeout(std::time::Duration::from_secs(3), socket.read_to_end(&mut body))
            .await.unwrap().unwrap();
        String::from_utf8(body).unwrap()
    }

    #[tokio::test]
    async fn rest_and_websocket_reject_missing_token_and_cross_site_origin() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, router(test_state(port))).await;
        });
        let no_token = request(port, "/api/v1/status", "").await;
        assert!(no_token.starts_with("HTTP/1.1 401"), "{no_token}");
        let wrong_token = request(port, "/health", "Authorization: Bearer invalid\r\n").await;
        assert!(wrong_token.starts_with("HTTP/1.1 401"), "{wrong_token}");
        let good = "Authorization: Bearer 0123456789abcdef0123456789abcdef\r\n";
        let status = request(port, "/api/v1/status", good).await;
        assert!(status.starts_with("HTTP/1.1 200"), "{status}");
        let cross_origin = format!("{good}Origin: https://evil.example\r\n");
        let forbidden = request(port, "/api/v1/status", &cross_origin).await;
        assert!(forbidden.starts_with("HTTP/1.1 403"), "{forbidden}");
        let malicious_ws = format!(
            "{good}Origin: https://evil.example\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n"
        );
        let forbidden_ws = request(port, "/api/v1/events", &malicious_ws).await;
        assert!(forbidden_ws.starts_with("HTTP/1.1 403"), "{forbidden_ws}");
        let localhost_origin = format!("{good}Origin: http://localhost:{port}\r\n");
        let same_origin = request(port, "/health", &localhost_origin).await;
        assert!(same_origin.starts_with("HTTP/1.1 200"), "{same_origin}");
        task.abort();
        let _ = task.await;
    }

    #[test]
    fn auth_token_validation_does_not_depend_on_origin() {
        let auth = ApiAuth {
            token: "0123456789abcdef0123456789abcdef".into(),
            port: 8787,
        };
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Bearer 0123456789abcdef0123456789abcdef".parse().unwrap());
        assert!(auth.valid_bearer(&headers));
        headers.insert(header::ORIGIN, "https://external.example".parse().unwrap());
        assert!(!auth.permitted_origin(&headers));
        headers.insert(header::ORIGIN, "http://127.0.0.1:8787".parse().unwrap());
        assert!(auth.permitted_origin(&headers));
    }
}
