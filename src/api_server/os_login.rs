//! Simulate Windows login-screen keyboard entry after a remote session connects.
//!
//! Optional guided mode: grab a JPEG, spawn an external OCR process, match
//! configurable regexes (cad / password / desktop / ...), then type.

use crate::common::input::{MOUSE_BUTTON_LEFT, MOUSE_TYPE_DOWN, MOUSE_TYPE_MOVE, MOUSE_TYPE_UP};
use crate::ui_session_interface::Session;
use hbb_common::log;
use serde::{Deserialize, Serialize};
use std::thread;
use std::time::Duration;

use super::frame_store::FrameStore;
use super::handler::{HeadlessHandler, OsLoginStatus, SessionStatus};
use super::ocr;

#[derive(Clone, Debug, Deserialize, Serialize)]
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
    /// Send Ctrl+Alt+Delete first (common on Windows lock: "Press Ctrl+Alt+Delete to unlock").
    #[serde(default = "default_true")]
    pub ctrl_alt_del: bool,
    /// Run the OCR-guided loop instead of a single key sequence.
    #[serde(default)]
    pub guided: bool,
}

impl Default for OsLoginParams {
    fn default() -> Self {
        Self {
            username: String::new(),
            password: String::new(),
            activate: true,
            delay_ms: 2500,
            username_first: true,
            ctrl_alt_del: true,
            guided: false,
        }
    }
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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct OsLoginRule {
    /// Phase id: `cad`, `wrong_password`, `password`, `switch_user`, `desktop`.
    #[serde(default)]
    pub id: String,
    /// Regex or literal strings; first matching rule wins. Case-insensitive.
    #[serde(default)]
    pub any: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct OsLoginGuide {
    pub ocr_cmd: Vec<String>,
    pub ocr_timeout_ms: u64,
    pub rules: Vec<OsLoginRule>,
    pub max_rounds: u32,
    pub round_delay_ms: u64,
}

impl Default for OsLoginGuide {
    fn default() -> Self {
        Self {
            ocr_cmd: Vec::new(),
            ocr_timeout_ms: 5000,
            rules: default_os_login_rules(),
            max_rounds: 12,
            round_delay_ms: 1500,
        }
    }
}

impl OsLoginGuide {
    pub fn has_ocr(&self) -> bool {
        self.ocr_cmd.iter().any(|s| !s.is_empty())
    }
}

/// Built-in multilingual hints. Config `os_login_rules` replaces these when non-empty.
pub fn default_os_login_rules() -> Vec<OsLoginRule> {
    vec![
        OsLoginRule {
            id: "wrong_password".into(),
            any: vec![
                "密码不正确".into(),
                "密码错误".into(),
                "incorrect password".into(),
                "wrong password".into(),
                "the password is incorrect".into(),
            ],
        },
        OsLoginRule {
            id: "cad".into(),
            any: vec![
                r"Ctrl\+Alt\+Del".into(),
                r"Ctrl-Alt-Del".into(),
                r"Ctrl\+Alt\+Delete".into(),
                "解锁".into(),
                "unlock".into(),
            ],
        },
        OsLoginRule {
            id: "switch_user".into(),
            any: vec![
                "切换用户".into(),
                "其他用户".into(),
                "Other user".into(),
                "Switch user".into(),
            ],
        },
        OsLoginRule {
            id: "desktop".into(),
            any: vec![
                "回收站".into(),
                "Recycle Bin".into(),
                "此电脑".into(),
                "This PC".into(),
            ],
        },
        OsLoginRule {
            id: "password".into(),
            any: vec!["密码".into(), "Password".into(), r"\bPIN\b".into()],
        },
    ]
}

fn compile_rule(pat: &str) -> Option<hbb_common::regex::Regex> {
    let p = pat.trim();
    if p.is_empty() {
        return None;
    }
    hbb_common::regex::RegexBuilder::new(p)
        .case_insensitive(true)
        .build()
        .ok()
        .or_else(|| {
            hbb_common::regex::RegexBuilder::new(&hbb_common::regex::escape(p))
                .case_insensitive(true)
                .build()
                .ok()
        })
}

/// First matching rule id, or None.
pub fn match_phase(text: &str, rules: &[OsLoginRule]) -> Option<String> {
    if text.trim().is_empty() {
        return None;
    }
    for rule in rules {
        if rule.id.is_empty() {
            continue;
        }
        for pat in &rule.any {
            if let Some(re) = compile_rule(pat) {
                if re.is_match(text) {
                    return Some(rule.id.clone());
                }
            }
        }
    }
    None
}

fn username_visible(ocr_text: &str, username: &str) -> bool {
    let u = username.trim();
    if u.is_empty() {
        return false;
    }
    ocr_text.to_lowercase().contains(&u.to_lowercase())
}

fn click_xy(session: &Session<HeadlessHandler>, x: i32, y: i32) {
    let mask_move = MOUSE_TYPE_MOVE;
    session.send_mouse(mask_move, x, y, false, false, false, false);
    let down = (MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_DOWN;
    let up = (MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_UP;
    session.send_mouse(down, x, y, false, false, false, false);
    session.send_mouse(up, x, y, false, false, false, false);
}

fn click_matching_line(session: &Session<HeadlessHandler>, result: &ocr::OcrResult, rule_id: &str, rules: &[OsLoginRule]) {
    let Some(rule) = rules.iter().find(|r| r.id == rule_id) else {
        return;
    };
    let regs: Vec<_> = rule.any.iter().filter_map(|p| compile_rule(p)).collect();
    for line in &result.lines {
        if !regs.iter().any(|re| re.is_match(&line.text)) {
            continue;
        }
        if let (Some(x), Some(y), Some(w), Some(h)) = (line.x, line.y, line.w, line.h) {
            click_xy(session, x + w / 2, y + h / 2);
            return;
        }
    }
}

fn type_password_only(session: &Session<HeadlessHandler>, password: &str) {
    if !password.is_empty() {
        session.input_string(password);
        thread::sleep(Duration::from_millis(150));
    }
    session.input_key("VK_RETURN", false, true, false, false, false, false);
}

fn type_username_then_password(session: &Session<HeadlessHandler>, username: &str, password: &str) {
    if !username.is_empty() {
        session.input_string(username);
        thread::sleep(Duration::from_millis(200));
        session.input_key("VK_TAB", false, true, false, false, false, false);
        thread::sleep(Duration::from_millis(250));
    }
    type_password_only(session, password);
}

fn set_phase(session: &Session<HeadlessHandler>, phase: &str) {
    if let Ok(mut s) = session.ui_handler.state.write() {
        s.os_login_phase = phase.to_string();
    }
}

/// Wake login UI (optional), type username, Tab, password, Enter.
pub fn perform(session: &Session<HeadlessHandler>, params: &OsLoginParams) -> Result<(), String> {
    if params.is_empty() {
        return Err("os username/password empty".to_string());
    }
    set_status(session, OsLoginStatus::Running, None);
    log::info!(
        "os-login begin cad={} activate={} user_len={} pass_len={}",
        params.ctrl_alt_del,
        params.activate,
        params.username.len(),
        params.password.len()
    );

    if params.ctrl_alt_del {
        session.ctrl_alt_del();
        thread::sleep(Duration::from_millis(1800));
    }

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

fn fallback_cad_password(session: &Session<HeadlessHandler>, params: &OsLoginParams) -> Result<(), String> {
    let mut p = params.clone();
    p.username_first = false;
    p.ctrl_alt_del = true;
    p.activate = true;
    perform(session, &p)
}

/// OCR-guided loop. Falls back to CAD + password-only when OCR is missing or unmatched.
pub fn perform_guided(
    session: &Session<HeadlessHandler>,
    params: &OsLoginParams,
    frames: &FrameStore,
    guide: &OsLoginGuide,
) -> Result<(), String> {
    if params.is_empty() {
        return Err("os username/password empty".to_string());
    }
    set_status(session, OsLoginStatus::Running, None);
    let session_id = session.ui_handler.snapshot().session_id;
    let rules = if guide.rules.is_empty() {
        default_os_login_rules()
    } else {
        guide.rules.clone()
    };
    let delay = Duration::from_millis(guide.round_delay_ms.max(300));
    let mut typed_password = false;
    let mut sent_cad = false;

    for round in 0..guide.max_rounds.max(1) {
        let snap = session.ui_handler.snapshot();
        if matches!(snap.status, SessionStatus::Error) {
            return Err(snap.last_error);
        }
        let frame = frames.get(&session_id, 0);
        let mut phase = None;
        let mut ocr_res = ocr::OcrResult::default();
        if let Some(ref frame) = frame {
            if guide.has_ocr() {
                match frame.encode_jpeg(70) {
                    Ok(jpeg) => match ocr::write_temp_jpeg(&jpeg) {
                        Ok(path) => {
                            match ocr::run_ocr(&guide.ocr_cmd, &path, guide.ocr_timeout_ms) {
                                Ok(r) => {
                                    ocr_res = r;
                                    phase = match_phase(&ocr_res.text, &rules);
                                    log::info!(
                                        "os-login ocr round={round} phase={:?} text_len={}",
                                        phase,
                                        ocr_res.text.len()
                                    );
                                    let _ = std::fs::remove_file(&path);
                                }
                                Err(e) => {
                                    log::warn!("os-login ocr failed: {e}");
                                    let _ = std::fs::remove_file(&path);
                                }
                            }
                        }
                        Err(e) => log::warn!("os-login jpeg temp: {e}"),
                    },
                    Err(e) => log::warn!("os-login jpeg: {e}"),
                }
            }
        }

        if let Some(ref p) = phase {
            set_phase(session, p);
        }

        match phase.as_deref() {
            Some("desktop") => {
                set_status(session, OsLoginStatus::Done, None);
                log::info!("os-login desktop detected");
                return Ok(());
            }
            Some("wrong_password") => {
                session.input_key("VK_RETURN", false, true, false, false, false, false);
                typed_password = false;
            }
            Some("cad") => {
                session.ctrl_alt_del();
                sent_cad = true;
            }
            Some("switch_user") => {
                click_matching_line(session, &ocr_res, "switch_user", &rules);
                thread::sleep(Duration::from_millis(400));
                type_username_then_password(session, &params.username, &params.password);
                typed_password = true;
            }
            Some("password") => {
                let visible = username_visible(&ocr_res.text, &params.username);
                if visible || params.username.is_empty() {
                    type_password_only(session, &params.password);
                } else {
                    type_username_then_password(session, &params.username, &params.password);
                }
                typed_password = true;
            }
            _ => {
                if !guide.has_ocr() || (round == 0 && frame.is_none()) {
                    // No OCR / no frame yet: wait, then fallback once.
                } else if round + 1 == guide.max_rounds && !typed_password {
                    log::warn!("os-login OCR unmatched, fallback CAD+password");
                    fallback_cad_password(session, params)?;
                    return Ok(());
                }
            }
        }

        if round == 0 && phase.is_none() && frame.is_some() && !guide.has_ocr() {
            fallback_cad_password(session, params)?;
            return Ok(());
        }

        thread::sleep(delay);
        if sent_cad && typed_password && round >= 2 {
            // Give desktop OCR a chance; if still no desktop, treat as done after typing.
            if phase.is_none() {
                set_status(session, OsLoginStatus::Done, None);
                return Ok(());
            }
        }
    }

    if typed_password || sent_cad {
        set_status(session, OsLoginStatus::Done, None);
        Ok(())
    } else {
        log::warn!("os-login guided exhausted, fallback");
        fallback_cad_password(session, params)
    }
}

/// Poll until session is connected, then run OS login once (guided if OCR/rules set).
pub fn spawn_after_connect(
    session: Session<HeadlessHandler>,
    params: OsLoginParams,
    frames: std::sync::Arc<FrameStore>,
    guide: OsLoginGuide,
) {
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
                    if matches!(
                        snap.os_login_status,
                        OsLoginStatus::Done | OsLoginStatus::Running | OsLoginStatus::Failed
                    ) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(delay));
                    let use_guided = params.guided || guide.has_ocr() || !guide.rules.is_empty();
                    let res = if use_guided {
                        perform_guided(&session, &params, &frames, &guide)
                    } else {
                        let mut p = params.clone();
                        p.username_first = false;
                        perform(&session, &p)
                    };
                    if let Err(e) = res {
                        set_status(&session, OsLoginStatus::Failed, Some(e));
                    }
                    return;
                }
                SessionStatus::WaitingPassword | SessionStatus::Waiting2fa => {}
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_prefer_wrong_password_over_password() {
        let rules = default_os_login_rules();
        assert_eq!(
            match_phase("密码不正确。请再试一次。", &rules).as_deref(),
            Some("wrong_password")
        );
        assert_eq!(
            match_phase("Administrator\n密码", &rules).as_deref(),
            Some("password")
        );
        assert_eq!(
            match_phase("按 Ctrl+Alt+Delete 解锁。", &rules).as_deref(),
            Some("cad")
        );
        assert_eq!(
            match_phase("Press Ctrl+Alt+Delete to unlock.", &rules).as_deref(),
            Some("cad")
        );
        assert_eq!(
            match_phase("Recycle Bin\nThis PC", &rules).as_deref(),
            Some("desktop")
        );
    }

    #[test]
    fn custom_rules_override_order() {
        let rules = vec![OsLoginRule {
            id: "desktop".into(),
            any: vec!["桌面标记".into()],
        }];
        assert_eq!(
            match_phase("xxx 桌面标记 yyy", &rules).as_deref(),
            Some("desktop")
        );
        assert!(match_phase("Ctrl+Alt+Del", &rules).is_none());
    }
}
