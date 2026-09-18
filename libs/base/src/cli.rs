use crate::video_codec::VIDEO_CODEC_VALUES;

pub fn is_cli_help_arg(arg: &str) -> bool {
    matches!(arg, "--help" | "-h" | "-help")
}

pub fn main_help_text(version: &str) -> String {
    let codec = VIDEO_CODEC_VALUES;
    format!(
        "\
RustDesk {version}

Usage:
  rustdesk [options]
  rustdesk --server [options]
  rustdesk --connect <id> [options]

Roles:
  --server              Run the local controlled service (this process).
  --service             Run as the OS service wrapper (starts --server).
  --tray                Tray icon only.

Connect:
  --connect <id>        Open a remote desktop session.
  --play, --file-transfer, --view-camera, --port-forward, --terminal, --rdp
  --password <password> With --connect: session password.
                        Alone (installed + admin): set permanent password (persists).
  --relay               Force relay for this connection.
  --rendezvous-server <host[:port]>, --key <key>
                        Session-only hbbs for --connect. Not persisted.

  --video-codec <{codec}>
                        Process-lifetime encode/decode preference. Not persisted.

Controlled direct IP access (this process, not persisted):
  --direct-access-port <n>
                        Listen on TCP port n for direct IP access (what peers /
                        web-direct connect to). Implies enable for this run.
                        Default without the flag: config direct-access-port, or
                        21118 (RENDEZVOUS_PORT+2). This is not web-direct's
                        --direct-port allowlist, and not --rendezvous-server.
  --direct-server [Y|N]
                        Enable or disable that listen for this process.
                        Bare --direct-server enables. Default without the flag:
                        config option direct-server.

  --version             Print version and exit.
  --help, -h, -help     Print this help and exit.

Web:
  rustdesk-web-v2-direct is a separate binary; see deploy/v2/web-direct/README.md.

Examples:
  rustdesk --server --direct-access-port 21119
  rustdesk --help
  rustdesk -h
  rustdesk -help
"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_help_args_include_dash_help() {
        for arg in ["--help", "-h", "-help"] {
            assert!(is_cli_help_arg(arg), "{}", arg);
        }
        assert!(!is_cli_help_arg("--server"));
        assert!(!is_cli_help_arg("--help-me"));
        assert!(!is_cli_help_arg("-H"));
    }

    #[test]
    fn main_help_covers_direct_access_and_roles() {
        let help = main_help_text("1.5.0");
        assert!(help.starts_with("RustDesk 1.5.0"));
        for needle in [
            "--server",
            "--service",
            "--tray",
            "--connect",
            "--play",
            "--password",
            "--relay",
            "--rendezvous-server",
            "--key",
            "--video-codec",
            "--direct-access-port",
            "--direct-server",
            "rustdesk --server --direct-access-port 21119",
            "-help",
            "rustdesk-web-v2-direct",
            "deploy/v2/web-direct/README.md",
            "--direct-port",
        ] {
            assert!(help.contains(needle), "missing {}", needle);
        }
        assert!(help.contains("not persisted") || help.contains("Not persisted"));
        assert!(help.contains("Implies enable"));
    }
}
