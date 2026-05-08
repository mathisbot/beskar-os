//! `BeskarOS` command deck.

mod app;
mod config;
mod pipeline;
mod qemu;
mod ui;

use anyhow::Result;
use app::{Action, App};
use config::{BuildConfig, QemuConfig};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    crossterm::{
        event::{self, Event},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
};
use std::{io, path::PathBuf, time::Duration};

fn main() -> Result<()> {
    let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let build = BuildConfig::new(pipeline::discover_userspace_apps(&workspace_root));
    let qemu = QemuConfig::default();
    let mut app = App::new(workspace_root, build, qemu);

    let mut stdout = io::stdout();
    let mut restore = TerminalRestore::enter(&mut stdout)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_tui(&mut terminal, &mut app);
    restore.leave(&mut terminal)?;

    result
}

fn run_tui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::render(frame, app))?;

        if let Some(action) = app.poll_task() {
            dispatch_action(app, action);
        }

        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
            && key.kind == event::KeyEventKind::Press
        {
            let action = app.handle_key(key.code, key.modifiers);
            dispatch_action(app, action);
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn dispatch_action(app: &mut App, action: Action) {
    if app.is_running() {
        return;
    }

    match action {
        Action::None => {}
        Action::Build => start_build(app, false),
        Action::BuildAndRun => start_build(app, true),
        Action::RunQemu => {
            let task =
                qemu::start_qemu(&app.qemu, &app.build.output_dir, app.workspace_root.clone());
            app.set_running(task, None);
        }
    }
}

fn start_build(app: &mut App, run_afterwards: bool) {
    let task = pipeline::start_build(app.build.clone(), app.workspace_root.clone());
    let then = run_afterwards.then_some(Action::RunQemu);
    app.set_running(task, then);
}

struct TerminalRestore {
    armed: bool,
}

impl TerminalRestore {
    fn enter(stdout: &mut io::Stdout) -> Result<Self> {
        enable_raw_mode()?;
        let restore = Self { armed: true };
        execute!(stdout, EnterAlternateScreen)?;
        Ok(restore)
    }

    fn leave(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        self.armed = false;
        Ok(())
    }
}

impl Drop for TerminalRestore {
    fn drop(&mut self) {
        if self.armed {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
        }
    }
}
