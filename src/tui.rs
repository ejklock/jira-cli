use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    widgets::{Block, BorderType, Borders, Paragraph},
    Terminal,
};
use std::io::{self, Write};

use crate::i18n::t;
use crate::store::instances::Instance;

const TTY_ERROR_KEY: &str = "Error: 'browse' requires an interactive terminal (TTY).";

/// Entry point for `jira browse`.
///
/// Checks the TTY guard, then enters the raw-mode draw loop. Returns 0 on
/// clean quit (`q` or Ctrl+C) and 1 on the non-TTY guard path.
pub async fn browse(instance: &Instance, is_tty: bool, stderr: &mut impl Write) -> i32 {
    use crate::cli::{browse_tty_action, BrowseAction};

    match browse_tty_action(is_tty) {
        BrowseAction::TtyError => {
            writeln!(stderr, "{}", t(TTY_ERROR_KEY)).ok();
            1
        }
        BrowseAction::RunTui => run_tui(instance),
    }
}

fn run_tui(_instance: &Instance) -> i32 {
    let mut stdout = io::stdout();
    if enable_raw_mode().is_err() {
        return 1;
    }
    if execute!(stdout, EnterAlternateScreen).is_err() {
        let _ = disable_raw_mode();
        return 1;
    }

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = match Terminal::new(backend) {
        Ok(t) => t,
        Err(_) => {
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
            let _ = disable_raw_mode();
            return 1;
        }
    };

    let exit_code = draw_loop(&mut terminal);

    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = disable_raw_mode();
    exit_code
}

fn draw_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> i32 {
    loop {
        let _ = terminal.draw(render_placeholder);

        match event::read() {
            Ok(Event::Key(key)) => {
                let quit = key.code == KeyCode::Char('q')
                    || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL));
                if quit {
                    return 0;
                }
            }
            Err(_) => return 1,
            Ok(_) => {}
        }
    }
}

fn render_placeholder(frame: &mut ratatui::Frame) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let block = Block::default()
        .title(" browse ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    frame.render_widget(block, chunks[0]);

    let hint = Paragraph::new(t("quit") + ": q").alignment(Alignment::Center);
    frame.render_widget(hint, chunks[1]);
}

#[cfg(test)]
#[path = "../tests/unit/tui.rs"]
mod tests;
