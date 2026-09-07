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
use std::io::stdout;

#[derive(Debug, PartialEq, Eq)]
pub enum UserAction {
    Run(String),
    Edit(String),
    Cancel,
}

pub fn render_interactive_confirmation(resp: &AiCommandResponse) -> Result<UserAction, ChelpError> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let action = loop {
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

            // 1. Command Block
            let cmd_p = Paragraph::new(resp.command.clone())
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .block(Block::default().borders(Borders::ALL).title(" Suggested Command "));
            f.render_widget(cmd_p, chunks[0]);

            // 2. Explanation Block
            let exp_p = Paragraph::new(resp.explanation.clone())
                .style(Style::default().fg(Color::White))
                .wrap(Wrap { trim: true })
                .block(Block::default().borders(Borders::ALL).title(" Explanation "));
            f.render_widget(exp_p, chunks[1]);

            // 3. Safety Block
            let (safety_text, safety_style) = match resp.safety_level {
                SafetyLevel::Safe => ("● SAFE (Read-Only)", Style::default().fg(Color::Green)),
                SafetyLevel::Caution => ("● CAUTION (State Change)", Style::default().fg(Color::Yellow)),
                SafetyLevel::Destructive => ("▲ HIGH RISK / DESTRUCTIVE ACTION", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            };

            let warning_line = if let Some(w) = &resp.destructive_warning {
                format!(" - {}", w)
            } else {
                String::new()
            };

            let safety_p = Paragraph::new(Line::from(vec![
                Span::styled(safety_text, safety_style),
                Span::raw(warning_line),
            ]))
            .block(Block::default().borders(Borders::ALL).title(" Safety Assessment "));
            f.render_widget(safety_p, chunks[2]);

            // 4. Controls Block
            let controls = Line::from(vec![
                Span::styled("[Enter] ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("Run in shell   "),
                Span::styled("[Tab / e] ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("Edit on prompt   "),
                Span::styled("[Esc] ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("Cancel"),
            ]);
            let ctrl_p = Paragraph::new(controls)
                .block(Block::default().borders(Borders::ALL).title(" Actions "));
            f.render_widget(ctrl_p, chunks[3]);
        })?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Enter => break UserAction::Run(resp.command.clone()),
                KeyCode::Tab | KeyCode::Char('e') => break UserAction::Edit(resp.command.clone()),
                KeyCode::Esc | KeyCode::Char('q') => break UserAction::Cancel,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break UserAction::Cancel,
                _ => {}
            }
        }
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(action)
}
