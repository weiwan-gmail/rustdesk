use crate::common::input::{
    MOUSE_BUTTON_BACK, MOUSE_BUTTON_FORWARD, MOUSE_BUTTON_LEFT, MOUSE_BUTTON_RIGHT,
    MOUSE_BUTTON_WHEEL, MOUSE_TYPE_DOWN, MOUSE_TYPE_MOVE, MOUSE_TYPE_MOVE_RELATIVE, MOUSE_TYPE_TRACKPAD,
    MOUSE_TYPE_UP, MOUSE_TYPE_WHEEL,
};
use crate::ui_session_interface::Session;
use serde::Deserialize;
use std::thread;
use std::time::Duration;

use super::handler::HeadlessHandler;

#[derive(Debug, Deserialize)]
pub struct InputAction {
    pub action: String,
    #[serde(default)]
    pub x: Option<i32>,
    #[serde(default)]
    pub y: Option<i32>,
    #[serde(default)]
    pub button: Option<String>,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub down: Option<bool>,
    #[serde(default)]
    pub press: Option<bool>,
    #[serde(default)]
    pub alt: Option<bool>,
    #[serde(default)]
    pub ctrl: Option<bool>,
    #[serde(default)]
    pub shift: Option<bool>,
    #[serde(default)]
    pub command: Option<bool>,
    #[serde(default)]
    pub dx: Option<i32>,
    #[serde(default)]
    pub dy: Option<i32>,
}

fn button_mask(button: Option<&str>) -> i32 {
    match button.unwrap_or("left") {
        "left" => MOUSE_BUTTON_LEFT,
        "right" => MOUSE_BUTTON_RIGHT,
        "middle" | "wheel" => MOUSE_BUTTON_WHEEL,
        "back" => MOUSE_BUTTON_BACK,
        "forward" => MOUSE_BUTTON_FORWARD,
        _ => MOUSE_BUTTON_LEFT,
    }
}

fn send_mouse(
    session: &Session<HeadlessHandler>,
    event_type: i32,
    button: i32,
    x: i32,
    y: i32,
    alt: bool,
    ctrl: bool,
    shift: bool,
    command: bool,
) {
    let mask = (button << 3) | event_type;
    session.send_mouse(mask, x, y, alt, ctrl, shift, command);
}

pub fn apply_action(
    session: &Session<HeadlessHandler>,
    action: InputAction,
) -> Result<(), String> {
    let alt = action.alt.unwrap_or(false);
    let ctrl = action.ctrl.unwrap_or(false);
    let shift = action.shift.unwrap_or(false);
    let command = action.command.unwrap_or(false);
    let x = action.x.unwrap_or(0);
    let y = action.y.unwrap_or(0);
    let btn = button_mask(action.button.as_deref());

    match action.action.as_str() {
        "mouse_move" => {
            send_mouse(
                session,
                MOUSE_TYPE_MOVE,
                0,
                x,
                y,
                alt,
                ctrl,
                shift,
                command,
            );
        }
        "mouse_move_relative" => {
            send_mouse(
                session,
                MOUSE_TYPE_MOVE_RELATIVE,
                0,
                action.dx.unwrap_or(x),
                action.dy.unwrap_or(y),
                alt,
                ctrl,
                shift,
                command,
            );
        }
        "mouse_down" => {
            send_mouse(session, MOUSE_TYPE_DOWN, btn, x, y, alt, ctrl, shift, command);
        }
        "mouse_up" => {
            send_mouse(session, MOUSE_TYPE_UP, btn, x, y, alt, ctrl, shift, command);
        }
        "mouse_click" | "click" => {
            let click_type = action.r#type.as_deref().unwrap_or("click");
            let times = if click_type == "double_click" || click_type == "double" {
                2
            } else {
                1
            };
            // Move first so click lands on intended pixel.
            send_mouse(session, MOUSE_TYPE_MOVE, 0, x, y, false, false, false, false);
            for i in 0..times {
                send_mouse(session, MOUSE_TYPE_DOWN, btn, x, y, alt, ctrl, shift, command);
                send_mouse(session, MOUSE_TYPE_UP, btn, x, y, alt, ctrl, shift, command);
                if i + 1 < times {
                    thread::sleep(Duration::from_millis(40));
                }
            }
        }
        "mouse_wheel" | "wheel" => {
            send_mouse(
                session,
                MOUSE_TYPE_WHEEL,
                0,
                action.dx.unwrap_or(x),
                action.dy.unwrap_or(y),
                alt,
                ctrl,
                shift,
                command,
            );
        }
        "mouse_trackpad" | "trackpad" => {
            send_mouse(
                session,
                MOUSE_TYPE_TRACKPAD,
                0,
                action.dx.unwrap_or(x),
                action.dy.unwrap_or(y),
                alt,
                ctrl,
                shift,
                command,
            );
        }
        "keyboard_type" | "type" => {
            let text = action
                .text
                .ok_or_else(|| "text is required for keyboard_type".to_string())?;
            session.input_string(&text);
        }
        "keyboard_key" | "key" => {
            let key = action
                .key
                .or(action.text)
                .ok_or_else(|| "key is required for keyboard_key".to_string())?;
            let press = action.press.unwrap_or(true);
            let down = action.down.unwrap_or(false);
            session.input_key(&key, down, press, alt, ctrl, shift, command);
        }
        other => return Err(format!("unknown action: {other}")),
    }
    Ok(())
}
