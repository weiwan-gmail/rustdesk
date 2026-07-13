use crate::client::QualityStatus;
use crate::ui_session_interface::InvokeUiSession;
use hbb_common::{
    log,
    message_proto::*,
    rendezvous_proto::ConnType,
};
use scrap::ImageRgb;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use super::frame_store::FrameStore;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionStatus {
    Connecting,
    Connected,
    WaitingPassword,
    Waiting2fa,
    Error,
    Disconnected,
}

impl Default for SessionStatus {
    fn default() -> Self {
        Self::Connecting
    }
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::WaitingPassword => "waiting_password",
            Self::Waiting2fa => "waiting_2fa",
            Self::Error => "error",
            Self::Disconnected => "disconnected",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OsLoginStatus {
    Idle,
    Pending,
    Running,
    Done,
    Failed,
}

impl Default for OsLoginStatus {
    fn default() -> Self {
        Self::Idle
    }
}

impl OsLoginStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Default)]
pub struct SessionState {
    pub session_id: String,
    pub peer_id: String,
    pub status: SessionStatus,
    pub last_error: String,
    pub last_msgbox: String,
    pub fingerprint: String,
    pub is_secured: bool,
    pub is_direct: bool,
    pub stream_type: String,
    pub peer_username: String,
    pub peer_hostname: String,
    pub peer_platform: String,
    pub displays: Vec<(i32, i32, i32, i32)>, // x,y,w,h
    pub current_display: i32,
    pub clipboard_text: String,
    pub permissions: HashMap<String, bool>,
    pub os_login_status: OsLoginStatus,
    pub os_login_error: String,
}

#[derive(Clone, Default)]
pub struct HeadlessHandler {
    pub state: Arc<RwLock<SessionState>>,
    pub frames: Arc<FrameStore>,
}

impl HeadlessHandler {
    pub fn new(session_id: String, peer_id: String, frames: Arc<FrameStore>) -> Self {
        let state = SessionState {
            session_id,
            peer_id,
            status: SessionStatus::Connecting,
            ..Default::default()
        };
        Self {
            state: Arc::new(RwLock::new(state)),
            frames,
        }
    }

    pub fn snapshot(&self) -> SessionState {
        self.state.read().unwrap().clone()
    }

    fn set_status(&self, status: SessionStatus) {
        if let Ok(mut s) = self.state.write() {
            s.status = status;
        }
    }
}

impl InvokeUiSession for HeadlessHandler {
    fn set_cursor_data(&self, _cd: CursorData) {}
    fn set_cursor_id(&self, _id: String) {}
    fn set_cursor_position(&self, _cp: CursorPosition) {}

    fn set_display(&self, x: i32, y: i32, w: i32, h: i32, _cursor_embedded: bool, _scale: f64) {
        if let Ok(mut s) = self.state.write() {
            if s.displays.is_empty() {
                s.displays.push((x, y, w, h));
            } else {
                let idx = s.current_display.max(0) as usize;
                if idx < s.displays.len() {
                    s.displays[idx] = (x, y, w, h);
                } else {
                    s.displays.push((x, y, w, h));
                }
            }
        }
    }

    fn switch_display(&self, display: &SwitchDisplay) {
        if let Ok(mut s) = self.state.write() {
            s.current_display = display.display;
        }
    }

    fn set_peer_info(&self, peer_info: &PeerInfo) {
        if let Ok(mut s) = self.state.write() {
            s.peer_username = peer_info.username.clone();
            s.peer_hostname = peer_info.hostname.clone();
            s.peer_platform = peer_info.platform.clone();
            s.current_display = peer_info.current_display;
            s.displays = peer_info
                .displays
                .iter()
                .map(|d| (d.x, d.y, d.width, d.height))
                .collect();
            s.status = SessionStatus::Connected;
        }
    }

    fn set_displays(&self, displays: &Vec<DisplayInfo>) {
        if let Ok(mut s) = self.state.write() {
            s.displays = displays
                .iter()
                .map(|d| (d.x, d.y, d.width, d.height))
                .collect();
        }
    }

    fn set_platform_additions(&self, _data: &str) {}

    fn on_connected(&self, conn_type: ConnType) {
        log::info!("headless session connected: {:?}", conn_type);
        self.set_status(SessionStatus::Connected);
        match conn_type {
            ConnType::DEFAULT_CONN => {
                crate::keyboard::client::start_grab_loop();
            }
            _ => {}
        }
    }

    fn update_privacy_mode(&self) {}

    fn set_permission(&self, name: &str, value: bool) {
        if let Ok(mut s) = self.state.write() {
            s.permissions.insert(name.to_string(), value);
        }
    }

    fn close_success(&self) {
        self.set_status(SessionStatus::Disconnected);
    }

    fn update_quality_status(&self, _qs: QualityStatus) {}

    fn set_connection_type(&self, is_secured: bool, direct: bool, stream_type: &str) {
        if let Ok(mut s) = self.state.write() {
            s.is_secured = is_secured;
            s.is_direct = direct;
            s.stream_type = stream_type.to_string();
        }
    }

    fn set_fingerprint(&self, fingerprint: String) {
        if let Ok(mut s) = self.state.write() {
            s.fingerprint = fingerprint;
        }
    }

    fn job_error(&self, _id: i32, _err: String, _file_num: i32) {}
    fn job_done(&self, _id: i32, _file_num: i32) {}
    fn clear_all_jobs(&self) {}
    fn new_message(&self, _msg: String) {}
    fn update_transfer_list(&self) {}
    fn load_last_job(&self, _cnt: i32, _job_json: &str, _auto_start: bool) {}
    fn update_folder_files(
        &self,
        _id: i32,
        _entries: &Vec<FileEntry>,
        _path: String,
        _is_local: bool,
        _only_count: bool,
    ) {
    }
    fn confirm_delete_files(&self, _id: i32, _i: i32, _name: String) {}
    fn override_file_confirm(
        &self,
        _id: i32,
        _file_num: i32,
        _to: String,
        _is_upload: bool,
        _is_identical: bool,
    ) {
    }
    fn update_block_input_state(&self, _on: bool) {}
    fn job_progress(&self, _id: i32, _file_num: i32, _speed: f64, _finished_size: f64) {}
    fn adapt_size(&self) {}

    fn on_rgba(&self, display: usize, rgba: &mut ImageRgb) {
        let session_id = self
            .state
            .read()
            .map(|s| s.session_id.clone())
            .unwrap_or_default();
        if !session_id.is_empty() {
            self.frames.update(&session_id, display, rgba);
        }
    }

    fn msgbox(&self, msgtype: &str, title: &str, text: &str, link: &str, _retry: bool) {
        log::info!("headless msgbox: {msgtype} | {title} | {text} | {link}");
        if let Ok(mut s) = self.state.write() {
            s.last_msgbox = format!("{msgtype}|{title}|{text}|{link}");
            match msgtype {
                "input-password" | "re-input-password" => {
                    s.status = SessionStatus::WaitingPassword;
                }
                "input-2fa" => {
                    s.status = SessionStatus::Waiting2fa;
                }
                "error" => {
                    s.status = SessionStatus::Error;
                    s.last_error = if text.is_empty() {
                        title.to_string()
                    } else {
                        text.to_string()
                    };
                }
                "success" => {
                    s.status = SessionStatus::Connected;
                }
                _ => {}
            }
        }
    }

    fn cancel_msgbox(&self, _tag: &str) {}
    fn switch_back(&self, _id: &str) {}
    fn portable_service_running(&self, _running: bool) {}
    fn on_voice_call_started(&self) {}
    fn on_voice_call_closed(&self, _reason: &str) {}
    fn on_voice_call_waiting(&self) {}
    fn on_voice_call_incoming(&self) {}
    fn get_rgba(&self, _display: usize) -> *const u8 {
        std::ptr::null()
    }
    fn next_rgba(&self, _display: usize) {}
    fn set_multiple_windows_session(&self, _sessions: Vec<WindowsSession>) {}
    fn set_current_display(&self, disp_idx: i32) {
        if let Ok(mut s) = self.state.write() {
            s.current_display = disp_idx;
        }
    }
    fn update_record_status(&self, _start: bool) {}
    fn printer_request(&self, _id: i32, _path: String) {}
    fn handle_screenshot_resp(&self, _sid: String, _msg: String) {}
    fn handle_terminal_response(&self, _response: TerminalResponse) {}

    fn on_clipboard(&self, text: String) {
        if let Ok(mut s) = self.state.write() {
            s.clipboard_text = text;
        }
    }
}
