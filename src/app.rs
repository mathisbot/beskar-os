//! One-screen TUI state and input behavior.

use crate::{
    config::{BuildConfig, QemuConfig},
    pipeline::{Task, TaskEvent, TaskKind},
};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use std::path::PathBuf;

const MAX_LOG_LINES: usize = 20_000;
const BASE_CONTROL_COUNT: usize = 5;
const QEMU_CONTROLS: [Control; 17] = [
    Control::Ovmf,
    Control::Accel,
    Control::Cpu,
    Control::Machine,
    Control::Smp,
    Control::Memory,
    Control::Nic,
    Control::Nvme,
    Control::Xhci,
    Control::UsbKeyboard,
    Control::VirtioVga,
    Control::Display,
    Control::NoReboot,
    Control::NoShutdown,
    Control::GdbStub,
    Control::GdbWait,
    Control::QemuDebugLog,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Build,
    BuildAndRun,
    RunQemu,
    Profile,
    OutputDir,
    Ramdisk(usize),
    Ovmf,
    Accel,
    Cpu,
    Machine,
    Smp,
    Memory,
    Nic,
    Nvme,
    Xhci,
    UsbKeyboard,
    VirtioVga,
    Display,
    NoReboot,
    NoShutdown,
    GdbStub,
    GdbWait,
    QemuDebugLog,
}

impl Control {
    #[must_use]
    #[inline]
    pub const fn is_text(&self) -> bool {
        matches!(self, Self::OutputDir | Self::Ovmf)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    Build,
    BuildAndRun,
    RunQemu,
}

pub struct RunningTask {
    pub task: Task,
    pub then: Option<Action>,
}

impl RunningTask {
    #[must_use]
    #[inline]
    pub const fn new(task: Task, then: Option<Action>) -> Self {
        Self { task, then }
    }
}

pub struct Logs {
    lines: Vec<String>,
    top: usize,
    view_height: usize,
    follow: bool,
    full_screen: bool,
}

impl Logs {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            lines: Vec::new(),
            top: 0,
            view_height: 1,
            follow: true,
            full_screen: false,
        }
    }

    #[must_use]
    #[inline]
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.lines.len()
    }

    #[must_use]
    #[inline]
    pub const fn top(&self) -> usize {
        self.top
    }

    #[must_use]
    #[inline]
    pub const fn follow(&self) -> bool {
        self.follow
    }

    #[must_use]
    #[inline]
    pub const fn full_screen(&self) -> bool {
        self.full_screen
    }

    #[inline]
    pub const fn close_full_screen(&mut self) {
        self.full_screen = false;
    }

    #[inline]
    pub const fn toggle_full_screen(&mut self) {
        self.full_screen = !self.full_screen;
    }

    pub fn push(&mut self, line: String) {
        self.lines.push(line);
        if self.lines.len() > MAX_LOG_LINES {
            let excess = self.lines.len() - MAX_LOG_LINES;
            self.lines.drain(0..excess);
        }
        self.clamp();
    }

    pub fn set_view_height(&mut self, height: usize) {
        self.view_height = height.max(1);
        self.clamp();
    }

    pub fn scroll(&mut self, delta: isize) {
        let max_scroll = self.max_scroll();
        let current = if self.follow { max_scroll } else { self.top };
        self.follow = false;
        self.top = current.saturating_add_signed(delta).min(max_scroll);
        if self.top == max_scroll && delta.is_positive() {
            self.follow = true;
        }
    }

    pub const fn scroll_top(&mut self) {
        self.follow = false;
        self.top = 0;
    }

    pub const fn scroll_tail(&mut self) {
        self.follow = true;
        self.top = self.max_scroll();
    }

    pub const fn toggle_follow(&mut self) {
        self.follow = !self.follow;
        if self.follow {
            self.top = self.max_scroll();
        }
    }

    fn clamp(&mut self) {
        self.top = if self.follow {
            self.max_scroll()
        } else {
            self.top.min(self.max_scroll())
        };
    }

    #[must_use]
    #[inline]
    const fn max_scroll(&self) -> usize {
        self.lines.len().saturating_sub(self.view_height)
    }
}

#[allow(clippy::struct_excessive_bools)]
pub struct App {
    pub workspace_root: PathBuf,
    pub build: BuildConfig,
    pub qemu: QemuConfig,
    pub selected: usize,
    pub editing: bool,
    pub logs: Logs,
    pub running: Option<RunningTask>,
    pub last_result: Option<(TaskKind, bool)>,
    pub should_quit: bool,
}

impl App {
    #[must_use]
    pub const fn new(workspace_root: PathBuf, build: BuildConfig, qemu: QemuConfig) -> Self {
        Self {
            workspace_root,
            build,
            qemu,
            selected: 0,
            editing: false,
            logs: Logs::new(),
            running: None,
            last_result: None,
            should_quit: false,
        }
    }

    #[must_use]
    pub fn selected_control(&self) -> Control {
        match self.selected {
            0 => Control::Build,
            1 => Control::BuildAndRun,
            2 => Control::RunQemu,
            3 => Control::Profile,
            4 => Control::OutputDir,
            index if index < BASE_CONTROL_COUNT + self.build.ramdisk.len() => {
                Control::Ramdisk(index - BASE_CONTROL_COUNT)
            }
            index => QEMU_CONTROLS
                .get(index - BASE_CONTROL_COUNT - self.build.ramdisk.len())
                .copied()
                .unwrap_or(Control::Build),
        }
    }

    #[must_use]
    #[inline]
    pub fn is_selected(&self, control: Control) -> bool {
        self.selected_control() == control
    }

    #[must_use]
    #[inline]
    pub const fn is_running(&self) -> bool {
        self.running.is_some()
    }

    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Action {
        if self.editing {
            return self.handle_edit_key(code);
        }

        if self.logs.full_screen() {
            return self.handle_log_key(code, modifiers);
        }

        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                if !self.is_running() {
                    self.should_quit = true;
                }
                Action::None
            }
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                if !self.is_running() {
                    self.should_quit = true;
                }
                Action::None
            }
            KeyCode::Char('b') if !self.is_running() => Action::Build,
            KeyCode::Char('r') if !self.is_running() => Action::RunQemu,
            KeyCode::Char('B') if !self.is_running() => Action::BuildAndRun,
            KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => {
                self.select_previous();
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.select_next();
                Action::None
            }
            KeyCode::Left | KeyCode::Char('-') => {
                self.adjust_selected(-1);
                Action::None
            }
            KeyCode::Right | KeyCode::Char('+') => {
                self.adjust_selected(1);
                Action::None
            }
            KeyCode::Enter | KeyCode::Char(' ') => self.activate_selected(),
            KeyCode::Char('a') => {
                self.set_all_ramdisk(true);
                Action::None
            }
            KeyCode::Char('n') => {
                self.set_all_ramdisk(false);
                Action::None
            }
            KeyCode::PageUp | KeyCode::Char('[' | 'u') => {
                self.logs.scroll(-10);
                Action::None
            }
            KeyCode::PageDown | KeyCode::Char(']' | 'd') => {
                self.logs.scroll(10);
                Action::None
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.logs.scroll_top();
                Action::None
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.logs.scroll_tail();
                Action::None
            }
            KeyCode::Char('f') => {
                self.logs.toggle_follow();
                Action::None
            }
            KeyCode::Char('l') => {
                self.logs.toggle_full_screen();
                Action::None
            }
            _ => Action::None,
        }
    }

    pub fn poll_task(&mut self) -> Option<Action> {
        let running = self.running.as_ref()?;
        let mut finished = None;

        loop {
            match running.task.rx.try_recv() {
                Ok(TaskEvent::Line(line)) => self.logs.push(line),
                Ok(TaskEvent::Finished { kind, success }) => {
                    finished = Some((kind, success));
                    break;
                }
                Err(_) => break,
            }
        }

        let (kind, success) = finished?;
        let then = self.running.as_ref().and_then(|running| running.then);

        if let Some(mut running) = self.running.take() {
            running.task.join();
        }

        self.last_result = Some((kind, success));
        self.logs.push(format!(
            "{} {}",
            kind.label(),
            if success { "complete" } else { "failed" }
        ));

        if success {
            return then;
        }

        None
    }

    pub fn set_running(&mut self, task: Task, then: Option<Action>) {
        self.running = Some(RunningTask::new(task, then));
        self.last_result = None;
        self.logs.scroll_tail();
    }

    fn handle_log_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Action {
        match code {
            KeyCode::Char('q' | 'l') | KeyCode::Esc => {
                self.logs.close_full_screen();
            }
            KeyCode::Char('c')
                if modifiers.contains(KeyModifiers::CONTROL) && !self.is_running() =>
            {
                self.should_quit = true;
            }
            KeyCode::PageUp | KeyCode::Char('[' | 'u') => self.logs.scroll(-10),
            KeyCode::PageDown | KeyCode::Char(']' | 'd') => self.logs.scroll(10),
            KeyCode::Home | KeyCode::Char('g') => self.logs.scroll_top(),
            KeyCode::End | KeyCode::Char('G') => self.logs.scroll_tail(),
            KeyCode::Char('f') => self.logs.toggle_follow(),
            _ => {}
        }

        Action::None
    }

    fn handle_edit_key(&mut self, code: KeyCode) -> Action {
        match code {
            KeyCode::Enter | KeyCode::Esc => {
                self.editing = false;
            }
            KeyCode::Backspace => {
                self.text_field_mut().pop();
            }
            KeyCode::Char(char) if !char.is_control() => {
                self.text_field_mut().push(char);
            }
            _ => {}
        }
        Action::None
    }

    fn activate_selected(&mut self) -> Action {
        if self.is_running() {
            return Action::None;
        }

        match self.selected_control() {
            Control::Build => Action::Build,
            Control::BuildAndRun => Action::BuildAndRun,
            Control::RunQemu => Action::RunQemu,
            control if control.is_text() => {
                self.editing = true;
                Action::None
            }
            _ => {
                self.adjust_selected(1);
                Action::None
            }
        }
    }

    #[inline]
    const fn select_previous(&mut self) {
        self.editing = false;
        self.selected = self.selected.saturating_sub(1);
    }

    #[inline]
    fn select_next(&mut self) {
        self.editing = false;
        let max = self.control_count().saturating_sub(1);
        self.selected = (self.selected + 1).min(max);
    }

    fn adjust_selected(&mut self, direction: i32) {
        match self.selected_control() {
            Control::Profile => {
                self.build.profile = if direction.is_negative() {
                    self.build.profile.previous()
                } else {
                    self.build.profile.next()
                };
            }
            Control::Ramdisk(index) => {
                if let Some(entry) = self.build.ramdisk.get_mut(index) {
                    entry.enabled = !entry.enabled;
                }
            }
            Control::Accel => {
                self.qemu.accel = if direction.is_negative() {
                    self.qemu.accel.previous()
                } else {
                    self.qemu.accel.next()
                };
            }
            Control::Cpu => {
                self.qemu.cpu = if direction.is_negative() {
                    self.qemu.cpu.previous()
                } else {
                    self.qemu.cpu.next()
                };
            }
            Control::Machine => {
                self.qemu.machine = if direction.is_negative() {
                    self.qemu.machine.previous()
                } else {
                    self.qemu.machine.next()
                };
            }
            Control::Smp => {
                self.qemu.smp = step_u16(self.qemu.smp, direction, 1, 256);
            }
            Control::Memory => {
                self.qemu.memory_mib = step_u32(self.qemu.memory_mib, direction * 64, 64, 262_144);
            }
            Control::Nic => self.qemu.nic = !self.qemu.nic,
            Control::Nvme => self.qemu.nvme = !self.qemu.nvme,
            Control::Xhci => self.qemu.xhci = !self.qemu.xhci,
            Control::UsbKeyboard => self.qemu.usb_keyboard = !self.qemu.usb_keyboard,
            Control::VirtioVga => self.qemu.virtio_vga = !self.qemu.virtio_vga,
            Control::Display => {
                self.qemu.display = if direction.is_negative() {
                    self.qemu.display.previous()
                } else {
                    self.qemu.display.next()
                };
            }
            Control::NoReboot => self.qemu.no_reboot = !self.qemu.no_reboot,
            Control::NoShutdown => self.qemu.no_shutdown = !self.qemu.no_shutdown,
            Control::GdbStub => {
                self.qemu.gdb_stub = !self.qemu.gdb_stub;
                if !self.qemu.gdb_stub {
                    self.qemu.gdb_wait = false;
                }
            }
            Control::GdbWait => {
                self.qemu.gdb_wait = !self.qemu.gdb_wait;
                if self.qemu.gdb_wait {
                    self.qemu.gdb_stub = true;
                }
            }
            Control::QemuDebugLog => self.qemu.qemu_debug_log = !self.qemu.qemu_debug_log,
            Control::Build
            | Control::BuildAndRun
            | Control::RunQemu
            | Control::OutputDir
            | Control::Ovmf => {}
        }
    }

    #[must_use]
    #[inline]
    fn text_field_mut(&mut self) -> &mut String {
        match self.selected_control() {
            Control::Ovmf => &mut self.qemu.ovmf_path,
            _ => &mut self.build.output_dir,
        }
    }

    #[inline]
    fn set_all_ramdisk(&mut self, enabled: bool) {
        for entry in &mut self.build.ramdisk {
            entry.enabled = enabled;
        }
    }
}

impl App {
    #[must_use]
    #[inline]
    const fn control_count(&self) -> usize {
        BASE_CONTROL_COUNT + self.build.ramdisk.len() + QEMU_CONTROLS.len()
    }
}

#[must_use]
#[inline]
fn step_u16(value: u16, direction: i32, min: u16, max: u16) -> u16 {
    let next = i32::from(value) + direction;
    u16::try_from(next.clamp(i32::from(min), i32::from(max))).unwrap_or(min)
}

#[must_use]
#[inline]
fn step_u32(value: u32, delta: i32, min: u32, max: u32) -> u32 {
    let next = i64::from(value) + i64::from(delta);
    u32::try_from(next.clamp(i64::from(min), i64::from(max))).unwrap_or(min)
}
