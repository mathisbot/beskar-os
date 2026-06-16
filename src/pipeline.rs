//! Build orchestration and process logging.

use crate::config::{BuildConfig, Profile};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    Build,
    Qemu,
}

impl TaskKind {
    #[must_use]
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Qemu => "qemu",
        }
    }
}

#[derive(Debug)]
pub enum TaskEvent {
    Line(String),
    Finished { kind: TaskKind, success: bool },
}

const RAMDISK_NAME_LEN: usize = 32;

pub struct Task {
    pub rx: Receiver<TaskEvent>,
    handle: Option<JoinHandle<()>>,
}

impl Task {
    pub fn join(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        self.join();
    }
}

pub fn discover_userspace_apps(workspace_root: &Path) -> Vec<String> {
    let userspace_dir = workspace_root.join("userspace");
    let mut apps = Vec::new();

    let Ok(entries) = fs::read_dir(userspace_dir) else {
        return apps;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir()
            && path.join("Cargo.toml").exists()
            && is_binary_crate(&path)
            && let Some(name) = path.file_name().and_then(|name| name.to_str())
        {
            apps.push(name.to_string());
        }
    }

    apps.sort();
    apps
}

#[must_use]
#[inline]
fn is_binary_crate(path: &Path) -> bool {
    path.join("src/main.rs").exists()
        || fs::read_to_string(path.join("Cargo.toml"))
            .is_ok_and(|contents| contents.contains("[[bin]]"))
}

pub fn start_build(config: BuildConfig, workspace_root: PathBuf) -> Task {
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
        let success = build_pipeline(&config, &workspace_root, &tx);
        let _ = tx.send(TaskEvent::Finished {
            kind: TaskKind::Build,
            success,
        });
    });

    Task {
        rx,
        handle: Some(handle),
    }
}

pub fn start_logged_process(
    kind: TaskKind,
    program: String,
    args: Vec<String>,
    workspace_root: PathBuf,
    first_line: String,
) -> Task {
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
        let _ = tx.send(TaskEvent::Line(first_line));
        let success = run_command(&program, &args, &workspace_root, &tx);
        let _ = tx.send(TaskEvent::Finished { kind, success });
    });

    Task {
        rx,
        handle: Some(handle),
    }
}

fn build_pipeline(config: &BuildConfig, workspace_root: &Path, tx: &Sender<TaskEvent>) -> bool {
    let target_dir = target_dir(workspace_root);
    let profile = config.profile;
    let selected_apps = config
        .selected_apps()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if !cargo_step(
        "bootloader",
        &[
            "build",
            "-p",
            "bootloader",
            "--target",
            "x86_64-unknown-uefi",
        ],
        profile,
        workspace_root,
        tx,
    ) {
        return false;
    }

    if !cargo_step(
        "kernel",
        &["build", "-p", "kernel", "--target", "x86_64-unknown-none"],
        profile,
        workspace_root,
        tx,
    ) {
        return false;
    }

    for app in &selected_apps {
        if !cargo_step(
            &format!("userspace/{app}"),
            &["build", "-p", app, "--target", "x86_64-unknown-none"],
            profile,
            workspace_root,
            tx,
        ) {
            return false;
        }
    }

    assemble_disk(config, &selected_apps, workspace_root, &target_dir, tx)
}

fn cargo_step(
    label: &str,
    base_args: &[&str],
    profile: Profile,
    workspace_root: &Path,
    tx: &Sender<TaskEvent>,
) -> bool {
    let _ = tx.send(TaskEvent::Line(format!("> build {label}")));

    let mut args = base_args
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if let Some(flag) = profile.cargo_flag() {
        args.push(flag.to_string());
    }

    if run_command("cargo", &args, workspace_root, tx) {
        let _ = tx.send(TaskEvent::Line(format!("  ok {label}")));
        true
    } else {
        let _ = tx.send(TaskEvent::Line(format!("  failed {label}")));
        false
    }
}

fn run_command(
    program: &str,
    args: &[String],
    workspace_root: &Path,
    tx: &Sender<TaskEvent>,
) -> bool {
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(workspace_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("CARGO_TERM_COLOR", "never");

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            let _ = tx.send(TaskEvent::Line(format!(
                "[error] failed to spawn {program}: {err}"
            )));
            return false;
        }
    };

    let stdout_thread = child
        .stdout
        .take()
        .map(|stream| spawn_line_pipe(stream, tx.clone()));
    let stderr_thread = child
        .stderr
        .take()
        .map(|stream| spawn_line_pipe(stream, tx.clone()));

    let success = match child.wait() {
        Ok(status) => status.success(),
        Err(err) => {
            let _ = tx.send(TaskEvent::Line(format!(
                "[error] failed to wait for {program}: {err}"
            )));
            false
        }
    };

    if let Some(handle) = stdout_thread {
        let _ = handle.join();
    }
    if let Some(handle) = stderr_thread {
        let _ = handle.join();
    }

    success
}

fn spawn_line_pipe<R>(stream: R, tx: Sender<TaskEvent>) -> JoinHandle<()>
where
    R: std::io::Read + Send + 'static,
{
    thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            let _ = tx.send(TaskEvent::Line(line));
        }
    })
}

fn assemble_disk(
    config: &BuildConfig,
    selected_apps: &[String],
    workspace_root: &Path,
    target_dir: &Path,
    tx: &Sender<TaskEvent>,
) -> bool {
    let _ = tx.send(TaskEvent::Line("> assemble efi image".to_string()));

    let out_root = workspace_root.join(&config.output_dir);
    let efi_dir = out_root.join("efi");
    let efi_boot_dir = efi_dir.join("boot");
    if let Err(err) = fs::create_dir_all(&efi_boot_dir) {
        let _ = tx.send(TaskEvent::Line(format!("[error] create EFI tree: {err}")));
        return false;
    }

    let artifacts = [
        (
            bootloader_artifact(target_dir, config.profile),
            efi_boot_dir.join("bootx64.efi"),
        ),
        (
            kernel_artifact(target_dir, config.profile),
            efi_dir.join("kernelx64.elf"),
        ),
    ];

    for (src, dst) in artifacts {
        if let Err(err) = fs::copy(&src, &dst) {
            let _ = tx.send(TaskEvent::Line(format!(
                "[error] copy {} -> {}: {err}",
                src.display(),
                dst.display()
            )));
            return false;
        }
    }

    let mut ramdisk = Vec::new();
    for app in selected_apps {
        let artifact = userspace_artifact(target_dir, config.profile, app);
        match fs::read(&artifact) {
            Ok(bytes) => {
                let ramdisk_name = format!("/{app}");
                let header = match RawHeader::new(&ramdisk_name, bytes.len()) {
                    Ok(header) => header,
                    Err(err) => {
                        let _ = tx.send(TaskEvent::Line(format!("[error] {err}: {ramdisk_name}")));
                        return false;
                    }
                };
                header.append_to(&mut ramdisk);
                ramdisk.extend_from_slice(&bytes);
                let _ = tx.send(TaskEvent::Line(format!(
                    "  ramdisk + /{app} ({} bytes)",
                    bytes.len()
                )));
            }
            Err(err) => {
                let _ = tx.send(TaskEvent::Line(format!(
                    "[error] read {}: {err}",
                    artifact.display()
                )));
                return false;
            }
        }
    }

    let ramdisk_path = efi_dir.join("ramdisk.img");
    if let Err(err) = fs::write(&ramdisk_path, &ramdisk) {
        let _ = tx.send(TaskEvent::Line(format!("[error] write ramdisk: {err}")));
        return false;
    }

    let _ = tx.send(TaskEvent::Line(format!(
        "  ready {} ({} ramdisk bytes)",
        out_root.display(),
        ramdisk.len()
    )));
    true
}

#[must_use]
#[inline]
fn target_dir(workspace_root: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map_or_else(|| workspace_root.join("target"), PathBuf::from)
}

#[must_use]
#[inline]
fn bootloader_artifact(target_dir: &Path, profile: Profile) -> PathBuf {
    target_dir
        .join("x86_64-unknown-uefi")
        .join(profile.target_dir())
        .join("bootloader.efi")
}

#[must_use]
#[inline]
fn kernel_artifact(target_dir: &Path, profile: Profile) -> PathBuf {
    target_dir
        .join("x86_64-unknown-none")
        .join(profile.target_dir())
        .join("kernel")
}

#[must_use]
#[inline]
fn userspace_artifact(target_dir: &Path, profile: Profile, app: &str) -> PathBuf {
    target_dir
        .join("x86_64-unknown-none")
        .join(profile.target_dir())
        .join(app)
}

#[repr(C, packed)]
struct RawHeader {
    name: [u8; RAMDISK_NAME_LEN],
    size: u64,
}

impl RawHeader {
    fn new(name: &str, size: usize) -> Result<Self, &'static str> {
        let mut fixed_name = [0; RAMDISK_NAME_LEN];
        let bytes = name.as_bytes();
        if bytes.len() > fixed_name.len() {
            return Err("ramdisk path exceeds fixed header name length");
        }
        let size = u64::try_from(size).map_err(|_| "ramdisk payload size exceeds u64")?;
        let len = bytes.len();
        fixed_name[..len].copy_from_slice(&bytes[..len]);
        Ok(Self {
            name: fixed_name,
            size,
        })
    }

    #[inline]
    fn append_to(&self, ramdisk: &mut Vec<u8>) {
        ramdisk.extend_from_slice(&self.name);
        // The kernel ramdisk parser runs on x86_64, where usize is 8-byte little-endian.
        ramdisk.extend_from_slice(&self.size.to_le_bytes());
    }
}
