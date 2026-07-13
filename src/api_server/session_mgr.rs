use crate::ui_session_interface::Session;
use hbb_common::{log, rendezvous_proto::ConnType};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};
use uuid::Uuid;

use super::credentials::SharedCredentialStore;
use super::frame_store::FrameStore;
use super::handler::{HeadlessHandler, OsLoginStatus, SessionState, SessionStatus};
use super::os_login::{self, OsLoginParams};

pub type HeadlessSession = Arc<Session<HeadlessHandler>>;

#[derive(Default)]
pub struct SessionManager {
    sessions: RwLock<HashMap<String, HeadlessSession>>,
    pub credentials: Option<SharedCredentialStore>,
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
    #[serde(default)]
    pub os_username: Option<String>,
    #[serde(default)]
    pub os_password: Option<String>,
    #[serde(default)]
    pub auto_os_login: Option<bool>,
    #[serde(default)]
    pub os_login_delay_ms: Option<u64>,
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
    pub os_login_status: String,
    pub os_login_error: String,
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
            os_login_status: s.os_login_status.as_str().to_string(),
            os_login_error: s.os_login_error.clone(),
        }
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_credentials(credentials: SharedCredentialStore) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            credentials: Some(credentials),
        }
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

    fn resolve_creds(&self, req: &ConnectRequest) -> (String, String, String, String) {
        let mut password = req.password.clone().unwrap_or_default();
        let mut os_user = req.os_username.clone().unwrap_or_default();
        let mut os_pass = req.os_password.clone().unwrap_or_default();
        if let Some(store) = &self.credentials {
            if let Some(c) = store.lookup(&req.peer_id, None) {
                if password.is_empty() && !c.rustdesk_password.is_empty() {
                    password = c.rustdesk_password;
                }
                if os_user.is_empty() && !c.os_username.is_empty() {
                    os_user = c.os_username;
                }
                if os_pass.is_empty() && !c.os_password.is_empty() {
                    os_pass = c.os_password;
                }
            }
        }
        (req.peer_id.trim().to_string(), password, os_user, os_pass)
    }

    pub fn connect(
        &self,
        req: ConnectRequest,
        frames: Arc<FrameStore>,
    ) -> Result<SessionInfo, String> {
        let (peer_id, password, os_user, os_pass) = self.resolve_creds(&req);
        if peer_id.is_empty() {
            return Err("peer_id is required".to_string());
        }
        let session_id = Uuid::new_v4().to_string();
        let force_relay = req.relay.unwrap_or(false);
        let auto_os = req.auto_os_login.unwrap_or(true);
        let delay_ms = req.os_login_delay_ms.unwrap_or(2500);

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

        std::thread::sleep(Duration::from_millis(200));

        if let Some(code) = req.two_factor.clone() {
            let _ = code;
        }

        if auto_os && (!os_user.is_empty() || !os_pass.is_empty()) {
            let params = OsLoginParams {
                username: os_user,
                password: os_pass,
                activate: true,
                delay_ms,
                username_first: true,
            };
            os_login::spawn_after_connect((*session).clone(), params);
        }

        Ok(SessionInfo::from(&session.ui_handler.snapshot()))
    }

    pub fn os_login(
        &self,
        session_id: &str,
        params: OsLoginParams,
    ) -> Result<SessionInfo, String> {
        let session = self
            .get(session_id)
            .ok_or_else(|| "session not found".to_string())?;
        os_login::perform(&session, &params).map_err(|e| {
            if let Ok(mut s) = session.ui_handler.state.write() {
                s.os_login_status = OsLoginStatus::Failed;
                s.os_login_error = e.clone();
            }
            e
        })?;
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
