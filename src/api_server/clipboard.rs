use crate::client::{Data, Interface};
use crate::ui_session_interface::Session;
use hbb_common::{
    compress::compress as compress_func,
    log,
    message_proto::{Clipboard, ClipboardFormat, Message, MultiClipboards},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

use super::handler::HeadlessHandler;
use super::session_mgr::SessionManager;

#[derive(Debug, Deserialize)]
pub struct ClipboardSetRequest {
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct ClipboardResponse {
    pub text: String,
}

pub fn get_clipboard(session: &Session<HeadlessHandler>) -> ClipboardResponse {
    let text = session
        .ui_handler
        .state
        .read()
        .map(|s| s.clipboard_text.clone())
        .unwrap_or_default();
    ClipboardResponse { text }
}

pub fn set_clipboard_text(session: &Session<HeadlessHandler>, text: &str) {
    let compressed = compress_func(text.as_bytes());
    let compress = compressed.len() < text.as_bytes().len();
    let content = if compress {
        compressed
    } else {
        text.as_bytes().to_vec()
    };
    let cb = Clipboard {
        compress,
        content: content.into(),
        format: ClipboardFormat::Text.into(),
        ..Default::default()
    };
    let mut msg = Message::new();
    msg.set_multi_clipboards(MultiClipboards {
        clipboards: vec![cb],
        ..Default::default()
    });
    session.send(Data::Message(msg));
    if let Ok(mut s) = session.ui_handler.state.write() {
        s.clipboard_text = text.to_string();
    }
}

/// Send Ctrl+C, then wait briefly for inbound clipboard text.
pub fn copy_and_wait(
    sessions: &Arc<SessionManager>,
    session_id: &str,
    timeout_ms: u64,
) -> Result<ClipboardResponse, String> {
    let session = sessions
        .get(session_id)
        .ok_or_else(|| "session not found".to_string())?;
    let before = session
        .ui_handler
        .state
        .read()
        .map(|s| s.clipboard_text.clone())
        .unwrap_or_default();

    session.input_key("VK_C", false, true, false, true, false, false);

    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms.max(50));
    while std::time::Instant::now() < deadline {
        let now = session
            .ui_handler
            .state
            .read()
            .map(|s| s.clipboard_text.clone())
            .unwrap_or_default();
        if !now.is_empty() && now != before {
            return Ok(ClipboardResponse { text: now });
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let text = session
        .ui_handler
        .state
        .read()
        .map(|s| s.clipboard_text.clone())
        .unwrap_or_default();
    if text.is_empty() {
        log::warn!("copy_and_wait timed out with empty clipboard for {session_id}");
    }
    Ok(ClipboardResponse { text })
}
