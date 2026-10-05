use crate::platform::virtual_gpu::{
    should_force_d3d_warp, DisplayAdapterInfo, ENV_FLUTTER_D3D_WARP,
};
use base::config::keys;
use hbb_common::{config::Config, log};
use winapi::{
    shared::minwindef::FALSE,
    um::winuser::{EnumDisplayDevicesW, DISPLAY_DEVICE_MIRRORING_DRIVER, DISPLAY_DEVICEW},
};

fn wchar_zstring(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

fn enum_display_adapters() -> Vec<DisplayAdapterInfo> {
    let mut adapters = Vec::new();
    let mut i: u32 = 0;
    loop {
        let mut dd: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
        dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
        let ok = unsafe { EnumDisplayDevicesW(std::ptr::null(), i, &mut dd, 0) };
        if ok == FALSE {
            break;
        }
        i += 1;
        if dd.StateFlags & DISPLAY_DEVICE_MIRRORING_DRIVER != 0 {
            continue;
        }
        adapters.push(DisplayAdapterInfo {
            device_id: wchar_zstring(&dd.DeviceID),
            device_string: wchar_zstring(&dd.DeviceString),
        });
    }
    adapters
}

/// Set `RUSTDESK_FLUTTER_D3D_WARP=1` before the Flutter engine starts so the
/// Windows runner can force ANGLE onto D3D11 WARP. Called from `core_main`.
pub fn apply_flutter_software_render_env() {
    let option = Config::get_option(keys::OPTION_ALLOW_ALWAYS_SOFTWARE_RENDER);
    let adapters = enum_display_adapters();
    for a in &adapters {
        log::info!(
            "Display adapter: '{}' id={}",
            a.device_string,
            a.device_id
        );
    }

    let already = std::env::var(ENV_FLUTTER_D3D_WARP).ok().as_deref() == Some("1");
    let force = if option == "N" {
        false
    } else if option == "Y" || already {
        true
    } else {
        should_force_d3d_warp(&option, &adapters)
    };

    if force {
        #[allow(unused_unsafe)]
        unsafe {
            std::env::set_var(ENV_FLUTTER_D3D_WARP, "1");
        }
        if option == "Y" {
            log::info!(
                "allow-always-software-render=Y: forcing Flutter Windows ANGLE onto D3D11 WARP ({})",
                ENV_FLUTTER_D3D_WARP
            );
        } else if already {
            log::info!(
                "Honoring existing {}=1 for Flutter Windows ANGLE D3D11 WARP",
                ENV_FLUTTER_D3D_WARP
            );
        } else {
            log::info!(
                "No real GPU adapter detected (headless / Basic Display / virtual GPU). Forcing Flutter Windows ANGLE onto D3D11 WARP for this process. Set allow-always-software-render=Y to persist."
            );
        }
    } else {
        #[allow(unused_unsafe)]
        unsafe {
            std::env::remove_var(ENV_FLUTTER_D3D_WARP);
        }
    }
}
