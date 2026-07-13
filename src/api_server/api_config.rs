//! Load `--api-config` JSON for headless controller options.

use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ApiConfigFile {
    #[serde(default)]
    pub bind: Option<String>,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub credentials_csv: Option<String>,
    #[serde(default)]
    pub relay: Option<bool>,
    #[serde(default)]
    pub os_login_delay_ms: Option<u64>,
    #[serde(default)]
    pub auto_os_login: Option<bool>,
    #[serde(default)]
    pub connect: Option<ApiConnectConfig>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ApiConnectConfig {
    #[serde(default)]
    pub peer_id: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub os_username: Option<String>,
    #[serde(default)]
    pub os_password: Option<String>,
    #[serde(default)]
    pub relay: Option<bool>,
    #[serde(default)]
    pub auto_os_login: Option<bool>,
    #[serde(default)]
    pub os_login_delay_ms: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct ApiLaunchOptions {
    pub bind: String,
    pub token: String,
    pub credentials_csv: Option<String>,
    pub relay: bool,
    pub os_login_delay_ms: u64,
    pub auto_os_login: bool,
    pub connect_peer: Option<String>,
    pub connect_password: Option<String>,
    pub os_username: Option<String>,
    pub os_password: Option<String>,
    /// True when launched via --api-connect or config.connect
    pub oneshot_connect: bool,
    // Track which CLI fields were explicitly provided.
    pub cli_bind: bool,
    pub cli_token: bool,
    pub cli_delay: bool,
    pub cli_auto_os: bool,
    pub cli_relay: bool,
}

impl Default for ApiLaunchOptions {
    fn default() -> Self {
        Self::defaults()
    }
}

impl ApiLaunchOptions {
    pub fn defaults() -> Self {
        Self {
            bind: "127.0.0.1:21120".to_string(),
            token: String::new(),
            credentials_csv: None,
            relay: false,
            os_login_delay_ms: 2500,
            auto_os_login: true,
            connect_peer: None,
            connect_password: None,
            os_username: None,
            os_password: None,
            oneshot_connect: false,
            cli_bind: false,
            cli_token: false,
            cli_delay: false,
            cli_auto_os: false,
            cli_relay: false,
        }
    }
}

pub fn load_file(path: &Path) -> Result<ApiConfigFile, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext == "toml" {
        hbb_common::toml::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))
    } else {
        // Default: JSON (.json or extensionless)
        serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))
    }
}

/// Merge file config under CLI overrides. Explicit CLI wins.
pub fn merge(file: Option<ApiConfigFile>, cli: ApiLaunchOptions) -> ApiLaunchOptions {
    let mut out = ApiLaunchOptions::defaults();
    if let Some(f) = file {
        if let Some(v) = f.bind.filter(|s| !s.is_empty()) {
            out.bind = v;
        }
        if let Some(v) = f.token {
            out.token = v;
        }
        if let Some(v) = f.credentials_csv.filter(|s| !s.is_empty()) {
            out.credentials_csv = Some(v);
        }
        if let Some(v) = f.relay {
            out.relay = v;
        }
        if let Some(v) = f.os_login_delay_ms {
            out.os_login_delay_ms = v;
        }
        if let Some(v) = f.auto_os_login {
            out.auto_os_login = v;
        }
        if let Some(c) = f.connect {
            out.oneshot_connect = true;
            if let Some(v) = c.peer_id.filter(|s| !s.is_empty()) {
                out.connect_peer = Some(v);
            }
            if let Some(v) = c.password {
                out.connect_password = Some(v);
            }
            if let Some(v) = c.os_username {
                out.os_username = Some(v);
            }
            if let Some(v) = c.os_password {
                out.os_password = Some(v);
            }
            if let Some(v) = c.relay {
                out.relay = v;
            }
            if let Some(v) = c.auto_os_login {
                out.auto_os_login = v;
            }
            if let Some(v) = c.os_login_delay_ms {
                out.os_login_delay_ms = v;
            }
        }
    }

    if cli.cli_bind {
        out.bind = cli.bind;
    }
    if cli.cli_token {
        out.token = cli.token;
    }
    if cli.credentials_csv.is_some() {
        out.credentials_csv = cli.credentials_csv;
    }
    if cli.cli_relay {
        out.relay = cli.relay;
    }
    if cli.cli_delay {
        out.os_login_delay_ms = cli.os_login_delay_ms;
    }
    if cli.cli_auto_os {
        out.auto_os_login = cli.auto_os_login;
    }
    if cli.oneshot_connect {
        out.oneshot_connect = true;
    }
    if let Some(v) = cli.connect_peer.filter(|s| !s.is_empty()) {
        out.connect_peer = Some(v);
        out.oneshot_connect = true;
    }
    if let Some(v) = cli.connect_password {
        out.connect_password = Some(v);
    }
    if let Some(v) = cli.os_username {
        out.os_username = Some(v);
    }
    if let Some(v) = cli.os_password {
        out.os_password = Some(v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_cli_over_file() {
        let dir = std::env::temp_dir().join(format!("rd-api-cfg-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("a.json");
        std::fs::write(
            &path,
            r#"{"bind":"127.0.0.1:1","token":"t1","connect":{"peer_id":"1","os_username":"u1","os_password":"p1"}}"#,
        )
        .unwrap();
        let file = load_file(&path).unwrap();
        let mut cli = ApiLaunchOptions::defaults();
        cli.token = "t2".into();
        cli.cli_token = true;
        cli.connect_peer = Some("2".into());
        cli.oneshot_connect = true;
        let m = merge(Some(file), cli);
        assert_eq!(m.token, "t2");
        assert_eq!(m.connect_peer.as_deref(), Some("2"));
        assert_eq!(m.os_username.as_deref(), Some("u1"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_toml_api_config() {
        let dir = std::env::temp_dir().join(format!("rd-api-toml-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("a.toml");
        std::fs::write(
            &path,
            r#"
bind = "127.0.0.1:9"
token = "tok"
os_login_delay_ms = 3000
[connect]
peer_id = "10.0.0.1"
os_username = "Admin"
"#,
        )
        .unwrap();
        let f = load_file(&path).unwrap();
        assert_eq!(f.bind.as_deref(), Some("127.0.0.1:9"));
        assert_eq!(f.connect.as_ref().unwrap().peer_id.as_deref(), Some("10.0.0.1"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
