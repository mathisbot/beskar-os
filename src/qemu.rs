//! QEMU command generation and launcher.
use crate::{
    config::{DisplayBackend, QemuConfig},
    pipeline::LogMsg,
};
use std::{
    io::{BufRead, BufReader},
    path::Path,
    process::{Command, Stdio},
    sync::mpsc::Sender,
    thread,
};

/// Constructs the QEMU argument list from config.
pub fn build_args(config: &QemuConfig, output_dir: &str) -> Vec<String> {
    let mut args = Vec::new();

    args.push(format!(
        "-drive if=pflash,format=raw,readonly=on,file={}",
        config.ovmf_path
    ));
    args.push(format!("-drive format=raw,file=fat:rw:{output_dir}"));
    args.push(format!("-smp {}", config.cores));
    args.push(format!("-m {}", config.ram_mib));
    args.push(format!("-cpu {}", config.cpu.as_str()));
    args.push(format!("-accel {}", config.accel.as_str()));
    args.push("-serial stdio".to_string());
    args.push("-M q35".to_string());

    if config.nic {
        args.push("-netdev user,id=net0,net=192.168.1.0/24,host=192.168.1.1".to_string());
        args.push("-device e1000e,netdev=net0,mac=52:54:00:12:34:56".to_string());
    }
    if config.nvme {
        args.push("-device nvme,serial=beskar0".to_string());
    }
    if config.xhci {
        args.push("-device qemu-xhci".to_string());
    }
    if config.virtio_vga {
        let display = config
            .display
            .as_ref()
            .map_or("sdl", DisplayBackend::as_str);
        args.push("-device virtio-vga".to_string());
        args.push(format!("-display {display},gl=on"));
    }

    args
}

/// Returns a human-readable multi-line command preview.
pub fn command_preview(config: &QemuConfig, output_dir: &str) -> String {
    let args = build_args(config, output_dir);
    let joined = args
        .iter()
        .map(|a| format!("  {a}"))
        .collect::<Vec<_>>()
        .join(" \\\n");
    format!("qemu-system-x86_64 \\\n{joined}")
}

/// Spawns QEMU in a background thread, piping its output into `tx`.
pub fn start_qemu(
    config: QemuConfig,
    output_dir: String,
    workspace_root: &Path,
    tx: Sender<LogMsg>,
) -> thread::JoinHandle<()> {
    let workspace_root = workspace_root.to_path_buf();

    thread::spawn(move || {
        let args = build_args(&config, &output_dir);
        let _ = tx.send(LogMsg::Line(format!(
            "» launching: qemu-system-x86_64 {}",
            args.join(" ")
        )));

        let mut cmd = Command::new("qemu-system-x86_64");
        cmd.args(args.iter().flat_map(|a| {
            // Each arg string is built as "flag value" — split at first space so the OS
            // receives them as two separate tokens, which is what QEMU expects.
            a.splitn(2, ' ').map(str::to_owned).collect::<Vec<_>>()
        }))
        .current_dir(&workspace_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(LogMsg::Line(format!("[error] failed to spawn QEMU: {e}")));
                let _ = tx.send(LogMsg::Done(false));
                return;
            }
        };

        let stderr = child.stderr.take().expect("stderr piped");
        let tx2 = tx.clone();
        let stderr_thread = thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let _ = tx2.send(LogMsg::Line(line));
            }
        });

        let stdout = child.stdout.take().expect("stdout piped");
        let tx3 = tx.clone();
        let stdout_thread = thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let _ = tx3.send(LogMsg::Line(line));
            }
        });

        let success = child.wait().is_ok_and(|status| status.success());
        let _ = stderr_thread.join();
        let _ = stdout_thread.join();
        let _ = tx.send(LogMsg::Done(success));
    })
}
