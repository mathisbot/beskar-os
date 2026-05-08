//! QEMU command construction.

use crate::{
    config::{AccelBackend, CpuModel, DisplayMode, QemuConfig},
    pipeline::{self, Task, TaskKind},
};
use std::path::PathBuf;

const QEMU: &str = "qemu-system-x86_64";

pub fn build_args(config: &QemuConfig, output_dir: &str) -> Vec<String> {
    let mut args = vec![
        "-drive".to_string(),
        format!("if=pflash,format=raw,readonly=on,file={}", config.ovmf_path),
        "-drive".to_string(),
        format!("format=raw,file=fat:rw:{output_dir}"),
        "-smp".to_string(),
        config.smp.max(1).to_string(),
        "-m".to_string(),
        config.memory_mib.max(64).to_string(),
        "-cpu".to_string(),
        cpu_arg(config),
        "-accel".to_string(),
        config.accel.as_arg().to_string(),
        "-M".to_string(),
        config.machine.as_arg().to_string(),
        "-serial".to_string(),
        "stdio".to_string(),
        "-monitor".to_string(),
        "none".to_string(),
    ];

    if config.no_reboot {
        args.push("-no-reboot".to_string());
    }
    if config.no_shutdown {
        args.push("-no-shutdown".to_string());
    }
    if config.nic {
        args.extend([
            "-netdev".to_string(),
            "user,id=net0,net=192.168.1.0/24,host=192.168.1.1".to_string(),
            "-device".to_string(),
            "e1000e,netdev=net0,mac=52:54:00:12:34:56".to_string(),
        ]);
    }
    if config.nvme {
        args.extend(["-device".to_string(), "nvme,serial=beskar0".to_string()]);
    }
    if config.xhci {
        args.extend(["-device".to_string(), "qemu-xhci".to_string()]);
    }
    if config.usb_keyboard {
        args.extend(["-device".to_string(), "usb-kbd".to_string()]);
    }
    if config.virtio_vga {
        args.extend(["-device".to_string(), "virtio-vga".to_string()]);
    }
    match config.display {
        DisplayMode::Default => {}
        DisplayMode::Sdl | DisplayMode::Gtk => {
            args.extend([
                "-display".to_string(),
                format!("{},gl=on", config.display.label()),
            ]);
        }
        DisplayMode::None => {
            args.extend(["-display".to_string(), "none".to_string()]);
        }
    }
    if config.gdb_stub {
        args.extend(["-gdb".to_string(), "tcp::1234".to_string()]);
    }
    if config.gdb_wait {
        args.push("-S".to_string());
    }
    if config.qemu_debug_log {
        args.extend(["-d".to_string(), "int,cpu_reset,guest_errors".to_string()]);
    }

    args
}

#[must_use]
fn cpu_arg(config: &QemuConfig) -> String {
    match (config.cpu, config.accel) {
        (CpuModel::Host, AccelBackend::Kvm) => format!("{},+invtsc", CpuModel::Host.as_arg()),
        (cpu, _) => cpu.as_arg().to_string(),
    }
}

#[must_use]
pub fn command_preview(config: &QemuConfig, output_dir: &str) -> String {
    let mut preview = String::from(QEMU);
    for arg in build_args(config, output_dir) {
        preview.push(' ');
        preview.push_str(&shellish_quote(&arg));
    }
    preview
}

#[must_use]
pub fn start_qemu(config: &QemuConfig, output_dir: &str, workspace_root: PathBuf) -> Task {
    let args = build_args(config, output_dir);
    pipeline::start_logged_process(
        TaskKind::Qemu,
        QEMU.to_string(),
        args,
        workspace_root,
        format!("> launch {}", command_preview(config, output_dir)),
    )
}

#[must_use]
fn shellish_quote(arg: &str) -> String {
    if arg
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_./:=,+".contains(c))
    {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}
