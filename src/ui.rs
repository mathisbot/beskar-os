//! IMDR TUI rendering.
use crate::{
    app::{App, MAIN_ITEMS, Screen},
    config::{AccelBackend, CpuType, DisplayBackend, Profile},
    pipeline::DevOp,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

fn s_normal() -> Style {
    Style::default().fg(Color::Gray)
}
fn s_dim() -> Style {
    Style::default().fg(Color::DarkGray)
}
fn s_selected() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}
fn s_section() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}
fn s_border() -> Style {
    Style::default().fg(Color::DarkGray)
}
fn s_border_active() -> Style {
    Style::default().fg(Color::Yellow)
}
fn s_ok() -> Style {
    Style::default().fg(Color::Green)
}
fn s_err() -> Style {
    Style::default().fg(Color::Red)
}
fn s_warn() -> Style {
    Style::default().fg(Color::Yellow)
}
fn s_button_action(selected: bool) -> Style {
    if selected {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let [header_area, content_area, hint_area] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);

    render_header(frame, header_area);

    match app.screen {
        Screen::MainMenu => {
            render_main_menu(frame, app, content_area);
            render_hint(
                frame,
                "  ↑ ↓  Navigate    Enter  Select    q  Quit",
                hint_area,
            );
        }
        Screen::BuildDossier => {
            render_build_dossier(frame, app, content_area);
            let hint = if app.is_running() {
                "  Operation in progress…    q/Esc  to return after completion"
            } else {
                "  ↑ ↓ / Tab  Navigate    ← →  Cycle    Space / Enter  Toggle / Activate    Esc  Back"
            };
            render_hint(frame, hint, hint_area);
        }
        Screen::DevTools => {
            render_dev_tools(frame, app, content_area);
            let hint = if app.is_running() {
                "  Operation in progress…"
            } else if app.pkg_field_focused {
                "  Type package name    Enter  Confirm    Esc  Cancel"
            } else {
                "  ↑ ↓  Navigate    Enter  Run    p  Edit Package    Esc  Back"
            };
            render_hint(frame, hint, hint_area);
        }
    }
}

const BANNER: &str = concat!(
    " ██╗██████╗ ███╗  ███╗█████╗\n",
    " ██║██╔══██╗████╗████║██▄▄▀▀\n",
    " ██║██████╔╝██╔███ ██║██║ ██╗\n",
    " ╚═╝╚═════╝ ╚═╝    ╚═╝╚═╝ ╚═╝ ",
);

fn render_header(frame: &mut Frame, area: Rect) {
    // Split horizontally: small ASCII branding left, classification info right.
    let [left, right] =
        Layout::horizontal([Constraint::Length(33), Constraint::Min(0)]).areas(area);

    let banner = Paragraph::new(BANNER).style(s_section());
    frame.render_widget(banner, left);

    let info = Text::from(vec![
        Line::from(vec![Span::styled(
            "  Imperial Department of Military Research",
            s_section(),
        )]),
        Line::from(vec![Span::styled(
            "  ─────────────────────────────────────────",
            s_dim(),
        )]),
        Line::from(vec![
            Span::styled("  Classification  ", s_dim()),
            Span::styled("RESTRICTED", s_err().add_modifier(Modifier::BOLD)),
            Span::styled("  ·  BeskarOS Research Terminal", s_dim()),
        ]),
        Line::from(vec![Span::styled(
            "  Access granted. Select research operation.",
            s_dim(),
        )]),
    ]);
    frame.render_widget(Paragraph::new(info), right);
}

fn render_hint(frame: &mut Frame, text: &str, area: Rect) {
    let p = Paragraph::new(text).style(s_dim());
    frame.render_widget(p, area);
}

fn render_main_menu(frame: &mut Frame, app: &App, area: Rect) {
    let [list_area, desc_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).areas(area);

    // Build list items.
    let items: Vec<ListItem> = MAIN_ITEMS
        .iter()
        .enumerate()
        .map(|(i, (label, _))| {
            let prefix = if i == app.main_sel { "  ▸ " } else { "    " };
            let style = if i == app.main_sel {
                s_selected()
            } else {
                s_normal()
            };
            ListItem::new(Line::from(vec![Span::styled(
                format!("{prefix}{label}"),
                style,
            )]))
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(app.main_sel));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(s_border())
        .title(Span::styled(" Operations ", s_section()));

    frame.render_stateful_widget(List::new(items).block(block), list_area, &mut state);

    // Description of selected item.
    let desc = MAIN_ITEMS.get(app.main_sel).map_or("", |(_, d)| *d);
    let desc_block = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
        .border_style(s_border());
    let desc_para = Paragraph::new(format!("  {desc}"))
        .block(desc_block)
        .style(s_dim())
        .wrap(Wrap { trim: false });
    frame.render_widget(desc_para, desc_area);
}

fn render_build_dossier(frame: &mut Frame, app: &App, area: Rect) {
    let [form_area, log_area] =
        Layout::horizontal([Constraint::Percentage(52), Constraint::Percentage(48)]).areas(area);

    render_build_form(frame, app, form_area);
    render_log_panel(
        frame,
        app,
        log_area,
        " Operation Log ",
        app.build_form.log_scroll,
    );
}

fn render_build_form(frame: &mut Frame, app: &App, area: Rect) {
    let form = &app.build_form;
    let selected = form.selected;
    let n = form.apps.len();

    // Helper closures.
    let field_style = |idx: usize| -> Style {
        if idx == selected {
            s_selected()
        } else {
            s_normal()
        }
    };

    let text_field = |label: &str, value: &str, idx: usize| -> Line<'static> {
        let cursor = if idx == selected { "█" } else { " " };
        let lbl = format!("  {label:<14}");
        let val = format!(" {value}{cursor}");
        Line::from(vec![
            Span::styled(lbl, s_dim()),
            Span::styled(format!("[{val:<22}]"), field_style(idx)),
        ])
    };

    let selector_field =
        |label: &str, options: &[&str], current: usize, idx: usize| -> Line<'static> {
            let lbl = format!("  {label:<14}");
            let parts: Vec<Span> = std::iter::once(Span::styled(lbl, s_dim()))
                .chain(options.iter().enumerate().map(|(i, opt)| {
                    let s = if i == current {
                        if idx == selected {
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Gray)
                                .add_modifier(Modifier::BOLD)
                        }
                    } else {
                        s_dim()
                    };
                    Span::styled(format!(" {opt} "), s)
                }))
                .collect();
            Line::from(parts)
        };

    let checkbox = |label: &str, checked: bool, idx: usize| -> Line<'static> {
        let mark = if checked {
            Span::styled("[✓]", s_ok())
        } else {
            Span::styled("[ ]", s_dim())
        };
        let lbl = if idx == selected {
            Span::styled(format!("  {label}"), s_selected())
        } else {
            Span::styled(format!("  {label}"), s_normal())
        };
        Line::from(vec![Span::raw("  "), mark, Span::raw(" "), lbl])
    };

    let section = |title: &str| -> Line<'static> {
        Line::from(vec![Span::styled(format!("  ── {title} "), s_section())])
    };

    let spacer = || -> Line<'static> { Line::raw("") };

    // Build lines.
    let mut lines: Vec<Line> = vec![
        spacer(),
        section("Deployment"),
        text_field("Output Dir", &form.output_dir, 0),
        selector_field(
            "Profile",
            &Profile::ALL.iter().map(Profile::label).collect::<Vec<_>>(),
            form.profile_idx,
            1,
        ),
        spacer(),
        section("Ramdisk Payload"),
    ];

    for (i, (app_name, sel)) in form.apps.iter().enumerate() {
        let idx = 2 + i;
        lines.push(checkbox(app_name, *sel, idx));
    }

    lines.push(spacer());
    lines.push(section("QEMU Configuration"));
    lines.push(text_field("OVMF", &form.ovmf, 2 + n));
    lines.push(text_field("Cores", &form.cores, 3 + n));
    lines.push(text_field("RAM (MiB)", &form.ram, 4 + n));
    lines.push(selector_field(
        "CPU",
        &CpuType::ALL.iter().map(CpuType::as_str).collect::<Vec<_>>(),
        form.cpu_idx,
        5 + n,
    ));
    lines.push(selector_field(
        "Accel",
        &AccelBackend::ALL
            .iter()
            .map(AccelBackend::as_str)
            .collect::<Vec<_>>(),
        form.accel_idx,
        6 + n,
    ));

    lines.push(checkbox("NIC (e1000e)", form.nic, 7 + n));
    lines.push(checkbox("NVMe", form.nvme, 8 + n));
    lines.push(checkbox("XHCI controller", form.xhci, 9 + n));
    lines.push(checkbox("virtio-vga", form.virtio_vga, 10 + n));

    // Display is only meaningful with virtio-vga.
    let display_opts = {
        let mut opts = vec!["default"];
        opts.extend(DisplayBackend::ALL.iter().map(DisplayBackend::as_str));
        opts
    };
    let disp_style = if form.virtio_vga {
        field_style(11 + n)
    } else {
        s_dim()
    };
    let disp_line = {
        let lbl = "  Display      ";
        let parts: Vec<Span> = std::iter::once(Span::styled(lbl, s_dim()))
            .chain(display_opts.iter().enumerate().map(|(i, opt)| {
                let s = if i == form.display_idx {
                    if 11 + n == selected {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Gray)
                            .add_modifier(Modifier::BOLD)
                    }
                } else {
                    disp_style
                };
                Span::styled(format!(" {opt} "), s)
            }))
            .collect();
        Line::from(parts)
    };
    lines.push(disp_line);

    lines.push(spacer());

    // Action buttons.
    let build_sel = 12 + n == selected;
    let qemu_sel = 13 + n == selected;
    let disabled = app.is_running();

    let build_label = if disabled { " BUILDING… " } else { " BUILD " };
    let qemu_label = if disabled { " QEMU… " } else { " RUN QEMU " };

    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(build_label, s_button_action(!disabled && build_sel)),
        Span::raw("   "),
        Span::styled(qemu_label, s_button_action(!disabled && qemu_sel)),
    ]));

    // Status indicator from last run.
    if !app.is_running()
        && let Some(ok) = app.last_op_success
    {
        let (msg, style) = if ok {
            ("  ✓ Operation completed successfully.", s_ok())
        } else {
            ("  ✗ Operation failed. Check the log.", s_err())
        };
        lines.push(spacer());
        lines.push(Line::from(Span::styled(msg, style)));
    }

    // Scroll.
    let inner_height = area.height.saturating_sub(2) as usize;
    let scroll = {
        let max_scroll = lines.len().saturating_sub(inner_height);
        // Auto-scroll to bring selected field into view.
        let approx_selected_y = selected + 8; // rough offset accounting for section headers
        let scroll = if approx_selected_y > form.scroll + inner_height {
            approx_selected_y.saturating_sub(inner_height)
        } else {
            form.scroll
        };
        scroll.min(max_scroll)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(s_border_active())
        .title(Span::styled(" Research Configuration ", s_section()));

    let para = Paragraph::new(Text::from(lines))
        .block(block)
        .scroll((scroll as u16, 0));
    frame.render_widget(para, area);
}

fn render_log_panel(frame: &mut Frame, app: &App, area: Rect, title: &str, log_scroll: usize) {
    let lines: Vec<Line> = app
        .log_lines
        .iter()
        .map(|l| {
            let style = if l.starts_with("[error]") || l.starts_with("[FAILED]") {
                s_err()
            } else if l.starts_with("  ✓") || l.contains("OK") {
                s_ok()
            } else if l.starts_with("»") {
                Style::default().fg(Color::Cyan)
            } else if l.starts_with("WARN") || l.starts_with("⚠") {
                s_warn()
            } else {
                s_dim()
            };
            Line::from(Span::styled(l.clone(), style))
        })
        .collect();

    let total = lines.len();
    let inner_h = area.height.saturating_sub(2) as usize;

    let scroll = if log_scroll == usize::MAX || log_scroll >= total {
        total.saturating_sub(inner_h)
    } else {
        log_scroll.min(total.saturating_sub(inner_h))
    };

    let status_suffix = if app.is_running() {
        " running "
    } else {
        match app.last_op_success {
            Some(true) => " done ✓ ",
            Some(false) => " failed ✗ ",
            None => " ready ",
        }
    };

    let title_line = format!("{title}{status_suffix}");
    let title_style = match app.last_op_success {
        Some(true) if !app.is_running() => s_ok(),
        Some(false) if !app.is_running() => s_err(),
        _ => s_dim(),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(s_border())
        .title(Span::styled(title_line, title_style));

    let para = Paragraph::new(Text::from(lines))
        .block(block)
        .scroll((scroll as u16, 0));
    frame.render_widget(para, area);
}

fn render_dev_tools(frame: &mut Frame, app: &App, area: Rect) {
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).areas(area);

    render_dev_form(frame, app, left);
    render_log_panel(frame, app, right, " Output ", app.dev_form.log_scroll);
}

fn render_dev_form(frame: &mut Frame, app: &App, area: Rect) {
    let form = &app.dev_form;
    let pkg_cursor = if app.pkg_field_focused { "█" } else { " " };
    let pkg_style = if app.pkg_field_focused {
        s_selected()
    } else {
        s_normal()
    };

    let mut lines: Vec<Line> = vec![
        Line::raw(""),
        Line::from(vec![Span::styled("  ── Package ", s_section())]),
        Line::from(vec![
            Span::styled("  Target       ", s_dim()),
            Span::styled(
                format!("[{:<21}]", format!("{}{}", form.package, pkg_cursor)),
                pkg_style,
            ),
        ]),
        Line::raw(""),
        Line::from(vec![Span::styled("  ── Operations ", s_section())]),
    ];

    for (i, op) in DevOp::ALL.iter().enumerate() {
        let sel = i == form.op_selected && !app.pkg_field_focused;
        let prefix = if sel { "  ▸ " } else { "    " };
        let style = if sel { s_selected() } else { s_normal() };
        lines.push(Line::from(Span::styled(
            format!("{prefix}{}", op.label()),
            style,
        )));
    }

    if app.is_running() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            "  Running…",
            Style::default().fg(Color::Cyan),
        )));
    } else if let Some(ok) = app.last_op_success {
        lines.push(Line::raw(""));
        let (msg, style) = if ok {
            ("  ✓ Completed successfully.", s_ok())
        } else {
            ("  ✗ Operation failed.", s_err())
        };
        lines.push(Line::from(Span::styled(msg, style)));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(s_border_active())
        .title(Span::styled(" Development Tools ", s_section()));

    frame.render_widget(Paragraph::new(Text::from(lines)).block(block), area);
}
