#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use librustdesk::*;

#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
fn main() {
    // Allow headless API daemon even on flutter-feature builds of the binary.
    #[cfg(all(
        feature = "api-server",
        not(any(target_os = "android", target_os = "ios"))
    ))]
    {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let is_api = args.first().map(|s| {
            matches!(
                s.as_str(),
                "--api-server" | "--api-connect" | "--api-config" | "--headless-connect"
            )
        }) == Some(true);
        if is_api {
            let _ = crate::core_main::core_main();
            common::global_clean();
            return;
        }
    }

    if !common::global_init() {
        eprintln!("Global initialization failed.");
        return;
    }
    common::test_rendezvous_server();
    common::test_nat_type();
    common::global_clean();
}

#[cfg(not(any(
    target_os = "android",
    target_os = "ios",
    feature = "flutter"
)))]
fn main() {
    #[cfg(all(windows, not(feature = "inline")))]
    unsafe {
        winapi::um::shellscalingapi::SetProcessDpiAwareness(2);
    }
    if let Some(args) = crate::core_main::core_main().as_mut() {
        ui::start(args);
    }
    common::global_clean();
}
