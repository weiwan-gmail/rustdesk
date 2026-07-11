use axum::extract::ws::{Message, WebSocket};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Duration;

use super::AppState;

pub async fn handle_stream(
    socket: WebSocket,
    state: Arc<AppState>,
    session_id: String,
    display: usize,
    fps: f32,
    format: String,
    quality: u8,
) {
    let (mut sink, mut stream) = socket.split();
    let interval = Duration::from_secs_f32(1.0 / fps.max(0.2));
    let mut ticker = hbb_common::tokio::time::interval(interval);
    let mut last_updated = 0u64;

    loop {
        hbb_common::tokio::select! {
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(p))) => {
                        let _ = sink.send(Message::Pong(p)).await;
                    }
                    Some(Ok(Message::Text(t))) if t == "ping" => {
                        let _ = sink.send(Message::Text("pong".into())).await;
                    }
                    _ => {}
                }
            }
            _ = ticker.tick() => {
                if state.sessions.get(&session_id).is_none() {
                    let _ = sink.send(Message::Close(None)).await;
                    break;
                }
                let Some(frame) = state.frames.get(&session_id, display) else {
                    continue;
                };
                if frame.updated_at_ms == last_updated {
                    continue;
                }
                last_updated = frame.updated_at_ms;
                let encoded = if format.eq_ignore_ascii_case("png") {
                    frame.encode_png()
                } else {
                    frame.encode_jpeg(quality)
                };
                let Ok(bytes) = encoded else { continue };
                let payload = serde_json::json!({
                    "width": frame.width,
                    "height": frame.height,
                    "updated_at_ms": frame.updated_at_ms,
                    "mime": if format.eq_ignore_ascii_case("png") { "image/png" } else { "image/jpeg" },
                    "image_base64": B64.encode(&bytes),
                });
                if sink
                    .send(Message::Text(payload.to_string().into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    }
}
