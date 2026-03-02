//! Interactive build & developer tool for BeskarOS.

mod app;
mod config;
mod pipeline;
mod qemu;
mod ui;

use anyhow::Result;
use app::{App, FormActivateResult, Screen};
use clap::{Parser, Subcommand};
use config::{BuildConfig, QemuConfig};
use pipeline::{DevOp, discover_userspace_apps};
use ratatui::crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Parser)]
#[command(
    name = "imdr",
    about = "IMDR - Imperial Department of Military Research\nBeskarOS build & development tool.",
    long_about = None,
    version,
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Headless build: assemble a disk image with default configuration.
    Build {
        /// Output directory (default: efi_disk).
        #[arg(short, long, default_value = "efi_disk")]
        output: String,
        /// Build in release mode.
        #[arg(short, long)]
        release: bool,
        /// Userspace apps to include in the ramdisk (repeatable).
        #[arg(short, long)]
        app: Vec<String>,
    },

    /// Headless build + launch QEMU.
    Qemu {
        /// Output directory (default: efi_disk).
        #[arg(short, long, default_value = "efi_disk")]
        output: String,
        /// Build in release mode.
        #[arg(short, long)]
        release: bool,
        /// Number of CPU cores for QEMU.
        #[arg(long, default_value_t = 4)]
        cores: u32,
        /// RAM in MiB for QEMU.
        #[arg(long, default_value_t = 512)]
        ram: u32,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let workspace_root = find_workspace_root();

    match cli.command {
        Some(Commands::Build {
            output,
            release,
            app,
        }) => run_headless_build(workspace_root, output, release, app),
        Some(Commands::Qemu {
            output,
            release,
            cores,
            ram,
        }) => run_headless_qemu(workspace_root, output, release, cores, ram),
        None => run_tui(workspace_root),
    }
}

fn find_workspace_root() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn run_headless_build(
    workspace_root: PathBuf,
    output: String,
    release: bool,
    selected_apps: Vec<String>,
) -> Result<()> {
    let available = discover_userspace_apps(&workspace_root);
    let apps: Vec<(String, bool)> = available
        .into_iter()
        .map(|name| {
            let sel = if selected_apps.is_empty() {
                name == "bashkar"
            } else {
                selected_apps.contains(&name)
            };
            (name, sel)
        })
        .collect();

    let config = BuildConfig {
        output_dir: output,
        profile: if release {
            config::Profile::Release
        } else {
            config::Profile::Debug
        },
        userspace_apps: apps,
    };

    let (tx, rx) = std::sync::mpsc::channel();
    let handle = pipeline::start_build(config, workspace_root, tx);

    for msg in rx {
        match msg {
            pipeline::LogMsg::Line(s) => println!("{s}"),
            pipeline::LogMsg::Done(ok) => {
                let _ = handle.join();
                if ok {
                    println!("\nBuild complete.");
                    return Ok(());
                } else {
                    anyhow::bail!("Build failed.");
                }
            }
        }
    }

    Ok(())
}

fn run_headless_qemu(
    workspace_root: PathBuf,
    output: String,
    release: bool,
    cores: u32,
    ram: u32,
) -> Result<()> {
    run_headless_build(workspace_root.clone(), output.clone(), release, vec![])?;

    let qemu_cfg = QemuConfig {
        cores,
        ram_mib: ram,
        ..Default::default()
    };

    println!("\n{}", qemu::command_preview(&qemu_cfg, &output));
    println!("\nLaunching QEMU...");

    let (tx, rx) = std::sync::mpsc::channel();
    let handle = qemu::start_qemu(qemu_cfg, output, &workspace_root, tx);

    for msg in rx {
        match msg {
            pipeline::LogMsg::Line(s) => println!("{s}"),
            pipeline::LogMsg::Done(_) => {
                let _ = handle.join();
                return Ok(());
            }
        }
    }
    Ok(())
}

fn run_tui(workspace_root: PathBuf) -> Result<()> {
    let available_apps = discover_userspace_apps(&workspace_root);
    let build_cfg = BuildConfig::new(available_apps);
    let qemu_cfg = QemuConfig::default();
    let mut app = App::new(workspace_root.clone(), build_cfg, qemu_cfg);

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = tui_loop(&mut terminal, &mut app, &workspace_root);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn tui_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    workspace_root: &PathBuf,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, app))?;
        app.poll_logs();

        if event::poll(core::time::Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
            && key.kind == event::KeyEventKind::Press
        {
            let action = match &app.screen {
                Screen::MainMenu => {
                    handle_main_menu(app, key.code, key.modifiers);
                    TuiAction::None
                }
                Screen::BuildDossier => handle_build_form(app, key.code, key.modifiers),
                Screen::DevTools => handle_dev_tools(app, key.code, workspace_root),
            };
            match action {
                TuiAction::Build => trigger_build(app, workspace_root),
                TuiAction::LaunchQemu => launch_qemu_foreground(app, workspace_root, terminal)?,
                TuiAction::None => {}
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

enum TuiAction {
    None,
    Build,
    LaunchQemu,
}

fn handle_main_menu(app: &mut App, code: KeyCode, _mods: KeyModifiers) {
    match code {
        KeyCode::Up | KeyCode::Char('k') => app.main_menu_up(),
        KeyCode::Down | KeyCode::Char('j') => app.main_menu_down(),
        KeyCode::Enter => app.main_menu_select(),
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        _ => {}
    }
}

fn handle_build_form(app: &mut App, code: KeyCode, mods: KeyModifiers) -> TuiAction {
    if app.is_running() {
        match code {
            KeyCode::Up if app.build_form.log_scroll != usize::MAX => {
                app.build_form.log_scroll = app.build_form.log_scroll.saturating_sub(1);
            }

            KeyCode::Down => {
                app.build_form.log_scroll = app
                    .build_form
                    .log_scroll
                    .saturating_add(1)
                    .min(app.log_lines.len());
            }
            _ => {}
        }
        return TuiAction::None;
    }

    let field = app.build_form.field_at(app.build_form.selected);

    match code {
        KeyCode::Tab | KeyCode::Down => {
            if !mods.contains(KeyModifiers::SHIFT) {
                app.form_next();
            } else {
                app.form_prev();
            }
        }
        KeyCode::BackTab | KeyCode::Up => app.form_prev(),
        KeyCode::Left => app.form_cycle_left(),
        KeyCode::Right => app.form_cycle_right(),
        KeyCode::Enter | KeyCode::Char(' ') if field.is_checkbox() => {
            app.form_activate();
        }
        KeyCode::Enter if field.is_action() => {
            let result = app.form_activate();
            return match result {
                FormActivateResult::TriggerBuild => TuiAction::Build,
                FormActivateResult::TriggerQemu => TuiAction::LaunchQemu,
                FormActivateResult::None => TuiAction::None,
            };
        }
        KeyCode::Backspace if field.is_text() => app.form_backspace(),
        KeyCode::Char(c) if field.is_text() => app.form_type_char(c),
        KeyCode::Esc => {
            app.screen = Screen::MainMenu;
        }
        _ => {}
    }

    TuiAction::None
}

fn trigger_build(app: &mut App, workspace_root: &Path) {
    let cfg = app.build_form.to_build_config();
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = pipeline::start_build(cfg, workspace_root.to_path_buf(), tx);
    app.set_running(rx, handle);
}

/// Suspends the TUI, runs QEMU with an inherited terminal, then restores the TUI.
fn launch_qemu_foreground(
    app: &mut App,
    workspace_root: &PathBuf,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<()> {
    let qcfg = app.build_form.to_qemu_config();
    let output_dir = app.build_form.output_dir.clone();

    // Hand the terminal over to QEMU.
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    let args = qemu::build_args(&qcfg, &output_dir);
    let flat_args: Vec<String> = args
        .iter()
        .flat_map(|a| a.splitn(2, ' ').map(str::to_owned).collect::<Vec<_>>())
        .collect();

    let status = Command::new("qemu-system-x86_64")
        .args(&flat_args)
        .current_dir(workspace_root)
        .status();

    // Restore TUI.
    enable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        EnterAlternateScreen,
        EnableMouseCapture
    )?;
    terminal.clear()?;

    let ok = status.map(|s| s.success()).unwrap_or(false);
    let msg = if ok {
        "  ✓ QEMU exited.".to_string()
    } else {
        "  QEMU exited with non-zero status.".to_string()
    };
    app.log_lines.push(msg);
    app.last_op_success = Some(ok);

    Ok(())
}

fn handle_dev_tools(app: &mut App, code: KeyCode, workspace_root: &Path) -> TuiAction {
    if app.pkg_field_focused {
        match code {
            KeyCode::Char(c) => app.dev_type_char(c),
            KeyCode::Backspace => app.dev_backspace(),
            KeyCode::Enter | KeyCode::Esc => app.pkg_field_focused = false,
            _ => {}
        }
        return TuiAction::None;
    }

    if app.is_running() {
        match code {
            KeyCode::Up if app.dev_form.log_scroll != usize::MAX => {
                app.dev_form.log_scroll = app.dev_form.log_scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                app.dev_form.log_scroll = app
                    .dev_form
                    .log_scroll
                    .saturating_add(1)
                    .min(app.log_lines.len());
            }
            _ => {}
        }
        return TuiAction::None;
    }

    match code {
        KeyCode::Up | KeyCode::Char('k') => app.dev_op_up(),
        KeyCode::Down | KeyCode::Char('j') => app.dev_op_down(),
        KeyCode::Char('p') => app.pkg_field_focused = true,
        KeyCode::Enter => {
            let op = DevOp::ALL[app.dev_form.op_selected % DevOp::ALL.len()].clone();
            if op == pipeline::DevOp::LaunchQemu {
                return TuiAction::LaunchQemu;
            }
            let pkg = app.dev_form.package.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            let handle = pipeline::start_dev_op(op, pkg, workspace_root.to_path_buf(), tx);
            app.set_running(rx, handle);
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.screen = Screen::MainMenu;
        }
        _ => {}
    }

    TuiAction::None
}
