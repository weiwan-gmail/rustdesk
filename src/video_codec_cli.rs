use base::message_proto::supported_decoding::PreferCodec;
use base::video_codec::{set_cli_video_codec, VIDEO_CODEC_VALUES};
use hbb_common::log;

pub use base::video_codec::{next_command_arg, take_video_codec_arg};

pub fn apply_cli_video_codec(codec: PreferCodec) {
    log::info!("cli --video-codec {:?}", codec);
    set_cli_video_codec(Some(codec));
}

pub fn print_video_codec_help() {
    println!("RustDesk {}", crate::VERSION);
    println!();
    println!("  --video-codec <{VIDEO_CODEC_VALUES}>");
    println!("      Select video encode mode for this process. When connecting as a viewer,");
    println!("      this is also the decode preference sent to the peer.");
    println!("      Omitted: keep UI/config defaults (not persisted).");
    println!();
    println!("Examples:");
    println!("  rustdesk --server --video-codec vp9");
    println!("  rustdesk --video-codec vp8");
    println!("  rustdesk --connect <id> --video-codec av1");
}
