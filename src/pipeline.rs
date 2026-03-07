//! Build pipeline.
//!
//! Discovers workspace artifacts and orchestrates `cargo build` invocations.
use crate::config::{BuildConfig, Profile};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::Sender,
    thread,
};

#[derive(Debug)]
pub enum LogMsg {
    Line(String),
    Done(bool),
}

pub fn discover_userspace_apps(workspace_root: &Path) -> Vec<String> {
    let dir = workspace_root.join("userspace");
    let mut apps = Vec::new();

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && path.join("Cargo.toml").exists()
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
                && is_binary_crate(&path)
            {
                apps.push(name.to_string());
            }
        }
    }

    apps.sort();
    apps
}

/// Returns `true` if the crate at `path` produces a binary.
fn is_binary_crate(path: &Path) -> bool {
    if path.join("src").join("main.rs").exists() {
        return true;
    }

    if let Ok(contents) = fs::read_to_string(path.join("Cargo.toml"))
        && contents.contains("[[bin]]")
    {
        return true;
    }

    false
}

fn target_dir(workspace_root: &Path) -> PathBuf {
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        return PathBuf::from(dir);
    }
    workspace_root.join("target")
}

fn bootloader_artifact(target_dir: &Path, profile: &Profile) -> PathBuf {
    target_dir
        .join("x86_64-unknown-uefi")
        .join(profile.dir_name())
        .join("bootloader.efi")
}

fn kernel_artifact(target_dir: &Path, profile: &Profile) -> PathBuf {
    target_dir
        .join("x86_64-unknown-none")
        .join(profile.dir_name())
        .join("kernel")
}

fn userspace_artifact(target_dir: &Path, profile: &Profile, app: &str) -> PathBuf {
    target_dir
        .join("x86_64-unknown-none")
        .join(profile.dir_name())
        .join(app)
}

fn run_cargo(args: &[&str], workspace_root: &Path, tx: &Sender<LogMsg>) -> bool {
    let mut cmd = Command::new("cargo");
    cmd.args(args)
        .current_dir(workspace_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("CARGO_TERM_COLOR", "never");

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(LogMsg::Line(format!("[error] failed to spawn cargo: {e}")));
            return false;
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

    let success = child.wait().map(|s| s.success()).unwrap_or(false);
    let _ = stderr_thread.join();
    let _ = stdout_thread.join();
    success
}

fn build_pipeline(config: &BuildConfig, workspace_root: &Path, tx: &Sender<LogMsg>) {
    let tdir = target_dir(workspace_root);
    let profile = &config.profile;
    let parg = profile.cargo_flag();

    macro_rules! step {
        ($label:expr, $args:expr) => {{
            let _ = tx.send(LogMsg::Line(format!("» {}", $label)));
            let mut args: Vec<&str> = $args;
            if let Some(p) = parg {
                args.push(p);
            }
            if !run_cargo(&args, workspace_root, tx) {
                let _ = tx.send(LogMsg::Line(format!("[FAILED] {}", $label)));
                let _ = tx.send(LogMsg::Done(false));
                return;
            }
            let _ = tx.send(LogMsg::Line(format!("  ✓ {}", $label)));
        }};
    }

    step!(
        "bootloader",
        vec![
            "build",
            "-p",
            "bootloader",
            "--target",
            "x86_64-unknown-uefi"
        ]
    );
    step!(
        "kernel",
        vec!["build", "-p", "kernel", "--target", "x86_64-unknown-none"]
    );

    let selected_apps: Vec<String> = config
        .userspace_apps
        .iter()
        .filter(|(_, sel)| *sel)
        .map(|(name, _)| name.clone())
        .collect();

    for app in &selected_apps {
        step!(
            format!("userspace/{app}"),
            vec![
                "build",
                "-p",
                app.as_str(),
                "--target",
                "x86_64-unknown-none"
            ]
        );
    }

    let _ = tx.send(LogMsg::Line("» assembling disk image...".to_string()));

    let out_root = workspace_root.join(&config.output_dir);
    let efi_boot = out_root.join("efi").join("boot");

    if let Err(e) = fs::create_dir_all(&efi_boot) {
        let _ = tx.send(LogMsg::Line(format!("[error] create dirs: {e}")));
        let _ = tx.send(LogMsg::Done(false));
        return;
    }

    let artifacts = [
        (
            bootloader_artifact(&tdir, profile),
            efi_boot.join("bootx64.efi"),
        ),
        (
            kernel_artifact(&tdir, profile),
            out_root.join("efi").join("kernelx64.elf"),
        ),
    ];

    for (src, dst) in &artifacts {
        if let Err(e) = fs::copy(src, dst) {
            let _ = tx.send(LogMsg::Line(format!(
                "[error] copy {} -> {}: {e}",
                src.display(),
                dst.display()
            )));
            let _ = tx.send(LogMsg::Done(false));
            return;
        }
    }

    let mut ramdisk: Vec<u8> = Vec::new();
    for app in &selected_apps {
        let src = userspace_artifact(&tdir, profile, app);
        match fs::read(&src) {
            Ok(bytes) => {
                let header = RawHeader::new(&format!("/{app}"), bytes.len());
                ramdisk.extend_from_slice(header.as_bytes());
                ramdisk.extend_from_slice(&bytes);
            }
            Err(e) => {
                let _ = tx.send(LogMsg::Line(format!("[error] read {app}: {e}")));
                let _ = tx.send(LogMsg::Done(false));
                return;
            }
        }
    }

    let ramdisk_path = out_root.join("efi").join("ramdisk.img");
    if let Err(e) = fs::write(&ramdisk_path, &ramdisk) {
        let _ = tx.send(LogMsg::Line(format!("[error] write ramdisk: {e}")));
        let _ = tx.send(LogMsg::Done(false));
        return;
    }

    let _ = tx.send(LogMsg::Line(format!(
        "  ✓ artifacts written to {}",
        out_root.display()
    )));
    let _ = tx.send(LogMsg::Done(true));
}

pub fn start_build(
    config: BuildConfig,
    workspace_root: PathBuf,
    tx: Sender<LogMsg>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || build_pipeline(&config, &workspace_root, &tx))
}

#[derive(Debug, Clone, PartialEq)]
pub enum DevOp {
    QuickBuild,
    LaunchQemu,
    Clippy,
    Test,
    DepCheck,
}

impl DevOp {
    pub fn label(&self) -> &'static str {
        match self {
            DevOp::QuickBuild => "Quick Build  [full OS, debug]",
            DevOp::LaunchQemu => "Launch QEMU",
            DevOp::Clippy => "Run Clippy   [uses package]",
            DevOp::Test => "Run Tests    [uses package]",
            DevOp::DepCheck => "Check Dependencies  [workspace]",
        }
    }

    pub const ALL: &'static [DevOp] = &[
        DevOp::QuickBuild,
        DevOp::LaunchQemu,
        DevOp::Clippy,
        DevOp::Test,
        DevOp::DepCheck,
    ];
}

pub fn start_dev_op(
    op: DevOp,
    package: String,
    workspace_root: PathBuf,
    tx: Sender<LogMsg>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let label = op.label();
        let _ = tx.send(LogMsg::Line(format!("» {label}")));

        match op {
            DevOp::QuickBuild => {
                let apps = discover_userspace_apps(&workspace_root);
                let config = BuildConfig::new(apps);
                build_pipeline(&config, &workspace_root, &tx);
            }
            DevOp::LaunchQemu => {
                // Handled at the TUI layer (suspends terminal) — never reaches here.
                let _ = tx.send(LogMsg::Done(false));
            }
            DevOp::DepCheck => {
                let success =
                    run_cargo(&["update", "--dry-run", "--verbose"], &workspace_root, &tx);
                let _ = tx.send(LogMsg::Done(success));
            }
            DevOp::Clippy => {
                let success = run_cargo(
                    &["clippy", "-p", &package, "--", "-D", "warnings"],
                    &workspace_root,
                    &tx,
                );
                let _ = tx.send(LogMsg::Done(success));
            }
            DevOp::Test => {
                let success = run_cargo(&["test", "-p", &package], &workspace_root, &tx);
                let _ = tx.send(LogMsg::Done(success));
            }
        }
    })
}

// Ramdisk header — must match the kernel ramdisk parser exactly.
#[repr(C, packed)]
struct RawHeader {
    name: [u8; 32],
    size: usize,
}

impl RawHeader {
    fn new(name: &str, size: usize) -> Self {
        let mut n = [0_u8; 32];
        let bytes = name.as_bytes();
        let len = bytes.len().min(32);
        n[..len].copy_from_slice(&bytes[..len]);
        Self { name: n, size }
    }

    fn as_bytes(&self) -> &[u8] {
        let ptr: *const Self = self;
        let len = std::mem::size_of::<Self>();
        // SAFETY: RawHeader is repr(C, packed) — no padding, safe to read as bytes.
        unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), len) }
    }
}
