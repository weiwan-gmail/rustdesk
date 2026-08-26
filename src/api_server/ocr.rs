//! External OCR helper for OS-login UI detection.
//!
//! The OCR binary is fully decoupled: RustDesk writes a JPEG, spawns the
//! configured command (substituting `{image}`), and parses stdout.

use hbb_common::log;
use serde::Deserialize;
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

#[derive(Clone, Debug, Default, Deserialize)]
pub struct OcrLine {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub x: Option<i32>,
    #[serde(default)]
    pub y: Option<i32>,
    #[serde(default)]
    pub w: Option<i32>,
    #[serde(default)]
    pub h: Option<i32>,
}

#[derive(Clone, Debug, Default)]
pub struct OcrResult {
    pub text: String,
    pub lines: Vec<OcrLine>,
}

#[derive(Deserialize)]
struct OcrJson {
    #[serde(default)]
    text: String,
    #[serde(default)]
    lines: Vec<OcrLine>,
}

/// Replace `{image}` tokens in the command template.
pub fn subst_image(cmd: &[String], image: &Path) -> Vec<String> {
    let path = image.to_string_lossy();
    cmd.iter()
        .map(|s| s.replace("{image}", path.as_ref()))
        .collect()
}

pub fn run_ocr(cmd: &[String], image: &Path, timeout_ms: u64) -> Result<OcrResult, String> {
    if cmd.is_empty() {
        return Err("ocr_cmd is empty".to_string());
    }
    let argv = subst_image(cmd, image);
    let program = argv[0].clone();
    let args = argv[1..].to_vec();
    log::info!("ocr spawn {} {:?}", program, args);

    let timeout = Duration::from_millis(timeout_ms.max(200));
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let out = Command::new(&program)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();
        let _ = tx.send(out);
    });
    let output = rx
        .recv_timeout(timeout)
        .map_err(|_| format!("ocr timeout after {timeout_ms}ms"))?
        .map_err(|e| format!("ocr spawn failed: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ocr exited {}: {}",
            output.status.code().unwrap_or(-1),
            err.trim()
        ));
    }
    parse_ocr_stdout(&output.stdout)
}

pub fn parse_ocr_stdout(stdout: &[u8]) -> Result<OcrResult, String> {
    let raw = String::from_utf8_lossy(stdout);
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(OcrResult::default());
    }
    if let Ok(j) = serde_json::from_str::<OcrJson>(trimmed) {
        let mut text = j.text;
        if text.is_empty() {
            text = j
                .lines
                .iter()
                .map(|l| l.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
        }
        return Ok(OcrResult {
            text,
            lines: j.lines,
        });
    }
    // Plain text (e.g. tesseract stdout).
    Ok(OcrResult {
        text: trimmed.to_string(),
        lines: trimmed
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| OcrLine {
                text: l.to_string(),
                ..Default::default()
            })
            .collect(),
    })
}

pub fn write_temp_jpeg(bytes: &[u8]) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("rd-ocr");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!(
        "frame-{}-{}.jpg",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    let mut f = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    f.write_all(bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_and_plain() {
        let j = parse_ocr_stdout(
            r#"{"text":"Administrator\n密码","lines":[{"text":"密码","x":10,"y":20}]}"#.as_bytes(),
        )
        .unwrap();
        assert!(j.text.contains("Administrator"));
        assert_eq!(j.lines[0].x, Some(10));
        let p = parse_ocr_stdout(b"Press Ctrl+Alt+Delete to unlock.\n").unwrap();
        assert!(p.text.contains("Ctrl+Alt+Delete"));
    }

    #[test]
    fn subst_replaces_placeholder() {
        let v = subst_image(
            &["tesseract".into(), "{image}".into(), "stdout".into()],
            Path::new("/tmp/a.jpg"),
        );
        assert_eq!(v[1], "/tmp/a.jpg");
    }
}
