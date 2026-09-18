use std::sync::Mutex;

use crate::message_proto::supported_decoding::PreferCodec;
use lazy_static::lazy_static;
use log::warn;

lazy_static! {
    static ref CLI_VIDEO_CODEC: Mutex<Option<PreferCodec>> = Mutex::new(None);
}

pub const VIDEO_CODEC_VALUES: &str = "auto|vp8|vp9|av1|h264|h265";

/// Skip process-lifetime option flags so process-mode detection
/// (`--server`, `--cm`, GUI) still sees the real command as argv[1].
pub fn next_command_arg<I>(args: I) -> Option<String>
where
    I: IntoIterator<Item = String>,
{
    let mut skip_value = false;
    let mut skip_optional_bool = false;
    for arg in args {
        if skip_value {
            skip_value = false;
            continue;
        }
        if skip_optional_bool {
            skip_optional_bool = false;
            if crate::direct_access::is_direct_server_value(&arg) {
                continue;
            }
        }
        if arg == "--video-codec" || arg == "--direct-access-port" {
            skip_value = true;
            continue;
        }
        if arg.starts_with("--video-codec=")
            || arg.starts_with("--direct-access-port=")
            || arg.starts_with("--direct-server=")
        {
            continue;
        }
        if arg == "--direct-server" {
            skip_optional_bool = true;
            continue;
        }
        return Some(arg);
    }
    None
}

pub fn take_video_codec_arg(args: &mut Vec<String>) -> Result<Option<PreferCodec>, String> {
    let mut i = 0;
    let mut found = None;
    while i < args.len() {
        let arg = &args[i];
        let value = if arg == "--video-codec" {
            if i + 1 >= args.len() {
                return Err(format!(
                    "missing value for --video-codec ({VIDEO_CODEC_VALUES})"
                ));
            }
            let value = args[i + 1].clone();
            args.drain(i..i + 2);
            Some(value)
        } else if let Some(value) = arg.strip_prefix("--video-codec=") {
            let value = value.to_string();
            args.remove(i);
            Some(value)
        } else {
            i += 1;
            None
        };
        if let Some(value) = value {
            if found.is_some() {
                return Err("multiple --video-codec flags".to_string());
            }
            found = Some(parse_video_codec_name(&value)?);
        }
    }
    Ok(found)
}

/// Parse a `--video-codec` value. Names match per-peer `codec-preference`.
pub fn parse_video_codec_name(name: &str) -> Result<PreferCodec, String> {
    match name.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(PreferCodec::Auto),
        "vp8" => Ok(PreferCodec::VP8),
        "vp9" => Ok(PreferCodec::VP9),
        "av1" => Ok(PreferCodec::AV1),
        "h264" => Ok(PreferCodec::H264),
        "h265" => Ok(PreferCodec::H265),
        _ => Err(format!(
            "unknown video codec {name:?}; use auto, vp8, vp9, av1, h264, or h265"
        )),
    }
}

/// Process-lifetime encode/decode preference from `--video-codec`. `None` keeps
/// UI/config defaults. Not persisted.
pub fn set_cli_video_codec(codec: Option<PreferCodec>) {
    *CLI_VIDEO_CODEC.lock().unwrap() = codec;
}

pub fn cli_video_codec() -> Option<PreferCodec> {
    *CLI_VIDEO_CODEC.lock().unwrap()
}

/// Host CLI overrides peer prefer when that codec is usable for every peer.
pub fn effective_cli_encode_preference(
    peer: PreferCodec,
    vp8_useable: bool,
    av1_useable: bool,
    h264_useable: bool,
    h265_useable: bool,
) -> PreferCodec {
    let Some(cli) = cli_video_codec() else {
        return peer;
    };
    let usable = match cli {
        PreferCodec::Auto => return peer,
        PreferCodec::VP8 => vp8_useable,
        PreferCodec::VP9 => true,
        PreferCodec::AV1 => av1_useable,
        PreferCodec::H264 => h264_useable,
        PreferCodec::H265 => h265_useable,
    };
    if usable {
        cli
    } else {
        warn!(
            "--video-codec {:?} is not usable with current peers, keeping {:?}",
            cli, peer
        );
        peer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    lazy_static! {
        static ref TEST_LOCK: Mutex<()> = Mutex::new(());
    }

    fn reset() -> std::sync::MutexGuard<'static, ()> {
        let guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_cli_video_codec(None);
        guard
    }

    #[test]
    fn parse_video_codec_name_matches_codec_preference_keys() {
        assert_eq!(parse_video_codec_name("vp8").unwrap(), PreferCodec::VP8);
        assert_eq!(parse_video_codec_name("VP9").unwrap(), PreferCodec::VP9);
        assert_eq!(parse_video_codec_name(" av1 ").unwrap(), PreferCodec::AV1);
        assert_eq!(parse_video_codec_name("h264").unwrap(), PreferCodec::H264);
        assert_eq!(parse_video_codec_name("h265").unwrap(), PreferCodec::H265);
        assert_eq!(parse_video_codec_name("auto").unwrap(), PreferCodec::Auto);
        assert!(parse_video_codec_name("mpeg2").is_err());
    }

    #[test]
    fn omitted_cli_keeps_peer_preference() {
        let _guard = reset();
        assert_eq!(
            effective_cli_encode_preference(PreferCodec::VP9, true, true, false, false),
            PreferCodec::VP9
        );
    }

    #[test]
    fn cli_vp8_reaches_encode_preference_when_peer_can_decode() {
        let _guard = reset();
        set_cli_video_codec(Some(PreferCodec::VP8));
        assert_eq!(
            effective_cli_encode_preference(PreferCodec::Auto, true, false, false, false),
            PreferCodec::VP8
        );
        assert_eq!(
            effective_cli_encode_preference(PreferCodec::VP9, true, false, false, false),
            PreferCodec::VP8
        );
    }

    #[test]
    fn cli_h264_is_ignored_when_not_usable() {
        let _guard = reset();
        set_cli_video_codec(Some(PreferCodec::H264));
        assert_eq!(
            effective_cli_encode_preference(PreferCodec::Auto, true, false, false, false),
            PreferCodec::Auto
        );
    }

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn next_command_arg_skips_video_codec_flag() {
        assert_eq!(
            next_command_arg(args(&["--video-codec", "vp8", "--server"])),
            Some("--server".to_string())
        );
        assert_eq!(
            next_command_arg(args(&["--server", "--video-codec=vp9"])),
            Some("--server".to_string())
        );
        assert_eq!(next_command_arg(args(&["--video-codec", "vp8"])), None);
        assert_eq!(next_command_arg(args(&["--cm"])), Some("--cm".to_string()));
    }

    #[test]
    fn next_command_arg_skips_direct_access_flags() {
        assert_eq!(
            next_command_arg(args(&["--direct-access-port", "21119", "--server"])),
            Some("--server".to_string())
        );
        assert_eq!(
            next_command_arg(args(&["--direct-server", "--server"])),
            Some("--server".to_string())
        );
        assert_eq!(
            next_command_arg(args(&["--direct-server", "Y", "--server"])),
            Some("--server".to_string())
        );
        assert_eq!(
            next_command_arg(args(&[
                "--direct-access-port=21119",
                "--video-codec",
                "vp8",
                "--server"
            ])),
            Some("--server".to_string())
        );
        assert_eq!(
            next_command_arg(args(&["--direct-server=N", "--cm"])),
            Some("--cm".to_string())
        );
    }

    #[test]
    fn take_video_codec_arg_parses_strips_and_reaches_encode_path() {
        let _guard = reset();
        let mut a = args(&["--server", "--video-codec", "vp8"]);
        let codec = take_video_codec_arg(&mut a).unwrap();
        assert_eq!(codec, Some(PreferCodec::VP8));
        assert_eq!(a, args(&["--server"]));
        set_cli_video_codec(codec);
        assert_eq!(
            effective_cli_encode_preference(PreferCodec::Auto, true, false, false, false),
            PreferCodec::VP8
        );

        let mut a = args(&["--video-codec=VP9", "--connect", "id"]);
        assert_eq!(
            take_video_codec_arg(&mut a).unwrap(),
            Some(PreferCodec::VP9)
        );
        assert_eq!(a, args(&["--connect", "id"]));

        let mut a = args(&["--server"]);
        assert_eq!(take_video_codec_arg(&mut a).unwrap(), None);
        assert_eq!(a, args(&["--server"]));
    }

    #[test]
    fn take_video_codec_arg_rejects_unknown_and_missing() {
        let mut a = args(&["--video-codec", "mpeg2"]);
        assert!(take_video_codec_arg(&mut a).unwrap_err().contains("mpeg2"));

        let mut a = args(&["--video-codec"]);
        assert!(take_video_codec_arg(&mut a)
            .unwrap_err()
            .contains("missing value"));
    }
}
