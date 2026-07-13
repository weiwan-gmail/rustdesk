//! Simulate Windows login-screen keyboard entry after a remote session connects.

use crate::ui_session_interface::Session;
use hbb_common::log;
use serde::{Deserialize, Serialize};
use std::thread;
use std::time::Duration;

use super::handler::{HeadlessHandler, OsLoginStatus, SessionStatus};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct OsLoginParams {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default = "default_true")]
    pub activate: bool,
    #[serde(default = "default_delay")]
    pub delay_ms: u64,
    #[serde(default = "default_true")]
    pub username_first: bool,
}

fn default_true() -> bool {
    true
}
fn default_delay() -> u64 {
    2500
}

impl OsLoginParams {
    pub fn is_empty(&self) -> bool {
        self.username.is_empty() && self.password.is_empty()
    }
}

/// Wake login UI (optional), type username, Tab, password, Enter.
pub fn perform(session: &Session<HeadlessHandler>, params: &OsLoginParams) -> Result<(), String> {
    if params.is_empty() {
        return Err("os username/password empty".to_string());
    }
    set_status(session, OsLoginStatus::Running, None);
    log::info!(
        "os-login begin activate={} user_len={} pass_len={}",
        params.activate,
        params.username.len(),
        params.password.len()
    );

    if params.activate {
        session.input_os_password(String::new(), true);
        thread::sleep(Duration::from_millis(1200));
    }

    if params.username_first && !params.username.is_empty() {
        session.input_string(&params.username);
        thread::sleep(Duration::from_millis(200));
        session.input_key("VK_TAB", false, true, false, false, false, false);
        thread::sleep(Duration::from_millis(250));
    }

    if !params.password.is_empty() {
        session.input_string(&params.password);
        thread::sleep(Duration::from_millis(150));
    }
    session.input_key("VK_RETURN", false, true, false, false, false, false);
    set_status(session, OsLoginStatus::Done, None);
    log::info!("os-login sequence sent");
    Ok(())
}

fn set_status(session: &Session<HeadlessHandler>, status: OsLoginStatus, err: Option<String>) {
    if let Ok(mut s) = session.ui_handler.state.write() {
        if let Some(e) = err {
            s.os_login_error = e;
        } else if status != OsLoginStatus::Failed {
            s.os_login_error.clear();
        }
        s.os_login_status = status;
    }
}

/// Poll until session is connected, then run OS login once.
pub fn spawn_after_connect(session: Session<HeadlessHandler>, params: OsLoginParams) {
    if params.is_empty() {
        return;
    }
    set_status(&session, OsLoginStatus::Pending, None);
    thread::spawn(move || {
        let delay = params.delay_ms.max(100);
        for _ in 0..600 {
            let snap = session.ui_handler.snapshot();
            match snap.status {
                SessionStatus::Connected => {
                    // Skip if already finished (manual call won the race).
                    if matches!(
                        snap.os_login_status,
                        OsLoginStatus::Done | OsLoginStatus::Running | OsLoginStatus::Failed
                    ) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(delay));
                    if let Err(e) = perform(&session, &params) {
                        set_status(&session, OsLoginStatus::Failed, Some(e));
                    }
                    return;
                }
                SessionStatus::Error | SessionStatus::Disconnected => {
                    set_status(
                        &session,
                        OsLoginStatus::Failed,
                        Some("session not connected".into()),
                    );
                    return;
                }
                _ => {}
            }
            thread::sleep(Duration::from_millis(100));
        }
        set_status(
            &session,
            OsLoginStatus::Failed,
            Some("timeout waiting for connection".into()),
        );
    });
}
