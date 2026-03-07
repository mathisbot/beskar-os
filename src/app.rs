//! IMDR application state and event-handling logic.
use crate::{
    config::{AccelBackend, BuildConfig, CpuType, DisplayBackend, Profile, QemuConfig},
    pipeline::{DevOp, LogMsg},
};
use std::{path::PathBuf, sync::mpsc::Receiver, thread::JoinHandle};

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    MainMenu,
    BuildDossier,
    DevTools,
}

pub const MAIN_ITEMS: &[(&str, &str)] = &[
    (
        "Research Dossier: Build & Deployment",
        "Configure and assemble a BeskarOS image. Optionally launch in QEMU.",
    ),
    (
        "Development Tools",
        "Fast iteration: build, lint, test, dependency audit.",
    ),
    ("Exit Terminal", "Terminate this session."),
];

pub struct RunningOp {
    pub rx: Receiver<LogMsg>,
    // TODO: Somehow join this handle?
    pub _handle: JoinHandle<()>,
}

/// All mutable fields for the Build Dossier form.
pub struct BuildForm {
    pub output_dir: String,
    pub profile_idx: usize,

    pub apps: Vec<(String, bool)>,

    pub ovmf: String,
    pub cores: String,
    pub ram: String,
    pub cpu_idx: usize,
    pub accel_idx: usize,
    pub nic: bool,
    pub nvme: bool,
    pub xhci: bool,
    pub virtio_vga: bool,
    pub display_idx: usize,

    pub selected: usize,
    pub scroll: usize,
    pub log_scroll: usize,
}

impl BuildForm {
    pub fn new(build_cfg: &BuildConfig, qemu_cfg: &QemuConfig) -> Self {
        let profile_idx = match build_cfg.profile {
            Profile::Debug => 0,
            Profile::Release => 1,
        };
        let cpu_idx = CpuType::ALL
            .iter()
            .position(|c| c == &qemu_cfg.cpu)
            .unwrap_or(0);
        let accel_idx = AccelBackend::ALL
            .iter()
            .position(|a| a == &qemu_cfg.accel)
            .unwrap_or(0);
        let display_idx = match &qemu_cfg.display {
            None => 0,
            Some(d) => DisplayBackend::ALL
                .iter()
                .position(|x| x == d)
                .map(|i| i + 1)
                .unwrap_or(0),
        };

        Self {
            output_dir: build_cfg.output_dir.clone(),
            profile_idx,
            apps: build_cfg.userspace_apps.clone(),
            ovmf: qemu_cfg.ovmf_path.clone(),
            cores: qemu_cfg.cores.to_string(),
            ram: qemu_cfg.ram_mib.to_string(),
            cpu_idx,
            accel_idx,
            nic: qemu_cfg.nic,
            nvme: qemu_cfg.nvme,
            xhci: qemu_cfg.xhci,
            virtio_vga: qemu_cfg.virtio_vga,
            display_idx,
            selected: 0,
            scroll: 0,
            log_scroll: 0,
        }
    }

    /// Total navigable field count (including dynamic app entries).
    pub fn field_count(&self) -> usize {
        14 + self.apps.len()
    }

    /// Returns which kind of field `idx` represents.
    pub fn field_at(&self, idx: usize) -> FormField {
        let n = self.apps.len();
        match idx {
            0 => FormField::OutputDir,
            1 => FormField::Profile,
            i if i < 2 + n => FormField::App(i - 2),
            i if i == 2 + n => FormField::Ovmf,
            i if i == 3 + n => FormField::Cores,
            i if i == 4 + n => FormField::Ram,
            i if i == 5 + n => FormField::Cpu,
            i if i == 6 + n => FormField::Accel,
            i if i == 7 + n => FormField::Nic,
            i if i == 8 + n => FormField::Nvme,
            i if i == 9 + n => FormField::Xhci,
            i if i == 10 + n => FormField::VirtioVga,
            i if i == 11 + n => FormField::Display,
            i if i == 12 + n => FormField::ActionBuild,
            _ => FormField::ActionQemu,
        }
    }

    /// Extracts a `BuildConfig` from the form state.
    pub fn to_build_config(&self) -> BuildConfig {
        BuildConfig {
            output_dir: self.output_dir.clone(),
            profile: if self.profile_idx == 0 {
                Profile::Debug
            } else {
                Profile::Release
            },
            userspace_apps: self.apps.clone(),
        }
    }

    /// Extracts a `QemuConfig` from the form state.
    pub fn to_qemu_config(&self) -> QemuConfig {
        let display = match self.display_idx {
            0 => None,
            1 => Some(DisplayBackend::Sdl),
            _ => Some(DisplayBackend::Gtk),
        };
        QemuConfig {
            ovmf_path: self.ovmf.clone(),
            cores: self.cores.parse().unwrap_or(4),
            ram_mib: self.ram.parse().unwrap_or(512),
            cpu: CpuType::ALL[self.cpu_idx % CpuType::ALL.len()].clone(),
            accel: AccelBackend::ALL[self.accel_idx % AccelBackend::ALL.len()].clone(),
            nic: self.nic,
            nvme: self.nvme,
            xhci: self.xhci,
            virtio_vga: self.virtio_vga,
            display,
        }
    }
}

/// Semantic field type derived from field index.
#[derive(Debug, Clone, PartialEq)]
pub enum FormField {
    OutputDir,
    Profile,
    App(usize),
    Ovmf,
    Cores,
    Ram,
    Cpu,
    Accel,
    Nic,
    Nvme,
    Xhci,
    VirtioVga,
    Display,
    ActionBuild,
    ActionQemu,
}

impl FormField {
    pub fn is_text(&self) -> bool {
        matches!(
            self,
            FormField::OutputDir | FormField::Ovmf | FormField::Cores | FormField::Ram
        )
    }
    pub fn is_checkbox(&self) -> bool {
        matches!(
            self,
            FormField::App(_)
                | FormField::Nic
                | FormField::Nvme
                | FormField::Xhci
                | FormField::VirtioVga
        )
    }
    pub fn is_action(&self) -> bool {
        matches!(self, FormField::ActionBuild | FormField::ActionQemu)
    }
}

pub struct DevForm {
    pub package: String,
    pub op_selected: usize,
    pub log_scroll: usize,
}

impl Default for DevForm {
    fn default() -> Self {
        Self {
            package: "kernel".to_string(),
            op_selected: 0,
            log_scroll: 0,
        }
    }
}

pub struct App {
    pub screen: Screen,
    pub main_sel: usize,

    pub build_form: BuildForm,
    pub dev_form: DevForm,
    pub pkg_field_focused: bool,

    pub log_lines: Vec<String>,
    pub running: Option<RunningOp>,
    pub last_op_success: Option<bool>,

    pub should_quit: bool,
}

impl App {
    pub fn new(_workspace_root: PathBuf, build_cfg: BuildConfig, qemu_cfg: QemuConfig) -> Self {
        let build_form = BuildForm::new(&build_cfg, &qemu_cfg);
        Self {
            screen: Screen::MainMenu,
            main_sel: 0,
            build_form,
            dev_form: DevForm::default(),
            pkg_field_focused: false,
            log_lines: Vec::new(),
            running: None,
            last_op_success: None,
            should_quit: false,
        }
    }

    /// Drains any pending log messages from the active operation.
    pub fn poll_logs(&mut self) {
        if let Some(op) = &self.running {
            loop {
                match op.rx.try_recv() {
                    Ok(LogMsg::Line(s)) => {
                        self.log_lines.push(s);
                    }
                    Ok(LogMsg::Done(ok)) => {
                        self.last_op_success = Some(ok);
                        self.running = None;
                        break;
                    }
                    Err(_) => break,
                }
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    pub fn set_running(&mut self, rx: Receiver<LogMsg>, handle: std::thread::JoinHandle<()>) {
        self.log_lines.clear();
        self.last_op_success = None;
        self.running = Some(RunningOp {
            rx,
            _handle: handle,
        });
        // Reset log scroll to bottom.
        self.build_form.log_scroll = usize::MAX;
        self.dev_form.log_scroll = usize::MAX;
    }

    pub fn main_menu_up(&mut self) {
        if self.main_sel > 0 {
            self.main_sel -= 1;
        }
    }

    pub fn main_menu_down(&mut self) {
        if self.main_sel + 1 < MAIN_ITEMS.len() {
            self.main_sel += 1;
        }
    }

    pub fn main_menu_select(&mut self) {
        match self.main_sel {
            0 => self.screen = Screen::BuildDossier,
            1 => self.screen = Screen::DevTools,
            _ => self.should_quit = true,
        }
    }

    pub fn form_next(&mut self) {
        let max = self.build_form.field_count().saturating_sub(1);
        if self.build_form.selected < max {
            self.build_form.selected += 1;
        }
    }

    pub fn form_prev(&mut self) {
        if self.build_form.selected > 0 {
            self.build_form.selected -= 1;
        }
    }

    /// Handles Enter/Space on the focused build form field.
    pub fn form_activate(&mut self) -> FormActivateResult {
        let field = self.build_form.field_at(self.build_form.selected);
        match field {
            FormField::App(i) => {
                if let Some((_, sel)) = self.build_form.apps.get_mut(i) {
                    *sel = !*sel;
                }
                FormActivateResult::None
            }
            FormField::Nic => {
                self.build_form.nic = !self.build_form.nic;
                FormActivateResult::None
            }
            FormField::Nvme => {
                self.build_form.nvme = !self.build_form.nvme;
                FormActivateResult::None
            }
            FormField::Xhci => {
                self.build_form.xhci = !self.build_form.xhci;
                FormActivateResult::None
            }
            FormField::VirtioVga => {
                self.build_form.virtio_vga = !self.build_form.virtio_vga;
                FormActivateResult::None
            }
            FormField::ActionBuild => FormActivateResult::TriggerBuild,
            FormField::ActionQemu => FormActivateResult::TriggerQemu,
            _ => FormActivateResult::None,
        }
    }

    /// Handles Left key on the focused build form field (cycle selector backwards).
    pub fn form_cycle_left(&mut self) {
        let field = self.build_form.field_at(self.build_form.selected);
        match field {
            FormField::Profile => {
                let n = Profile::ALL.len();
                self.build_form.profile_idx = (self.build_form.profile_idx + n - 1) % n;
            }
            FormField::Cpu => {
                let n = CpuType::ALL.len();
                self.build_form.cpu_idx = (self.build_form.cpu_idx + n - 1) % n;
            }
            FormField::Accel => {
                let n = AccelBackend::ALL.len();
                self.build_form.accel_idx = (self.build_form.accel_idx + n - 1) % n;
            }
            FormField::Display => {
                let n = DisplayBackend::ALL.len() + 1; // +1 for None
                self.build_form.display_idx = (self.build_form.display_idx + n - 1) % n;
            }
            _ => {}
        }
    }

    /// Handles Right key on the focused build form field (cycle selector forwards).
    pub fn form_cycle_right(&mut self) {
        let field = self.build_form.field_at(self.build_form.selected);
        match field {
            FormField::Profile => {
                self.build_form.profile_idx =
                    (self.build_form.profile_idx + 1) % Profile::ALL.len();
            }
            FormField::Cpu => {
                self.build_form.cpu_idx = (self.build_form.cpu_idx + 1) % CpuType::ALL.len();
            }
            FormField::Accel => {
                self.build_form.accel_idx =
                    (self.build_form.accel_idx + 1) % AccelBackend::ALL.len();
            }
            FormField::Display => {
                self.build_form.display_idx =
                    (self.build_form.display_idx + 1) % (DisplayBackend::ALL.len() + 1);
            }
            _ => {}
        }
    }

    /// Handles character input on a focused text field.
    pub fn form_type_char(&mut self, c: char) {
        let field = self.build_form.field_at(self.build_form.selected);
        let text = match field {
            FormField::OutputDir => &mut self.build_form.output_dir,
            FormField::Ovmf => &mut self.build_form.ovmf,
            FormField::Cores => &mut self.build_form.cores,
            FormField::Ram => &mut self.build_form.ram,
            _ => return,
        };
        // Basic ASCII filter for numeric-only fields.
        if matches!(field, FormField::Cores | FormField::Ram) && !c.is_ascii_digit() {
            return;
        }
        text.push(c);
    }

    pub fn form_backspace(&mut self) {
        let field = self.build_form.field_at(self.build_form.selected);
        let text = match field {
            FormField::OutputDir => &mut self.build_form.output_dir,
            FormField::Ovmf => &mut self.build_form.ovmf,
            FormField::Cores => &mut self.build_form.cores,
            FormField::Ram => &mut self.build_form.ram,
            _ => return,
        };
        text.pop();
    }

    pub fn dev_op_up(&mut self) {
        if self.dev_form.op_selected > 0 {
            self.dev_form.op_selected -= 1;
        }
    }

    pub fn dev_op_down(&mut self) {
        if self.dev_form.op_selected + 1 < DevOp::ALL.len() {
            self.dev_form.op_selected += 1;
        }
    }

    pub fn dev_type_char(&mut self, c: char) {
        self.dev_form.package.push(c);
    }

    pub fn dev_backspace(&mut self) {
        self.dev_form.package.pop();
    }
}

pub enum FormActivateResult {
    None,
    TriggerBuild,
    TriggerQemu,
}
