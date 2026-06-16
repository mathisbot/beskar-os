//! Build and emulator configuration.

use core::fmt;

const DEFAULT_OUTPUT_DIR: &str = "efi_disk";
const DEFAULT_RAMDISK_APPS: &[&str] = &["bashkar"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Debug,
    Release,
}

impl Profile {
    pub const ALL: [Self; 2] = [Self::Debug, Self::Release];

    #[must_use]
    #[inline]
    pub const fn cargo_flag(self) -> Option<&'static str> {
        match self {
            Self::Debug => None,
            Self::Release => Some("--release"),
        }
    }

    #[must_use]
    #[inline]
    pub const fn target_dir(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }

    #[must_use]
    #[inline]
    pub fn next(self) -> Self {
        cycle_value(self, &Self::ALL, 1)
    }

    #[must_use]
    #[inline]
    pub fn previous(self) -> Self {
        cycle_value(self, &Self::ALL, -1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RamdiskEntry {
    pub name: String,
    pub enabled: bool,
}

impl RamdiskEntry {
    #[must_use]
    #[inline]
    pub fn new(name: String) -> Self {
        let enabled = DEFAULT_RAMDISK_APPS.contains(&name.as_str());
        Self { name, enabled }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildConfig {
    pub output_dir: String,
    pub profile: Profile,
    pub ramdisk: Vec<RamdiskEntry>,
}

impl BuildConfig {
    #[must_use]
    #[inline]
    pub fn new(available_apps: Vec<String>) -> Self {
        Self {
            output_dir: DEFAULT_OUTPUT_DIR.to_string(),
            profile: Profile::Debug,
            ramdisk: available_apps.into_iter().map(RamdiskEntry::new).collect(),
        }
    }

    pub fn selected_apps(&self) -> impl Iterator<Item = &str> {
        self.ramdisk
            .iter()
            .filter(|entry| entry.enabled)
            .map(|entry| entry.name.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuModel {
    Host,
    Max,
    Qemu64,
}

impl CpuModel {
    pub const ALL: [Self; 3] = [Self::Host, Self::Max, Self::Qemu64];

    #[must_use]
    #[inline]
    pub const fn as_arg(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Max => "max",
            Self::Qemu64 => "qemu64",
        }
    }

    #[must_use]
    #[inline]
    pub fn next(self) -> Self {
        cycle_value(self, &Self::ALL, 1)
    }

    #[must_use]
    #[inline]
    pub fn previous(self) -> Self {
        cycle_value(self, &Self::ALL, -1)
    }
}

impl fmt::Display for CpuModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_arg())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccelBackend {
    Kvm,
    Tcg,
    Whpx,
    Hvf,
}

impl AccelBackend {
    pub const ALL: [Self; 4] = [Self::Kvm, Self::Tcg, Self::Whpx, Self::Hvf];

    #[must_use]
    #[inline]
    pub const fn as_arg(self) -> &'static str {
        match self {
            Self::Kvm => "kvm",
            Self::Tcg => "tcg",
            Self::Whpx => "whpx",
            Self::Hvf => "hvf",
        }
    }

    #[must_use]
    #[inline]
    pub fn next(self) -> Self {
        cycle_value(self, &Self::ALL, 1)
    }

    #[must_use]
    #[inline]
    pub fn previous(self) -> Self {
        cycle_value(self, &Self::ALL, -1)
    }
}

impl fmt::Display for AccelBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_arg())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Machine {
    Q35,
    Pc,
}

impl Machine {
    pub const ALL: [Self; 2] = [Self::Q35, Self::Pc];

    #[must_use]
    #[inline]
    pub const fn as_arg(self) -> &'static str {
        match self {
            Self::Q35 => "q35",
            Self::Pc => "pc",
        }
    }

    #[must_use]
    #[inline]
    pub fn next(self) -> Self {
        cycle_value(self, &Self::ALL, 1)
    }

    #[must_use]
    #[inline]
    pub fn previous(self) -> Self {
        cycle_value(self, &Self::ALL, -1)
    }
}

impl fmt::Display for Machine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_arg())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    Default,
    Sdl,
    Gtk,
    None,
}

impl DisplayMode {
    pub const ALL: [Self; 4] = [Self::Default, Self::Sdl, Self::Gtk, Self::None];

    #[must_use]
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Sdl => "sdl",
            Self::Gtk => "gtk",
            Self::None => "none",
        }
    }

    #[must_use]
    #[inline]
    pub fn next(self) -> Self {
        cycle_value(self, &Self::ALL, 1)
    }

    #[must_use]
    #[inline]
    pub fn previous(self) -> Self {
        cycle_value(self, &Self::ALL, -1)
    }
}

impl fmt::Display for DisplayMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct QemuConfig {
    pub ovmf_path: String,
    pub accel: AccelBackend,
    pub cpu: CpuModel,
    pub machine: Machine,
    pub smp: u16,
    pub memory_mib: u32,
    pub nic: bool,
    pub nvme: bool,
    pub xhci: bool,
    pub usb_keyboard: bool,
    pub virtio_vga: bool,
    pub display: DisplayMode,
    pub no_reboot: bool,
    pub no_shutdown: bool,
    pub gdb_stub: bool,
    pub gdb_wait: bool,
    pub qemu_debug_log: bool,
}

impl Default for QemuConfig {
    fn default() -> Self {
        Self {
            ovmf_path: "edk2-x86_64-code.fd".to_string(),
            accel: default_accel(),
            cpu: default_cpu(),
            machine: Machine::Q35,
            smp: 4,
            memory_mib: 512,
            nic: false,
            nvme: false,
            xhci: false,
            usb_keyboard: false,
            virtio_vga: false,
            display: DisplayMode::Default,
            no_reboot: true,
            no_shutdown: false,
            gdb_stub: false,
            gdb_wait: false,
            qemu_debug_log: false,
        }
    }
}

#[must_use]
#[inline]
const fn default_cpu() -> CpuModel {
    if cfg!(target_os = "linux") {
        CpuModel::Host
    } else {
        CpuModel::Max
    }
}

#[must_use]
#[inline]
const fn default_accel() -> AccelBackend {
    if cfg!(target_os = "linux") {
        AccelBackend::Kvm
    } else {
        AccelBackend::Tcg
    }
}

fn cycle_value<T: Copy + Eq>(current: T, values: &[T], direction: i8) -> T {
    let current_idx = values
        .iter()
        .position(|value| *value == current)
        .unwrap_or(0);
    let next_idx = if direction.is_negative() {
        current_idx.checked_sub(1).unwrap_or(values.len() - 1)
    } else {
        (current_idx + 1) % values.len()
    };
    values[next_idx]
}
