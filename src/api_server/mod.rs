//! Headless controller API daemon.
//!
//! Exposes HTTP REST + WebSocket endpoints so an external agent can connect to
//! remotes, fetch decoded screen frames, inject input, and read/write clipboard.
//! Also supports Windows OS login-screen auto typing and CSV credential lookup.

mod api_config;
mod auth;
mod clipboard;
mod credentials;
mod frame_store;
mod handler;
mod input;
mod ocr;
mod os_login;
mod routes;
mod session_mgr;
mod ws;

pub use api_config::{ApiConnectConfig, ApiLaunchOptions};
pub use credentials::{CredentialPublicInfo, CredentialStore, SharedCredentialStore};
pub use frame_store::{FrameStore, LatestFrame};
pub use handler::{HeadlessHandler, OsLoginStatus, SessionState, SessionStatus};
pub use os_login::{OsLoginGuide, OsLoginParams, OsLoginRule};
pub use session_mgr::{ConnectRequest, HeadlessSession, SessionManager};

use hbb_common::log;
use std::sync::Arc;

/// Blocking entry used by `core_main` for `--api-server` / `--api-connect`.
pub fn run(opts: ApiLaunchOptions) {
    log::info!("starting headless api-server on {}", opts.bind);
    let rt = match hbb_common::tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            log::error!("failed to create tokio runtime for api-server: {e}");
            eprintln!("failed to create tokio runtime for api-server: {e}");
            return;
        }
    };
    rt.block_on(async move {
        if let Err(e) = run_async(opts).await {
            log::error!("api-server exited with error: {e}");
            eprintln!("api-server exited with error: {e}");
        }
    });
}

/// Parse CLI-ish args already collected for api modes into [`ApiLaunchOptions`].
pub fn launch_options_from_args(args: &[String]) -> ApiLaunchOptions {
    let mut cli = ApiLaunchOptions::defaults();
    let mut api_config_path: Option<String> = None;
    let mut i = 0;
    // args[0] is --api-server / --api-connect / --api-config / --headless-connect
    if let Some(first) = args.first() {
        match first.as_str() {
            "--api-connect" | "--headless-connect" => {
                cli.oneshot_connect = true;
                if args.len() > 1 && !args[1].starts_with("--") {
                    cli.connect_peer = Some(args[1].clone());
                    i = 2;
                } else {
                    i = 1;
                }
            }
            "--api-config" => {
                // bare --api-config path as first token
                if args.len() > 1 {
                    api_config_path = Some(args[1].clone());
                }
                i = 2;
            }
            _ => {
                i = 1;
            }
        }
    }
    while i < args.len() {
        match args[i].as_str() {
            "--bind" if i + 1 < args.len() => {
                cli.bind = args[i + 1].clone();
                cli.cli_bind = true;
                i += 2;
            }
            "--token" if i + 1 < args.len() => {
                cli.token = args[i + 1].clone();
                cli.cli_token = true;
                i += 2;
            }
            "--api-config" if i + 1 < args.len() => {
                api_config_path = Some(args[i + 1].clone());
                i += 2;
            }
            "--credentials-csv" if i + 1 < args.len() => {
                cli.credentials_csv = Some(args[i + 1].clone());
                i += 2;
            }
            "--password" if i + 1 < args.len() => {
                cli.connect_password = Some(args[i + 1].clone());
                i += 2;
            }
            "--os-username" if i + 1 < args.len() => {
                cli.os_username = Some(args[i + 1].clone());
                i += 2;
            }
            "--os-password" if i + 1 < args.len() => {
                cli.os_password = Some(args[i + 1].clone());
                i += 2;
            }
            "--os-login-delay-ms" if i + 1 < args.len() => {
                if let Ok(v) = args[i + 1].parse::<u64>() {
                    cli.os_login_delay_ms = v;
                    cli.cli_delay = true;
                }
                i += 2;
            }
            "--relay" => {
                cli.relay = true;
                cli.cli_relay = true;
                i += 1;
            }
            "--no-auto-os-login" => {
                cli.auto_os_login = false;
                cli.cli_auto_os = true;
                i += 1;
            }
            "--show-gui" => {
                cli.show_gui = true;
                cli.cli_show_gui = true;
                i += 1;
            }
            "--no-gui" => {
                cli.show_gui = false;
                cli.cli_show_gui = true;
                i += 1;
            }
            "--rendezvous-server" if i + 1 < args.len() => {
                cli.rendezvous_server = Some(args[i + 1].clone());
                i += 2;
            }
            "--key" if i + 1 < args.len() => {
                cli.key = Some(args[i + 1].clone());
                i += 2;
            }
            "--relay-server" if i + 1 < args.len() => {
                cli.relay_server = Some(args[i + 1].clone());
                i += 2;
            }
            "--ocr-cmd" if i + 1 < args.len() => {
                cli.os_login_guide.ocr_cmd = args[i + 1]
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();
                i += 2;
            }
            "--ocr-timeout-ms" if i + 1 < args.len() => {
                if let Ok(v) = args[i + 1].parse::<u64>() {
                    cli.os_login_guide.ocr_timeout_ms = v;
                    cli.cli_ocr_timeout = true;
                }
                i += 2;
            }
            "--api-connect" | "--headless-connect" => {
                cli.oneshot_connect = true;
                if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    cli.connect_peer = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }

    let file = api_config_path.as_ref().and_then(|p| {
        match api_config::load_file(std::path::Path::new(p)) {
            Ok(c) => Some(c),
            Err(e) => {
                eprintln!("--api-config error: {e}");
                None
            }
        }
    });

    api_config::merge(file, cli)
}

#[cfg(test)]
mod launch_tests {
    use super::*;

    #[test]
    fn parse_api_connect_cli() {
        let args = vec![
            "--api-connect".into(),
            "192.168.1.10".into(),
            "--password".into(),
            "rd".into(),
            "--os-username".into(),
            "Admin".into(),
            "--os-password".into(),
            "secret".into(),
            "--bind".into(),
            "127.0.0.1:21199".into(),
            "--token".into(),
            "tok".into(),
        ];
        let o = launch_options_from_args(&args);
        assert!(o.oneshot_connect);
        assert_eq!(o.connect_peer.as_deref(), Some("192.168.1.10"));
        assert_eq!(o.connect_password.as_deref(), Some("rd"));
        assert_eq!(o.os_username.as_deref(), Some("Admin"));
        assert_eq!(o.os_password.as_deref(), Some("secret"));
        assert_eq!(o.bind, "127.0.0.1:21199");
        assert_eq!(o.token, "tok");
        assert!(!o.show_gui);
    }

    #[test]
    fn parse_show_gui_flags() {
        let with_gui = launch_options_from_args(&[
            "--api-server".into(),
            "--show-gui".into(),
            "--bind".into(),
            "127.0.0.1:1".into(),
        ]);
        assert!(with_gui.show_gui);
        let no_gui = launch_options_from_args(&[
            "--api-server".into(),
            "--show-gui".into(),
            "--no-gui".into(),
        ]);
        assert!(!no_gui.show_gui);
    }

    #[test]
    fn parse_ocr_cmd() {
        let o = launch_options_from_args(&[
            "--api-server".into(),
            "--ocr-cmd".into(),
            "tesseract {image} stdout".into(),
        ]);
        assert_eq!(
            o.os_login_guide.ocr_cmd,
            vec!["tesseract", "{image}", "stdout"]
        );
    }
}

async fn run_async(opts: ApiLaunchOptions) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Apply custom server options before any outbound connect.
    if let Some(ref host) = opts.rendezvous_server {
        log::info!("api-server using custom rendezvous: {host}");
        hbb_common::config::Config::set_option(
            "custom-rendezvous-server".to_string(),
            host.clone(),
        );
    }
    if let Some(ref key) = opts.key {
        hbb_common::config::Config::set_option("key".to_string(), key.clone());
    }
    if let Some(ref relay) = opts.relay_server {
        log::info!("api-server using relay-server: {relay}");
        hbb_common::config::Config::set_option("relay-server".to_string(), relay.clone());
    }
    if opts.relay {
        // Prefer relay path from this cloud/NAT environment.
        hbb_common::config::Config::set_option(
            "force-always-relay".to_string(),
            "Y".to_string(),
        );
    }

    let creds = credentials::open_csv(opts.credentials_csv.clone());
    let frames = Arc::new(FrameStore::new());
    let sessions = Arc::new(SessionManager::with_options(
        creds.clone(),
        frames.clone(),
        opts.os_login_guide.clone(),
    ));
    let state = Arc::new(AppState {
        sessions: sessions.clone(),
        frames: frames.clone(),
        token: opts.token.clone(),
        credentials: creds,
    });

    if opts.oneshot_connect {
        if let Some(peer) = opts.connect_peer.clone() {
            let req = ConnectRequest {
                peer_id: peer,
                password: opts.connect_password.clone(),
                relay: Some(opts.relay),
                two_factor: None,
                os_username: opts.os_username.clone(),
                os_password: opts.os_password.clone(),
                auto_os_login: Some(opts.auto_os_login),
                os_login_delay_ms: Some(opts.os_login_delay_ms),
            };
            match sessions.connect(req, frames.clone()) {
                Ok(info) => {
                    log::info!(
                        "oneshot connect started id={} peer={} status={}",
                        info.id,
                        info.peer_id,
                        info.status
                    );
                    println!(
                        "Connected session {} -> {} ({})",
                        info.id, info.peer_id, info.status
                    );
                }
                Err(e) => {
                    log::error!("oneshot connect failed: {e}");
                    eprintln!("oneshot connect failed: {e}");
                }
            }
        } else {
            eprintln!("--api-connect requires peer id/ip or connect.peer_id in --api-config");
        }
    }

    let app = routes::router(state);
    let listener = hbb_common::tokio::net::TcpListener::bind(&opts.bind).await?;
    log::info!("headless api-server listening on http://{}", opts.bind);
    println!("RustDesk headless API listening on http://{}", opts.bind);
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Clone)]
pub struct AppState {
    pub sessions: Arc<SessionManager>,
    pub frames: Arc<FrameStore>,
    pub token: String,
    pub credentials: SharedCredentialStore,
}
