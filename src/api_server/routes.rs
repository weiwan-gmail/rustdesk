use axum::{
    extract::{
        ws::WebSocketUpgrade,
        Path, Query, State,
    },
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use super::auth::require_token;
use super::clipboard::{self, ClipboardSetRequest};
use super::input::{self, InputAction};
use super::session_mgr::{ConnectRequest, SessionInfo};
use super::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/sessions", get(list_sessions).post(create_session))
        .route(
            "/api/v1/sessions/:id",
            get(get_session).delete(delete_session),
        )
        .route("/api/v1/sessions/:id/login", post(login_session))
        .route("/api/v1/sessions/:id/screen/latest", get(screen_latest))
        .route("/api/v1/sessions/:id/screen/stream", get(screen_stream))
        .route("/api/v1/sessions/:id/input/action", post(input_action))
        .route(
            "/api/v1/sessions/:id/clipboard",
            get(get_clipboard).post(set_clipboard),
        )
        .route("/api/v1/sessions/:id/clipboard/copy", post(clipboard_copy))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_token,
        ))
        .layer(cors)
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "ok": true, "service": "rustdesk-api-server" }))
}

async fn list_sessions(State(state): State<Arc<AppState>>) -> Json<Vec<SessionInfo>> {
    Json(state.sessions.list())
}

async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ConnectRequest>,
) -> Result<Json<SessionInfo>, (StatusCode, String)> {
    state
        .sessions
        .connect(req, state.frames.clone())
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

async fn get_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<SessionInfo>, (StatusCode, String)> {
    state
        .sessions
        .info(&id)
        .map(Json)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "session not found".to_string()))
}

async fn delete_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .sessions
        .disconnect(&id, &state.frames)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| (StatusCode::NOT_FOUND, e))
}

#[derive(Debug, Deserialize)]
pub struct LoginBody {
    pub password: String,
    #[serde(default)]
    pub two_factor: Option<String>,
}

async fn login_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<LoginBody>,
) -> Result<Json<SessionInfo>, (StatusCode, String)> {
    state
        .sessions
        .login(&id, body.password, body.two_factor)
        .map(Json)
        .map_err(|e| (StatusCode::NOT_FOUND, e))
}

#[derive(Debug, Deserialize)]
pub struct ScreenQuery {
    #[serde(default)]
    pub display: Option<usize>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub encoding: Option<String>,
    #[serde(default)]
    pub quality: Option<u8>,
}

#[derive(Debug, Serialize)]
pub struct ScreenJson {
    pub width: usize,
    pub height: usize,
    pub format: String,
    pub updated_at_ms: u64,
    pub image_base64: String,
    pub mime: String,
}

async fn screen_latest(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<ScreenQuery>,
) -> Result<Response, (StatusCode, String)> {
    if state.sessions.get(&id).is_none() {
        return Err((StatusCode::NOT_FOUND, "session not found".to_string()));
    }
    let display = q.display.unwrap_or(0);
    let frame = state
        .frames
        .get(&id, display)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "no frame available yet".to_string()))?;

    let fmt = q.format.as_deref().unwrap_or("jpeg").to_lowercase();
    let quality = q.quality.unwrap_or(80);
    let (bytes, mime) = match fmt.as_str() {
        "png" => (
            frame
                .encode_png()
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?,
            "image/png",
        ),
        _ => (
            frame
                .encode_jpeg(quality)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?,
            "image/jpeg",
        ),
    };

    let encoding = q.encoding.as_deref().unwrap_or("raw").to_lowercase();
    if encoding == "json" || encoding == "base64" {
        let body = ScreenJson {
            width: frame.width,
            height: frame.height,
            format: frame.format,
            updated_at_ms: frame.updated_at_ms,
            image_base64: B64.encode(&bytes),
            mime: mime.to_string(),
        };
        return Ok(Json(body).into_response());
    }

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, mime.parse().unwrap());
    headers.insert(
        header::HeaderName::from_static("x-frame-width"),
        frame.width.to_string().parse().unwrap(),
    );
    headers.insert(
        header::HeaderName::from_static("x-frame-height"),
        frame.height.to_string().parse().unwrap(),
    );
    headers.insert(
        header::HeaderName::from_static("x-frame-updated-at-ms"),
        frame.updated_at_ms.to_string().parse().unwrap(),
    );
    Ok((headers, bytes).into_response())
}

#[derive(Debug, Deserialize)]
pub struct StreamQuery {
    #[serde(default)]
    pub display: Option<usize>,
    #[serde(default)]
    pub fps: Option<f32>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub quality: Option<u8>,
}

async fn screen_stream(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<StreamQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if state.sessions.get(&id).is_none() {
        return Err((StatusCode::NOT_FOUND, "session not found".to_string()));
    }
    let display = q.display.unwrap_or(0);
    let fps = q.fps.unwrap_or(2.0).clamp(0.2, 15.0);
    let format = q.format.unwrap_or_else(|| "jpeg".to_string());
    let quality = q.quality.unwrap_or(70);
    Ok(ws.on_upgrade(move |socket| {
        super::ws::handle_stream(socket, state, id, display, fps, format, quality)
    }))
}

async fn input_action(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(action): Json<InputAction>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let session = state
        .sessions
        .get(&id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "session not found".to_string()))?;
    input::apply_action(&session, action)
        .map(|_| Json(serde_json::json!({ "ok": true })))
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

async fn get_clipboard(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<clipboard::ClipboardResponse>, (StatusCode, String)> {
    let session = state
        .sessions
        .get(&id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "session not found".to_string()))?;
    Ok(Json(clipboard::get_clipboard(&session)))
}

async fn set_clipboard(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ClipboardSetRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let session = state
        .sessions
        .get(&id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "session not found".to_string()))?;
    clipboard::set_clipboard_text(&session, &body.text);
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
pub struct CopyQuery {
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

async fn clipboard_copy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<CopyQuery>,
) -> Result<Json<clipboard::ClipboardResponse>, (StatusCode, String)> {
    let timeout_ms = q.timeout_ms.unwrap_or(800);
    let sessions = state.sessions.clone();
    let id2 = id.clone();
    hbb_common::tokio::task::spawn_blocking(move || {
        clipboard::copy_and_wait(&sessions, &id2, timeout_ms)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map(Json)
    .map_err(|e| (StatusCode::BAD_REQUEST, e))
}
