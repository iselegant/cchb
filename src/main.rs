mod app;
mod color;
mod event;
mod filter;
mod markdown;
mod session;
mod ui;

use anyhow::{Context, Result};
use app::AppState;
use color::Theme;
use crossterm::ExecutableCommand;
use crossterm::event::{Event, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::prelude::CrosstermBackend;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn get_claude_dir() -> Result<PathBuf> {
    let home = directories::BaseDirs::new().context("Failed to determine home directory")?;
    let claude_dir = home.home_dir().join(".claude");
    if !claude_dir.exists() {
        anyhow::bail!(
            "Claude Code data directory not found: {}\nMake sure Claude Code has been used at least once.",
            claude_dir.display()
        );
    }
    Ok(claude_dir)
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    terminal::enable_raw_mode().context("Failed to enable raw mode")?;
    io::stdout()
        .execute(EnterAlternateScreen)
        .context("Failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend).context("Failed to create terminal")?;
    Ok(terminal)
}

fn restore_terminal() {
    let _ = terminal::disable_raw_mode();
    let _ = io::stdout().execute(LeaveAlternateScreen);
}

/// Load conversation for the currently focused session if not already loaded.
fn maybe_load_focused_conversation(app: &mut AppState) {
    let current_session_id = app.selected_session().map(|s| s.session_id.clone());

    if current_session_id == app.loaded_session_id {
        return; // already loaded
    }

    if let Some(sess) = app.selected_session() {
        let path = sess.file_path.clone();
        if let Ok(messages) = session::load_conversation(&path) {
            let display = session::display_messages(messages);
            app.conversation = display;
        } else {
            app.conversation.clear();
        }
        app.loaded_session_id = current_session_id;
        app.conversation_scroll = 0;
    } else {
        app.conversation.clear();
        app.loaded_session_id = None;
    }
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut AppState,
    theme: &Theme,
    claude_dir: &Path,
) -> Result<()> {
    // Load initial conversation
    maybe_load_focused_conversation(app);

    loop {
        terminal.draw(|frame| {
            ui::render(frame, app, theme);
        })?;

        // Resolve pending cross-session search jumps after render populates match positions.
        while app.resolve_pending_search_jump() {
            maybe_load_focused_conversation(app);
            terminal.draw(|frame| {
                ui::render(frame, app, theme);
            })?;
        }

        // Poll for background search cache completion.
        app.poll_search_cache();

        // Auto-dismiss reload indicator after timeout.
        app.check_reload_expired();

        // Use shorter poll interval during logo sparkle animation for smooth color cycling.
        let poll_ms = if app.logo_sparkle_start.is_some() {
            50
        } else {
            250
        };
        if crossterm::event::poll(Duration::from_millis(poll_ms))?
            && let Event::Key(key) = crossterm::event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            // Handle reload (R key in normal mode)
            if app.mode == app::AppMode::Normal && key.code == crossterm::event::KeyCode::Char('R')
            {
                if let Ok(sessions) = session::discover_sessions(claude_dir) {
                    let indices: Vec<usize> = (0..sessions.len()).collect();
                    app.sessions = sessions;
                    app.filtered_indices = indices;
                    app.selected_index = 0;
                    app.loaded_session_id = None;
                    app.invalidate_search_content_cache();
                }
                continue;
            }

            event::handle_key(app, key)?;

            // Auto-load conversation when focus changes
            maybe_load_focused_conversation(app);

            if app.should_quit {
                break;
            }
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    // Set up panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        restore_terminal();
        original_hook(panic_info);
    }));

    let claude_dir = get_claude_dir()?;
    let sessions = session::discover_sessions(&claude_dir)?;

    if sessions.is_empty() {
        println!("No Claude Code sessions found.");
        return Ok(());
    }

    println!("Found {} sessions. Loading...", sessions.len());

    let mut app = AppState::new(sessions);
    let theme = Theme::default_theme();

    let mut terminal = setup_terminal()?;
    let result = run_app(&mut terminal, &mut app, &theme, &claude_dir);

    restore_terminal();
    result?;

    // If user requested session resume, launch claude --resume from the project directory
    if let Some(session_id) = &app.resume_session_id {
        let mut cmd = std::process::Command::new("claude");
        cmd.args(["--resume", session_id]);

        if let Some(ref project_path) = app.resume_project_path {
            let path = std::path::Path::new(project_path);
            if !path.is_dir() {
                std::fs::create_dir_all(path).ok();
            }
            if path.is_dir() {
                cmd.current_dir(path);
            }
        }

        let status = cmd
            .status()
            .context("Failed to launch claude. Is it installed and in your PATH?")?;

        if !status.success() {
            anyhow::bail!("claude exited with status: {status}");
        }
    }

    Ok(())
}
