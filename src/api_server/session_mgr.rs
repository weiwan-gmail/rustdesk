use crate::ui_session_interface::Session;
use hbb_common::{log, rendezvous_proto::ConnType};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};
use uuid::Uuid;

use super::frame_store::FrameStore;
use super::handler::{HeadlessHandler, SessionState, SessionStatus};

pub type HeadlessSession = Arc<Session<HeadlessHandler>>;

#[derive(Default)]
pub struct SessionManager {
    sessions: RwLock<HashMap<String, HeadlessSession>>,
}

#[derive(Debug, Deserialize)]
pub struct ConnectRequest {
    pub peer_id: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub relay: Option<bool>,
    #[serde(default)]
    pub two_factor: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub peer_id: String,
    pub status: String,
    pub last_error: String,
    pub peer_username: String,
    pub peer_hostname: String,
    pub peer_platform: String,
    pub displays: Vec<DisplayInfoJson>,
    pub current_display: i32,
    pub is_secured: bool,
    pub is_direct: bool,
    pub fingerprint: String,
}

#[derive(Debug, Serialize)]
pub struct DisplayInfoJson {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl From<&SessionState> for SessionInfo {
    fn from(s: &SessionState) -> Self {
        Self {
            id: s.session_id.clone(),
            peer_id: s.peer_id.clone(),
            status: s.status.as_str().to_string(),
            last_error: s.last_error.clone(),
            peer_username: s.peer_username.clone(),
            peer_hostname: s.peer_hostname.clone(),
            peer_platform: s.peer_platform.clone(),
            displays: s
                .displays
                .iter()
                .map(|(x, y, w, h)| DisplayInfoJson {
                    x: *x,
                    y: *y,
                    width: *w,
                    height: *h,
                })
                .collect(),
            current_display: s.current_display,
            is_secured: s.is_secured,
            is_direct: s.is_direct,
            fingerprint: s.fingerprint.clone(),
        }
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn list(&self) -> Vec<SessionInfo> {
        self.sessions
            .read()
            .unwrap()
            .values()
            .map(|s| SessionInfo::from(&s.ui_handler.snapshot()))
            .collect()
    }

    pub fn get(&self, session_id: &str) -> Option<HeadlessSession> {
        self.sessions.read().unwrap().get(session_id).cloned()
    }

    pub fn info(&self, session_id: &str) -> Option<SessionInfo> {
        self.get(session_id)
            .map(|s| SessionInfo::from(&s.ui_handler.snapshot()))
    }

    pub fn connect(
        &self,
        req: ConnectRequest,
        frames: Arc<FrameStore>,
    ) -> Result<SessionInfo, String> {
        let peer_id = req.peer_id.trim().to_string();
        if peer_id.is_empty() {
            return Err("peer_id is required".to_string());
        }
        let session_id = Uuid::new_v4().to_string();
        let password = req.password.clone().unwrap_or_default();
        let force_relay = req.relay.unwrap_or(false);

        let handler = HeadlessHandler::new(session_id.clone(), peer_id.clone(), frames);
        let session: Session<HeadlessHandler> = Session {
            password: password.clone(),
            server_keyboard_enabled: Arc::new(RwLock::new(true)),
            server_file_transfer_enabled: Arc::new(RwLock::new(true)),
            server_clipboard_enabled: Arc::new(RwLock::new(true)),
            reconnect_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            ui_handler: handler,
            ..Default::default()
        };

        session.lc.write().unwrap().initialize(
            peer_id.clone(),
            ConnType::DEFAULT_CONN,
            None,
            force_relay,
            None,
            None,
            None,
        );

        let session = Arc::new(session);
        self.sessions
            .write()
            .unwrap()
            .insert(session_id.clone(), session.clone());

        log::info!("headless connect session={session_id} peer={peer_id}");
        session.reconnect(force_relay);

        // Give the connection a short head-start; status remains pollable via GET.
        std::thread::sleep(Duration::from_millis(200));

        if let Some(code) = req.two_factor.clone() {
            // If 2FA is needed it will be requested asynchronously; caller can also POST login.
            let _ = code;
        }

        // If password was provided and we later enter waiting_password, login() can be used.
        // Hash handshake usually consumes Session.password automatically.
        Ok(SessionInfo::from(&session.ui_handler.snapshot()))
    }

    pub fn login(
        &self,
        session_id: &str,
        password: String,
        two_factor: Option<String>,
    ) -> Result<SessionInfo, String> {
        let session = self
            .get(session_id)
            .ok_or_else(|| "session not found".to_string())?;
        session.login(String::new(), String::new(), password, true);
        if let Some(code) = two_factor {
            if !code.is_empty() {
                session.send2fa(code, false);
            }
        }
        Ok(SessionInfo::from(&session.ui_handler.snapshot()))
    }

    pub fn disconnect(&self, session_id: &str, frames: &FrameStore) -> Result<(), String> {
        let session = self
            .sessions
            .write()
            .unwrap()
            .remove(session_id)
            .ok_or_else(|| "session not found".to_string())?;
        session.close();
        if let Ok(mut s) = session.ui_handler.state.write() {
            s.status = SessionStatus::Disconnected;
        }
        frames.remove_session(session_id);
        Ok(())
    }
}
