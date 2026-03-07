//! IMDR configuration types.

/// Userspace applications that are selected by default to be included in the RAM disk.
const DEFAULT_SELECTED: &[&str] = &["bashkar"];

#[derive(Debug, Clone, PartialEq)]
pub enum Profile {
    Debug,
    Release,
}

impl Profile {
    pub const ALL: &'static [Profile] = &[Profile::Debug, Profile::Release];

    pub fn label(&self) -> &'static str {
        match self {
            Profile::Debug => "Debug",
            Profile::Release => "Release",
        }
    }

    pub fn cargo_flag(&self) -> Option<&'static str> {
        match self {
            Profile::Debug => None,
            Profile::Release => Some("--release"),
        }
    }

    pub fn dir_name(&self) -> &'static str {
        match self {
            Profile::Debug => "debug",
            Profile::Release => "release",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BuildConfig {
    pub output_dir: String,
    pub profile: Profile,
    /// `(app_name, included_in_ramdisk)`
    pub userspace_apps: Vec<(String, bool)>,
}

impl BuildConfig {
    pub fn new(available_apps: Vec<String>) -> Self {
        let userspace_apps = available_apps
            .into_iter()
            .map(|name| {
                let sel = DEFAULT_SELECTED.contains(&name.as_str());
                (name, sel)
            })
            .collect();

        Self {
            output_dir: "efi_disk".to_string(),
            profile: Profile::Debug,
            userspace_apps,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CpuType {
    Max,
    Host,
}

impl CpuType {
    pub const ALL: &'static [CpuType] = &[CpuType::Max, CpuType::Host];

    pub fn as_str(&self) -> &'static str {
        match self {
            CpuType::Max => "max",
            CpuType::Host => "host",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AccelBackend {
    Tcg,
    Kvm,
    Whpx,
    Hvf,
}

impl AccelBackend {
    pub const ALL: &'static [AccelBackend] = &[
        AccelBackend::Tcg,
        AccelBackend::Kvm,
        AccelBackend::Whpx,
        AccelBackend::Hvf,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            AccelBackend::Tcg => "tcg",
            AccelBackend::Kvm => "kvm",
            AccelBackend::Whpx => "whpx",
            AccelBackend::Hvf => "hvf",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DisplayBackend {
    Sdl,
    Gtk,
}

impl DisplayBackend {
    pub const ALL: &'static [DisplayBackend] = &[DisplayBackend::Sdl, DisplayBackend::Gtk];

    pub fn as_str(&self) -> &'static str {
        match self {
            DisplayBackend::Sdl => "sdl",
            DisplayBackend::Gtk => "gtk",
        }
    }
}

#[derive(Debug, Clone)]
pub struct QemuConfig {
    pub ovmf_path: String,
    pub cores: u32,
    pub ram_mib: u32,
    pub cpu: CpuType,
    pub accel: AccelBackend,
    pub nic: bool,
    pub nvme: bool,
    pub xhci: bool,
    pub virtio_vga: bool,
    pub display: Option<DisplayBackend>,
}

impl Default for QemuConfig {
    fn default() -> Self {
        let cpu = if cfg!(target_os = "linux") {
            CpuType::Host
        } else {
            CpuType::Max
        };

        // Do not use WHPX on Windows, as it is incompatible with OVMF firmware images.
        let accel = if cfg!(target_os = "linux") {
            AccelBackend::Kvm
        } else {
            AccelBackend::Tcg
        };

        Self {
            ovmf_path: "edk2-x86_64-code.fd".to_string(),
            cores: 4,
            ram_mib: 512,
            cpu,
            accel,
            nic: false,
            nvme: false,
            xhci: false,
            virtio_vga: false,
            display: None,
        }
    }
}
