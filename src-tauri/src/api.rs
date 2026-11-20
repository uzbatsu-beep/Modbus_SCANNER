//! Local REST + WebSocket API (E-03).
//!
//! Binds to 127.0.0.1 only. All scan endpoints reuse the same `ScannerHandle`
//! as the GUI/CLI, so behaviour and results cannot drift. WebSocket clients
//! receive the same JSON events as the frontend via `EventSink`.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::StatusCode;
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
        .with_state(state)
}

/// Start the server; returns a handle that can request graceful shutdown.
pub async fn start(
    port: u16,
    scanner: Arc<ScannerHandle>,
    traffic: Arc<TrafficLogHandle>,
    loggers: Arc<LoggerHandle>,
) -> Result<ApiServerHandle, String> {
    let (tx, _) = broadcast::channel(256);
    let state = ApiState {
        scanner,
        traffic,
        loggers,
        events: tx,
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
    println!("Modbus Scanner API listening on http://127.0.0.1:{port}");
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
