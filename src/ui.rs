use crate::{
    app::{App, Control},
    config::Profile,
    qemu,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    frame.render_widget(Block::default().style(style_canvas()), area);

    let [header, body, footer] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .areas(area);

    if app.logs.full_screen() {
        render_log_header(frame, header);
        render_plain_log_view(frame, app, body);
    } else {
        render_header(frame, header);
        render_body(frame, app, body);
    }
    render_footer(frame, app, footer);
}

fn render_header(frame: &mut Frame<'_>, area: Rect) {
    let header = Text::from(vec![
        Line::from(vec![
            Span::styled(" BESKAR-OS", style_title()),
            Span::styled(" // foundry", style_accent()),
        ]),
        Line::from(Span::styled(" beskar systems standing by", style_dim())),
    ]);
    frame.render_widget(Paragraph::new(header), area);
}

fn render_body(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let [controls, logs] =
        Layout::horizontal([Constraint::Percentage(42), Constraint::Percentage(58)]).areas(area);
    let [build, qemu] =
        Layout::vertical([Constraint::Percentage(38), Constraint::Percentage(62)]).areas(controls);

    render_build_panel(frame, app, build);
    render_qemu_panel(frame, app, qemu);
    render_log_panel(frame, app, logs);
}

fn render_log_header(frame: &mut Frame<'_>, area: Rect) {
    let header = Text::from(vec![
        Line::from(vec![
            Span::styled(" BESKAR-OS", style_title()),
            Span::styled(" // log archive", style_accent()),
        ]),
        Line::from(Span::styled(" full log view", style_dim())),
    ]);
    frame.render_widget(Paragraph::new(header), area);
}

fn render_build_panel(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let mut lines = vec![
        section("ACTIONS"),
        action_line(app, Control::Build, "build image", "b"),
        action_line(app, Control::BuildAndRun, "build + run", "B"),
        action_line(app, Control::RunQemu, "run qemu", "r"),
        blank(),
        section("BUILD"),
        enum_line(
            app,
            Control::Profile,
            "profile",
            match app.build.profile {
                Profile::Debug => "debug",
                Profile::Release => "release",
            },
        ),
        text_line(
            app,
            Control::OutputDir,
            "destination",
            &app.build.output_dir,
        ),
        blank(),
        section("RAMDISK"),
    ];

    if app.build.ramdisk.is_empty() {
        lines.push(Line::from(Span::styled(
            "  no userspace binaries discovered",
            style_dim(),
        )));
    } else {
        for (index, entry) in app.build.ramdisk.iter().enumerate() {
            lines.push(toggle_line(
                app,
                Control::Ramdisk(index),
                &entry.name,
                entry.enabled,
            ));
        }
    }

    let block = deck_block(" image ", build_panel_active(app));
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .style(style_panel())
            .block(block),
        area,
    );
}

fn render_qemu_panel(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let lines = vec![
        section("FIRMWARE"),
        text_line(app, Control::Ovmf, "ovmf", &app.qemu.ovmf_path),
        blank(),
        section("CPU"),
        enum_line(app, Control::Accel, "accel", app.qemu.accel.as_arg()),
        enum_line(app, Control::Cpu, "cpu", app.qemu.cpu.as_arg()),
        enum_line(app, Control::Machine, "machine", app.qemu.machine.as_arg()),
        number_line(app, Control::Smp, "smp", &app.qemu.smp.to_string()),
        number_line(
            app,
            Control::Memory,
            "memory",
            &format!("{} MiB", app.qemu.memory_mib),
        ),
        blank(),
        section("DEVICES"),
        toggle_line(app, Control::Nic, "e1000e nic", app.qemu.nic),
        toggle_line(app, Control::Nvme, "nvme", app.qemu.nvme),
        toggle_line(app, Control::Xhci, "xhci", app.qemu.xhci),
        toggle_line(
            app,
            Control::UsbKeyboard,
            "usb keyboard",
            app.qemu.usb_keyboard,
        ),
        toggle_line(app, Control::VirtioVga, "virtio-vga", app.qemu.virtio_vga),
        enum_line(app, Control::Display, "display", app.qemu.display.label()),
        blank(),
        section("DEBUG"),
        toggle_line(app, Control::NoReboot, "no reboot", app.qemu.no_reboot),
        toggle_line(
            app,
            Control::NoShutdown,
            "no shutdown",
            app.qemu.no_shutdown,
        ),
        toggle_line(app, Control::GdbStub, "gdb :1234", app.qemu.gdb_stub),
        toggle_line(app, Control::GdbWait, "wait for gdb", app.qemu.gdb_wait),
        toggle_line(
            app,
            Control::QemuDebugLog,
            "qemu debug",
            app.qemu.qemu_debug_log,
        ),
        blank(),
        section("PREVIEW"),
        Line::from(Span::styled(
            qemu::command_preview(&app.qemu, &app.build.output_dir),
            style_dim(),
        )),
    ];

    let block = deck_block(" qemu ", qemu_panel_active(app));
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .style(style_panel())
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_log_panel(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let height = area.height.saturating_sub(2) as usize;
    app.logs.set_view_height(height);
    let scroll = app.logs.top();

    let title = if app.logs.follow() {
        format!(" logs  {} lines  tail ", app.logs.len())
    } else {
        format!(" logs  {} lines  scroll:{} ", app.logs.len(), scroll)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style_border())
        .title(Span::styled(title, style_accent()));

    frame.render_widget(
        Paragraph::new(log_text(app))
            .style(style_panel())
            .block(block)
            .scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0)),
        area,
    );
}

fn render_plain_log_view(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let height = area.height as usize;
    app.logs.set_view_height(height);
    let scroll = app.logs.top();

    frame.render_widget(
        Paragraph::new(log_text(app))
            .style(style_panel())
            .scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0)),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let edit_hint = if app.editing {
        "EDIT: type text, Enter/Esc commit"
    } else if app.is_running() {
        if app.logs.full_screen() {
            "LOGS: [ ] or u d scroll | g top | G tail | f follow | l/Esc close"
        } else {
            "RUNNING: [ ] or u d scroll logs | g top | G tail | f follow | l full logs"
        }
    } else if app.logs.full_screen() {
        "LOGS: [ ] or u d scroll | g top | G tail | f follow | l/Esc close | q close"
    } else {
        "NAV: Up/Down select | Left/Right change | Enter/Space activate | [ ]/u d logs | l full logs | q quit"
    };

    let footer = Text::from(vec![
        Line::from(Span::styled(edit_hint, style_dim())),
        selected_hint(app),
    ]);
    frame.render_widget(Paragraph::new(footer).style(style_canvas()), area);
}

fn log_text(app: &App) -> Text<'_> {
    app.logs
        .lines()
        .iter()
        .map(|line| Line::from(Span::styled(line.as_str(), log_style(line))))
        .collect::<Vec<_>>()
        .into()
}

fn action_line(app: &App, control: Control, label: &str, key: &str) -> Line<'static> {
    let selected = app.is_selected(control);
    let marker = if selected { ">" } else { " " };
    let status = if app.is_running() { "locked" } else { key };
    Line::from(vec![
        Span::styled(format!("{marker} "), style_marker(selected)),
        Span::styled(format!("{label:<18}"), style_control(selected)),
        Span::styled(format!(" {status:^6} "), style_pill(selected)),
    ])
}

fn text_line(app: &App, control: Control, label: &str, value: &str) -> Line<'static> {
    let selected = app.is_selected(control);
    let cursor = if selected && app.editing { "_" } else { "" };
    Line::from(vec![
        Span::styled(prefix(selected), style_marker(selected)),
        Span::styled(format!("{label:<12}"), style_label()),
        Span::styled(format!(" {value}{cursor}"), style_value(selected)),
    ])
}

fn enum_line(app: &App, control: Control, label: &str, value: &str) -> Line<'static> {
    let selected = app.is_selected(control);
    Line::from(vec![
        Span::styled(prefix(selected), style_marker(selected)),
        Span::styled(format!("{label:<12}"), style_label()),
        Span::styled(format!(" < {value} >"), style_value(selected)),
    ])
}

fn number_line(app: &App, control: Control, label: &str, value: &str) -> Line<'static> {
    let selected = app.is_selected(control);
    Line::from(vec![
        Span::styled(prefix(selected), style_marker(selected)),
        Span::styled(format!("{label:<12}"), style_label()),
        Span::styled(format!(" - {value} +"), style_value(selected)),
    ])
}

fn toggle_line(app: &App, control: Control, label: &str, enabled: bool) -> Line<'static> {
    let selected = app.is_selected(control);
    let mark = if enabled { "[x]" } else { "[ ]" };
    Line::from(vec![
        Span::styled(prefix(selected), style_marker(selected)),
        Span::styled(format!("{mark} "), style_pill(selected)),
        Span::styled(label.to_string(), style_control(selected)),
    ])
}

fn section(title: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::styled("  -- ", style_rail()),
        Span::styled(title, style_accent()),
    ])
}

fn blank() -> Line<'static> {
    Line::raw("")
}

const fn prefix(selected: bool) -> &'static str {
    if selected { "> " } else { "  " }
}

fn build_panel_active(app: &App) -> bool {
    matches!(
        app.selected_control(),
        Control::Build
            | Control::BuildAndRun
            | Control::RunQemu
            | Control::Profile
            | Control::OutputDir
            | Control::Ramdisk(_)
    )
}

fn qemu_panel_active(app: &App) -> bool {
    matches!(
        app.selected_control(),
        Control::Ovmf
            | Control::Accel
            | Control::Cpu
            | Control::Machine
            | Control::Smp
            | Control::Memory
            | Control::Nic
            | Control::Nvme
            | Control::Xhci
            | Control::UsbKeyboard
            | Control::VirtioVga
            | Control::Display
            | Control::NoReboot
            | Control::NoShutdown
            | Control::GdbStub
            | Control::GdbWait
            | Control::QemuDebugLog
    )
}

fn selected_hint(app: &App) -> Line<'static> {
    let hint = match app.selected_control() {
        Control::Build => "b/Enter builds the EFI tree and ramdisk",
        Control::BuildAndRun => "B/Enter builds first, then starts QEMU automatically",
        Control::RunQemu => "r/Enter starts QEMU with the current launch profile",
        Control::Profile => "debug is faster; release matches optimized artifacts",
        Control::OutputDir => "Enter edits the EFI destination directory",
        Control::Ramdisk(_) => "Space toggles this userspace binary; a/n selects all/none",
        Control::Ovmf => "Enter edits the OVMF firmware path",
        Control::Accel => "kvm is fastest on Linux; tcg is the portable fallback",
        Control::Cpu => "host exposes the real CPU; max is a broad emulated model",
        Control::Machine => "q35 is the normal modern PCIe machine",
        Control::Smp => "Left/Right changes virtual CPU count by one",
        Control::Memory => "Left/Right changes guest memory in 64 MiB steps",
        Control::Nic => "adds an e1000e NIC on user networking",
        Control::Nvme => "adds a QEMU NVMe controller",
        Control::Xhci => "adds an xHCI USB controller",
        Control::UsbKeyboard => "adds a USB keyboard device",
        Control::VirtioVga => "adds virtio-vga for a more flexible framebuffer",
        Control::Display => "selects QEMU display backend; none is useful for serial-only runs",
        Control::NoReboot => "keeps crashes visible instead of instantly rebooting",
        Control::NoShutdown => "keeps QEMU open after guest shutdown",
        Control::GdbStub => "opens the QEMU GDB stub on tcp::1234",
        Control::GdbWait => "starts paused for debugger attach; enables GDB stub",
        Control::QemuDebugLog => "enables QEMU interrupt/reset/guest-error diagnostics",
    };

    Line::from(vec![
        Span::styled("focus ", style_faint()),
        Span::styled(hint, style_plain_value()),
    ])
}

fn deck_block(title: &'static str, active: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(if active {
            style_accent()
        } else {
            style_border()
        })
        .title(Span::styled(title, style_accent()))
        .style(style_panel())
}

fn style_title() -> Style {
    Style::default()
        .fg(Color::Rgb(238, 240, 244))
        .bg(Color::Rgb(7, 8, 11))
        .add_modifier(Modifier::BOLD)
}

fn style_accent() -> Style {
    Style::default()
        .fg(Color::Rgb(205, 42, 52))
        .bg(Color::Rgb(7, 8, 11))
        .add_modifier(Modifier::BOLD)
}

fn style_dim() -> Style {
    Style::default()
        .fg(Color::Rgb(142, 148, 158))
        .bg(Color::Rgb(7, 8, 11))
}

fn style_faint() -> Style {
    Style::default()
        .fg(Color::Rgb(74, 80, 90))
        .bg(Color::Rgb(7, 8, 11))
}

fn style_border() -> Style {
    Style::default()
        .fg(Color::Rgb(54, 59, 68))
        .bg(Color::Rgb(11, 13, 18))
}

fn style_label() -> Style {
    Style::default()
        .fg(Color::Rgb(95, 101, 112))
        .bg(Color::Rgb(11, 13, 18))
}

fn style_marker(selected: bool) -> Style {
    if selected {
        Style::default()
            .fg(Color::Rgb(230, 58, 70))
            .bg(Color::Rgb(11, 13, 18))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Rgb(49, 54, 62))
            .bg(Color::Rgb(11, 13, 18))
    }
}

fn style_control(selected: bool) -> Style {
    if selected {
        Style::default()
            .fg(Color::Rgb(246, 247, 249))
            .bg(Color::Rgb(11, 13, 18))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Rgb(174, 180, 190))
            .bg(Color::Rgb(11, 13, 18))
    }
}

fn style_value(selected: bool) -> Style {
    if selected {
        Style::default()
            .fg(Color::Rgb(252, 252, 252))
            .bg(Color::Rgb(71, 18, 24))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Rgb(226, 229, 234))
            .bg(Color::Rgb(11, 13, 18))
    }
}

fn style_plain_value() -> Style {
    Style::default()
        .fg(Color::Rgb(226, 229, 234))
        .bg(Color::Rgb(7, 8, 11))
}

fn log_style(line: &str) -> Style {
    if line.starts_with("[error]") || line.contains("failed") || line.contains("panic") {
        Style::default()
            .fg(Color::Rgb(230, 58, 70))
            .bg(Color::Rgb(11, 13, 18))
    } else if line.contains("complete") || line.starts_with("  ok") || line.starts_with("  ready") {
        Style::default()
            .fg(Color::Rgb(120, 206, 156))
            .bg(Color::Rgb(11, 13, 18))
    } else if line.starts_with('>') {
        Style::default()
            .fg(Color::Rgb(132, 188, 255))
            .bg(Color::Rgb(11, 13, 18))
            .add_modifier(Modifier::BOLD)
    } else if line.contains("warning") || line.contains("WARN") {
        Style::default()
            .fg(Color::Rgb(229, 177, 83))
            .bg(Color::Rgb(11, 13, 18))
    } else {
        Style::default()
            .fg(Color::Rgb(156, 162, 172))
            .bg(Color::Rgb(11, 13, 18))
    }
}

fn style_canvas() -> Style {
    Style::default()
        .fg(Color::Rgb(226, 229, 234))
        .bg(Color::Rgb(7, 8, 11))
}

fn style_panel() -> Style {
    Style::default()
        .fg(Color::Rgb(226, 229, 234))
        .bg(Color::Rgb(11, 13, 18))
}

fn style_rail() -> Style {
    Style::default()
        .fg(Color::Rgb(92, 20, 28))
        .bg(Color::Rgb(7, 8, 11))
}

fn style_pill(selected: bool) -> Style {
    if selected {
        Style::default()
            .fg(Color::Rgb(252, 252, 252))
            .bg(Color::Rgb(161, 29, 40))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Rgb(150, 156, 166))
            .bg(Color::Rgb(20, 23, 30))
    }
}
