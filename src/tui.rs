// src/tui.rs
use crate::error::ChelpError;
use crate::models::{AiCommandResponse, SafetyLevel};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use std::io::IsTerminal;

#[derive(Debug, PartialEq, Eq)]
pub enum UserAction {
    Run(String),
    Edit(String),
    Cancel,
}

/// Pure key -> decision mapping so tests can drive it without a terminal.
///
/// Destructive commands are gated (spec §5.4): the first `Enter` only arms the
/// confirmation and returns `None` (the caller redraws with the prompt), `y`
/// then executes. `Tab`/`e` always routes back to the prompt for review.
pub fn handle_key_event(
    key: crossterm::event::KeyEvent,
    command: &str,
    destructive: bool,
    armed: &mut bool,
) -> Option<UserAction> {
    match key.code {
        KeyCode::Enter if destructive && !*armed => {
            *armed = true;
            None
        }
        KeyCode::Enter => Some(UserAction::Run(command.to_string())),
        KeyCode::Char('y') | KeyCode::Char('Y') if destructive && *armed => {
            Some(UserAction::Run(command.to_string()))
        }
        KeyCode::Tab | KeyCode::Char('e') => Some(UserAction::Edit(command.to_string())),
        KeyCode::Esc | KeyCode::Char('q') => Some(UserAction::Cancel),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(UserAction::Cancel)
        }
        _ => None,
    }
}

pub fn draw_confirmation_ui<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    resp: &AiCommandResponse,
    armed: bool,
) -> Result<(), ChelpError> {
    terminal.draw(|f| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Command
                Constraint::Length(3), // Explanation
                Constraint::Length(3), // Safety & Warning
                Constraint::Length(3), // Controls
            ])
            .split(f.size());

        let cmd_p = Paragraph::new(resp.command.clone())
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Suggested Command "),
            );
        f.render_widget(cmd_p, chunks[0]);

        let exp_p = Paragraph::new(resp.explanation.clone())
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Explanation "),
            );
        f.render_widget(exp_p, chunks[1]);

        let destructive = resp.safety_level == SafetyLevel::Destructive;
        let (safety_text, safety_style) = match resp.safety_level {
            SafetyLevel::Safe => ("● SAFE (Read-Only)", Style::default().fg(Color::Green)),
            SafetyLevel::Caution => (
                "● CAUTION (State Change)",
                Style::default().fg(Color::Yellow),
            ),
            SafetyLevel::Destructive => (
                "▲ HIGH RISK / DESTRUCTIVE ACTION",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        };
        let warning_line = match &resp.destructive_warning {
            Some(w) => format!(" - {}", w),
            None => String::new(),
        };

        let safety_p = Paragraph::new(Line::from(vec![
            Span::styled(safety_text, safety_style),
            Span::raw(warning_line),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Safety Assessment "),
        );
        f.render_widget(safety_p, chunks[2]);

        let controls = if destructive {
            if armed {
                Line::from(vec![
                    Span::styled("[y] ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw("Confirm   "),
                    Span::styled("[Tab / e] ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw("Edit on prompt   "),
                    Span::styled("[Esc] ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw("Cancel"),
                ])
            } else {
                Line::from(vec![
                    Span::styled("[Enter] ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw("Confirm   "),
                    Span::styled("[Tab / e] ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw("Edit on prompt   "),
                    Span::styled("[Esc] ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw("Cancel"),
                ])
            }
        } else {
            Line::from(vec![
                Span::styled("[Enter] ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("Run in shell   "),
                Span::styled("[Tab / e] ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("Edit on prompt   "),
                Span::styled("[Esc] ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("Cancel"),
            ])
        };
        let ctrl_p = Paragraph::new(controls)
            .block(Block::default().borders(Borders::ALL).title(" Actions "));
        f.render_widget(ctrl_p, chunks[3]);
    })?;
    Ok(())
}

pub fn render_interactive_confirmation_with<B: ratatui::backend::Backend, F>(
    terminal: &mut Terminal<B>,
    mut next_event: F,
    resp: &AiCommandResponse,
) -> Result<UserAction, ChelpError>
where
    F: FnMut() -> Result<Event, ChelpError>,
{
    let mut armed = false;
    loop {
        draw_confirmation_ui(terminal, resp, armed)?;
        let evt = next_event()?;
        if let Event::Key(key) = evt {
            let destructive = resp.safety_level == SafetyLevel::Destructive;
            if let Some(action) = handle_key_event(key, &resp.command, destructive, &mut armed) {
                return Ok(action);
            }
        }
    }
}

/// Renders the confirmation modal. The drawing surface is stdout; key input
/// comes from the controlling terminal — crossterm falls back to `/dev/tty`
/// when stdin is not a terminal, which is exactly the case inside a zsh ZLE
/// widget (children get a stdin that is not the tty). Without a drawable or
/// readable terminal there is nothing to show, so the command is handed back
/// as an edit instead of failing.
pub fn render_interactive_confirmation(resp: &AiCommandResponse) -> Result<UserAction, ChelpError> {
    if !std::io::stdout().is_terminal() {
        return Ok(UserAction::Edit(resp.command.clone()));
    }
    if enable_raw_mode().is_err() || execute!(std::io::stdout(), EnterAlternateScreen).is_err() {
        let _ = disable_raw_mode();
        return Ok(UserAction::Edit(resp.command.clone()));
    }

    let mut terminal = match Terminal::new(CrosstermBackend::new(std::io::stdout())) {
        Ok(terminal) => terminal,
        Err(e) => {
            let _ = disable_raw_mode();
            let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
            return Err(ChelpError::from(e));
        }
    };

    let action = render_interactive_confirmation_with(
        &mut terminal,
        || event::read().map_err(ChelpError::from),
        resp,
    );

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    action
}
