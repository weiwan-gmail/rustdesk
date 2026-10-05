/// Classification of Windows display adapters for conservative Flutter
/// software-render auto-fallback. Pure string/PCI-id heuristics so they can
/// be unit-tested off Windows.

pub const ENV_FLUTTER_D3D_WARP: &str = "RUSTDESK_FLUTTER_D3D_WARP";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayAdapterInfo {
    pub device_id: String,
    pub device_string: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterKind {
    RealGpu,
    VirtualOrBasic,
    Unknown,
}

const REAL_PCI_VENDORS: &[u16] = &[
    0x10DE, // NVIDIA
    0x1002, // AMD
    0x1022, // AMD (APU / system)
    0x8086, // Intel
    0x13B5, // ARM Mali
    0x5143, // Qualcomm
];

const VIRTUAL_PCI_VENDORS: &[u16] = &[
    0x1414, // Microsoft (Basic Display / Basic Render / Hyper-V)
    0x15AD, // VMware
    0x80EE, // VirtualBox
    0x1AF4, // Red Hat VirtIO
    0x1B36, // QXL
    0x1234, // QEMU / Bochs
    0x1013, // Cirrus
    0x1A03, // ASPEED BMC
    0x102B, // Matrox (server BMC G200)
    0x5853, // Xen
    0x1AB8, // Parallels
];

pub fn pci_vendor_id(device_id: &str) -> Option<u16> {
    let upper = device_id.to_ascii_uppercase();
    let rest = upper.split("VEN_").nth(1)?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
    if digits.len() != 4 {
        return None;
    }
    u16::from_str_radix(&digits, 16).ok()
}

fn name_looks_virtual_or_basic(device_id: &str, device_string: &str) -> bool {
    let id = device_id.to_ascii_lowercase();
    let name = device_string.to_ascii_lowercase();
    let blob = format!("{id} {name}");
    const NEEDLES: &[&str] = &[
        "basic display",
        "basic render",
        "hyper-v",
        "remote display adapter",
        "rdpdd",
        "rdp display",
        "microsoft remote display",
        "citrix",
        "vmware svga",
        "vmware indirect",
        "virtualbox",
        "virtio gpu",
        "virtio-gpu",
        "red hat qemu",
        "qxl",
        "stdvga",
        "parallels display",
        "parsec virtual",
        "rustdesk virtual",
        "usb mobile monitor",
        "indirect display driver",
    ];
    NEEDLES.iter().any(|n| blob.contains(n))
        || id.starts_with("root\\")
        || id.starts_with("swd\\")
}

pub fn classify_display_adapter(adapter: &DisplayAdapterInfo) -> AdapterKind {
    if let Some(vendor) = pci_vendor_id(&adapter.device_id) {
        if REAL_PCI_VENDORS.contains(&vendor) {
            return AdapterKind::RealGpu;
        }
        if VIRTUAL_PCI_VENDORS.contains(&vendor) {
            return AdapterKind::VirtualOrBasic;
        }
        return AdapterKind::Unknown;
    }
    if name_looks_virtual_or_basic(&adapter.device_id, &adapter.device_string) {
        AdapterKind::VirtualOrBasic
    } else if adapter.device_id.is_empty() && adapter.device_string.is_empty() {
        AdapterKind::Unknown
    } else {
        AdapterKind::Unknown
    }
}

pub fn should_auto_force_software_render(adapters: &[DisplayAdapterInfo]) -> bool {
    if adapters.is_empty() {
        return false;
    }
    let kinds: Vec<AdapterKind> = adapters.iter().map(classify_display_adapter).collect();
    let any_real = kinds.iter().any(|k| *k == AdapterKind::RealGpu);
    let any_virtual = kinds.iter().any(|k| *k == AdapterKind::VirtualOrBasic);
    !any_real && any_virtual
}

/// `allow-*` options are on only when the stored value is `Y`. Empty means unset
/// (auto-detect may apply); `N` is an explicit opt-out.
pub fn should_force_d3d_warp(option_value: &str, adapters: &[DisplayAdapterInfo]) -> bool {
    if option_value == "Y" {
        return true;
    }
    if option_value == "N" {
        return false;
    }
    should_auto_force_software_render(adapters)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter(id: &str, name: &str) -> DisplayAdapterInfo {
        DisplayAdapterInfo {
            device_id: id.to_string(),
            device_string: name.to_string(),
        }
    }

    #[test]
    fn pci_vendor_parses_display_device_id() {
        assert_eq!(
            pci_vendor_id(r"PCI\VEN_10DE&DEV_2204&SUBSYS_1234"),
            Some(0x10DE)
        );
        assert_eq!(
            pci_vendor_id(r"pci\ven_1414&dev_008c"),
            Some(0x1414)
        );
        assert_eq!(pci_vendor_id(r"ROOT\BasicDisplay\0000"), None);
    }

    #[test]
    fn nvidia_is_real_gpu() {
        assert_eq!(
            classify_display_adapter(&adapter(
                r"PCI\VEN_10DE&DEV_2204",
                "NVIDIA GeForce RTX 3060"
            )),
            AdapterKind::RealGpu
        );
    }

    #[test]
    fn intel_uhd_is_real_gpu() {
        assert_eq!(
            classify_display_adapter(&adapter(
                r"PCI\VEN_8086&DEV_9A49",
                "Intel(R) UHD Graphics"
            )),
            AdapterKind::RealGpu
        );
    }

    #[test]
    fn microsoft_basic_display_is_virtual() {
        assert_eq!(
            classify_display_adapter(&adapter(
                r"PCI\VEN_1414&DEV_008C",
                "Microsoft Basic Display Adapter"
            )),
            AdapterKind::VirtualOrBasic
        );
        assert_eq!(
            classify_display_adapter(&adapter(
                r"ROOT\BasicDisplay\0000",
                "Microsoft Basic Display Adapter"
            )),
            AdapterKind::VirtualOrBasic
        );
    }

    #[test]
    fn hyperv_and_vmware_are_virtual() {
        assert_eq!(
            classify_display_adapter(&adapter(
                r"PCI\VEN_1414&DEV_5353",
                "Microsoft Hyper-V Video"
            )),
            AdapterKind::VirtualOrBasic
        );
        assert_eq!(
            classify_display_adapter(&adapter(
                r"PCI\VEN_15AD&DEV_0405",
                "VMware SVGA 3D"
            )),
            AdapterKind::VirtualOrBasic
        );
    }

    #[test]
    fn aspeed_bmc_is_virtual_for_auto_fallback() {
        assert_eq!(
            classify_display_adapter(&adapter(
                r"PCI\VEN_1A03&DEV_2000",
                "ASPEED Graphics Family"
            )),
            AdapterKind::VirtualOrBasic
        );
    }

    #[test]
    fn auto_force_only_when_every_adapter_is_virtual() {
        let basic = adapter(r"PCI\VEN_1414&DEV_008C", "Microsoft Basic Display Adapter");
        let nvidia = adapter(r"PCI\VEN_10DE&DEV_2204", "NVIDIA GeForce RTX 3060");
        assert!(should_auto_force_software_render(&[basic.clone()]));
        assert!(!should_auto_force_software_render(&[nvidia.clone(), basic]));
        assert!(!should_auto_force_software_render(&[]));
        assert!(!should_auto_force_software_render(&[adapter(
            r"PCI\VEN_ABCD&DEV_0001",
            "Mystery Adapter"
        )]));
    }

    #[test]
    fn option_y_forces_even_on_real_gpu() {
        let nvidia = adapter(r"PCI\VEN_10DE&DEV_2204", "NVIDIA GeForce RTX 3060");
        assert!(should_force_d3d_warp("Y", &[nvidia.clone()]));
        assert!(!should_force_d3d_warp("N", &[nvidia.clone()]));
        assert!(!should_force_d3d_warp("", &[nvidia]));
    }

    #[test]
    fn option_n_opts_out_of_auto_force() {
        let basic = adapter(r"PCI\VEN_1414&DEV_008C", "Microsoft Basic Display Adapter");
        assert!(!should_force_d3d_warp("N", &[basic.clone()]));
        assert!(should_force_d3d_warp("", &[basic.clone()]));
        assert!(should_force_d3d_warp("Y", &[basic]));
    }
}
