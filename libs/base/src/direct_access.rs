use std::sync::Mutex;

use hbb_common::config::{keys::OPTION_DIRECT_SERVER, option2bool};
use lazy_static::lazy_static;

lazy_static! {
    static ref CLI_DIRECT_ACCESS: Mutex<DirectAccessCli> = Mutex::new(DirectAccessCli::default());
}

/// Process-lifetime `--direct-access-port` / `--direct-server`. `None` keeps
/// UI/config defaults. Not persisted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DirectAccessCli {
    pub port: Option<i32>,
    pub enabled: Option<bool>,
}

impl DirectAccessCli {
    pub fn is_set(self) -> bool {
        self.port.is_some() || self.enabled.is_some()
    }
}

pub fn is_direct_server_value(value: &str) -> bool {
    matches!(value.trim().to_ascii_uppercase().as_str(), "Y" | "N")
}

pub fn parse_direct_server_value(value: &str) -> Result<bool, String> {
    match value.trim().to_ascii_uppercase().as_str() {
        "Y" => Ok(true),
        "N" => Ok(false),
        _ => Err(format!(
            "unknown --direct-server value {value:?}; use Y or N"
        )),
    }
}

pub fn parse_direct_access_port(value: &str) -> Result<i32, String> {
    let port = value
        .trim()
        .parse::<i32>()
        .map_err(|_| format!("invalid --direct-access-port {value:?}; use a TCP port 1-65535"))?;
    if (1..=65535).contains(&port) {
        Ok(port)
    } else {
        Err(format!(
            "invalid --direct-access-port {port}; use a TCP port 1-65535"
        ))
    }
}

pub fn take_direct_access_args(args: &mut Vec<String>) -> Result<DirectAccessCli, String> {
    let mut i = 0;
    let mut cli = DirectAccessCli::default();
    while i < args.len() {
        let arg = &args[i];
        if arg == "--direct-access-port" {
            if i + 1 >= args.len() {
                return Err("missing value for --direct-access-port".to_string());
            }
            if cli.port.is_some() {
                return Err("multiple --direct-access-port flags".to_string());
            }
            let value = args[i + 1].clone();
            args.drain(i..i + 2);
            cli.port = Some(parse_direct_access_port(&value)?);
        } else if let Some(value) = arg.strip_prefix("--direct-access-port=") {
            if cli.port.is_some() {
                return Err("multiple --direct-access-port flags".to_string());
            }
            let value = value.to_string();
            args.remove(i);
            cli.port = Some(parse_direct_access_port(&value)?);
        } else if arg == "--direct-server" {
            if cli.enabled.is_some() {
                return Err("multiple --direct-server flags".to_string());
            }
            if i + 1 < args.len() && is_direct_server_value(&args[i + 1]) {
                let value = args[i + 1].clone();
                args.drain(i..i + 2);
                cli.enabled = Some(parse_direct_server_value(&value)?);
            } else {
                args.remove(i);
                cli.enabled = Some(true);
            }
        } else if let Some(value) = arg.strip_prefix("--direct-server=") {
            if cli.enabled.is_some() {
                return Err("multiple --direct-server flags".to_string());
            }
            let value = value.to_string();
            args.remove(i);
            if value.is_empty() {
                cli.enabled = Some(true);
            } else {
                cli.enabled = Some(parse_direct_server_value(&value)?);
            }
        } else {
            i += 1;
        }
    }
    if cli.port.is_some() && cli.enabled.is_none() {
        cli.enabled = Some(true);
    }
    Ok(cli)
}

pub fn set_cli_direct_access(cli: DirectAccessCli) {
    *CLI_DIRECT_ACCESS.lock().unwrap() = cli;
}

pub fn cli_direct_access() -> DirectAccessCli {
    *CLI_DIRECT_ACCESS.lock().unwrap()
}

pub fn cli_direct_access_port() -> Option<i32> {
    cli_direct_access().port
}

pub fn cli_direct_server() -> Option<bool> {
    cli_direct_access().enabled
}

/// Port the controlled process should listen on. CLI wins over config.
pub fn direct_access_port(config_value: &str, default_port: i32) -> i32 {
    if let Some(port) = cli_direct_access_port() {
        return port;
    }
    let port = config_value.parse::<i32>().unwrap_or(0);
    if port <= 0 {
        default_port
    } else {
        port
    }
}

/// Whether the controlled process should listen for direct IP access. CLI wins
/// over config. `stop-service` is checked by the caller.
pub fn direct_server_enabled(config_value: &str) -> bool {
    if let Some(enabled) = cli_direct_server() {
        return enabled;
    }
    option2bool(OPTION_DIRECT_SERVER, config_value)
}

#[cfg(test)]
mod tests {
    use super::*;

    lazy_static! {
        static ref TEST_LOCK: Mutex<()> = Mutex::new(());
    }

    fn reset() -> std::sync::MutexGuard<'static, ()> {
        let guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_cli_direct_access(DirectAccessCli::default());
        guard
    }

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn parse_port_rejects_non_tcp_range() {
        assert_eq!(parse_direct_access_port("21119").unwrap(), 21119);
        assert_eq!(parse_direct_access_port(" 1 ").unwrap(), 1);
        assert_eq!(parse_direct_access_port("65535").unwrap(), 65535);
        assert!(parse_direct_access_port("0").is_err());
        assert!(parse_direct_access_port("-1").is_err());
        assert!(parse_direct_access_port("65536").is_err());
        assert!(parse_direct_access_port("abc").is_err());
    }

    #[test]
    fn take_direct_access_port_implies_enable_and_strips() {
        let _guard = reset();
        let mut a = args(&["--server", "--direct-access-port", "21119"]);
        let cli = take_direct_access_args(&mut a).unwrap();
        assert_eq!(
            cli,
            DirectAccessCli {
                port: Some(21119),
                enabled: Some(true),
            }
        );
        assert_eq!(a, args(&["--server"]));
        set_cli_direct_access(cli);
        assert_eq!(direct_access_port("21118", 21118), 21119);
        assert!(direct_server_enabled(""));
        assert!(direct_server_enabled("N"));
    }

    #[test]
    fn take_direct_access_port_equals_form() {
        let mut a = args(&["--direct-access-port=21120", "--server"]);
        let cli = take_direct_access_args(&mut a).unwrap();
        assert_eq!(cli.port, Some(21120));
        assert_eq!(cli.enabled, Some(true));
        assert_eq!(a, args(&["--server"]));
    }

    #[test]
    fn take_direct_server_bare_flag_enables_without_changing_port() {
        let _guard = reset();
        let mut a = args(&["--direct-server", "--server"]);
        let cli = take_direct_access_args(&mut a).unwrap();
        assert_eq!(
            cli,
            DirectAccessCli {
                port: None,
                enabled: Some(true),
            }
        );
        assert_eq!(a, args(&["--server"]));
        set_cli_direct_access(cli);
        assert_eq!(direct_access_port("", 21118), 21118);
        assert_eq!(direct_access_port("21120", 21118), 21120);
        assert!(direct_server_enabled(""));
    }

    #[test]
    fn take_direct_server_yn() {
        let mut a = args(&["--direct-server", "Y", "--server"]);
        assert_eq!(take_direct_access_args(&mut a).unwrap().enabled, Some(true));
        assert_eq!(a, args(&["--server"]));

        let mut a = args(&["--direct-server=n", "--server"]);
        let cli = take_direct_access_args(&mut a).unwrap();
        assert_eq!(cli.enabled, Some(false));
        assert_eq!(a, args(&["--server"]));
    }

    #[test]
    fn omitted_cli_keeps_config() {
        let _guard = reset();
        assert_eq!(direct_access_port("", 21118), 21118);
        assert_eq!(direct_access_port("0", 21118), 21118);
        assert_eq!(direct_access_port("21120", 21118), 21120);
        assert!(!direct_server_enabled(""));
        assert!(direct_server_enabled("Y"));
        assert!(!direct_server_enabled("N"));
    }

    #[test]
    fn explicit_direct_server_n_wins_over_port_imply_enable() {
        let mut a = args(&[
            "--direct-access-port",
            "21119",
            "--direct-server",
            "N",
            "--server",
        ]);
        let cli = take_direct_access_args(&mut a).unwrap();
        assert_eq!(cli.port, Some(21119));
        assert_eq!(cli.enabled, Some(false));
        assert_eq!(a, args(&["--server"]));
    }

    #[test]
    fn take_rejects_unknown_and_missing_and_duplicate() {
        let mut a = args(&["--direct-access-port"]);
        assert!(take_direct_access_args(&mut a)
            .unwrap_err()
            .contains("missing value"));

        let mut a = args(&["--direct-access-port", "nope"]);
        assert!(take_direct_access_args(&mut a)
            .unwrap_err()
            .contains("nope"));

        let mut a = args(&["--direct-access-port", "1", "--direct-access-port=2"]);
        assert!(take_direct_access_args(&mut a)
            .unwrap_err()
            .contains("multiple"));

        let mut a = args(&["--direct-server=maybe"]);
        assert!(take_direct_access_args(&mut a)
            .unwrap_err()
            .contains("maybe"));
    }

    #[test]
    fn take_leaves_server_flag_when_no_direct_access_args() {
        let mut a = args(&["--server"]);
        let cli = take_direct_access_args(&mut a).unwrap();
        assert!(!cli.is_set());
        assert_eq!(a, args(&["--server"]));
    }

    #[test]
    fn take_direct_server_then_port_keeps_server_command() {
        let mut a = args(&[
            "--direct-server",
            "--direct-access-port",
            "21119",
            "--server",
        ]);
        let cli = take_direct_access_args(&mut a).unwrap();
        assert_eq!(cli.port, Some(21119));
        assert_eq!(cli.enabled, Some(true));
        assert_eq!(a, args(&["--server"]));
    }
}
