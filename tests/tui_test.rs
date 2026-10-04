// tests/tui_test.rs
use chelp::error::ChelpError;
use chelp::models::{AiCommandResponse, SafetyLevel};
use chelp::tui::{
    draw_confirmation_ui, handle_key_event, render_interactive_confirmation_with, UserAction,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::collections::VecDeque;

fn safe_response(command: &str) -> AiCommandResponse {
    AiCommandResponse {
        command: command.to_string(),
        explanation: "Explains the command".to_string(),
        safety_level: SafetyLevel::Safe,
        destructive_warning: None,
    }
}

#[test]
fn test_user_action_variants() {
    let action_run = UserAction::Run("git status".to_string());
    let action_edit = UserAction::Edit("git status".to_string());
    let action_cancel = UserAction::Cancel;

    match action_run {
        UserAction::Run(cmd) => assert_eq!(cmd, "git status"),
        _ => panic!("Expected Run"),
    }
    match action_edit {
        UserAction::Edit(cmd) => assert_eq!(cmd, "git status"),
        _ => panic!("Expected Edit"),
    }
    assert_eq!(action_cancel, UserAction::Cancel);
}

#[test]
fn test_handle_key_events() {
    let cmd = "docker ps -a";
    let mut armed = false;

    let key = |code| KeyEvent::new(code, KeyModifiers::NONE);

    // Enter -> Run
    assert_eq!(
        handle_key_event(key(KeyCode::Enter), cmd, false, &mut armed),
        Some(UserAction::Run(cmd.to_string()))
    );

    // Tab -> Edit
    assert_eq!(
        handle_key_event(key(KeyCode::Tab), cmd, false, &mut armed),
        Some(UserAction::Edit(cmd.to_string()))
    );

    // 'e' -> Edit
    assert_eq!(
        handle_key_event(key(KeyCode::Char('e')), cmd, false, &mut armed),
        Some(UserAction::Edit(cmd.to_string()))
    );

    // Esc / q / Ctrl+C -> Cancel
    assert_eq!(
        handle_key_event(key(KeyCode::Esc), cmd, false, &mut armed),
        Some(UserAction::Cancel)
    );
    assert_eq!(
        handle_key_event(key(KeyCode::Char('q')), cmd, false, &mut armed),
        Some(UserAction::Cancel)
    );
    let key_ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(
        handle_key_event(key_ctrl_c, cmd, false, &mut armed),
        Some(UserAction::Cancel)
    );

    // Unhandled keys -> None
    assert_eq!(handle_key_event(key(KeyCode::Char(' ')), cmd, false, &mut armed), None);
    assert_eq!(handle_key_event(key(KeyCode::Down), cmd, false, &mut armed), None);
}

/// Spec §5.4: a destructive command must not execute on the first Enter.
#[test]
fn test_destructive_command_requires_explicit_confirmation() {
    let cmd = "rm -rf /var/tmp/build-cache";
    let mut armed = false;
    let key = |code| KeyEvent::new(code, KeyModifiers::NONE);

    // First Enter only arms the confirmation and reports no action.
    assert_eq!(
        handle_key_event(key(KeyCode::Enter), cmd, true, &mut armed),
        None
    );
    assert!(armed, "first Enter should arm the gate");

    // 'y' before arming must not run anything.
    let mut unarmed = false;
    assert_eq!(
        handle_key_event(key(KeyCode::Char('y')), cmd, true, &mut unarmed),
        None
    );

    // 'y' after arming confirms.
    assert_eq!(
        handle_key_event(key(KeyCode::Char('y')), cmd, true, &mut armed),
        Some(UserAction::Run(cmd.to_string()))
    );

    // Tab stays available for review regardless of the gate.
    let mut fresh = false;
    assert_eq!(
        handle_key_event(key(KeyCode::Tab), cmd, true, &mut fresh),
        Some(UserAction::Edit(cmd.to_string()))
    );
}

#[test]
fn test_tui_render_safe_command_layout() {
    let backend = TestBackend::new(100, 20);
    let mut terminal = Terminal::new(backend).unwrap();

    let resp = safe_response("kubectl get pods -A");
    draw_confirmation_ui(&mut terminal, &resp, false).unwrap();

    let rendered_text = format!("{:?}", terminal.backend().buffer());
    assert!(rendered_text.contains("Suggested Command"));
    assert!(rendered_text.contains("kubectl get pods -A"));
    assert!(rendered_text.contains("Explanation"));
    assert!(rendered_text.contains("SAFE (Read-Only)"));
    assert!(rendered_text.contains("Run in shell"));
    assert!(rendered_text.contains("Edit on prompt"));
}

#[test]
fn test_tui_render_destructive_command_layout() {
    let resp = AiCommandResponse {
        command: "kubectl delete namespace production".to_string(),
        explanation: "Destroys production namespace and all running workloads".to_string(),
        safety_level: SafetyLevel::Destructive,
        destructive_warning: Some("Irreversible production workload deletion".to_string()),
    };

    // Unarmed: Enter offers confirmation rather than execution.
    let backend = TestBackend::new(100, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    draw_confirmation_ui(&mut terminal, &resp, false).unwrap();
    let rendered = format!("{:?}", terminal.backend().buffer());
    assert!(rendered.contains("HIGH RISK / DESTRUCTIVE ACTION"));
    assert!(rendered.contains("Irreversible production workload deletion"));
    assert!(rendered.contains("Confirm"));
    assert!(!rendered.contains("Run in shell"));

    // Armed: the modal asks for the explicit `y`.
    let mut armed_terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
    draw_confirmation_ui(&mut armed_terminal, &resp, true).unwrap();
    let armed = format!("{:?}", armed_terminal.backend().buffer());
    assert!(armed.contains("[y]"), "armed modal should ask for y: {}", armed);
}

#[test]
fn test_render_interactive_confirmation_with_mock_events() {
    let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
    let resp = safe_response("cargo check --release");

    // Unhandled key, then Tab -> Edit.
    let mut events = VecDeque::from([
        Event::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
        Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
    ]);

    let action = render_interactive_confirmation_with(
        &mut terminal,
        || {
            events.pop_front().ok_or_else(|| {
                ChelpError::Io(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "No more events",
                ))
            })
        },
        &resp,
    );

    assert_eq!(action.unwrap(), UserAction::Edit("cargo check --release".to_string()));
}

/// A destructive response walks Enter (arm) -> y (confirm) through the same
/// event loop the real terminal drives.
#[test]
fn test_render_interactive_confirmation_gates_destructive_command() {
    let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
    let resp = AiCommandResponse {
        command: "mkfs.ext4 /dev/sdb1".to_string(),
        explanation: "Formats the disk".to_string(),
        safety_level: SafetyLevel::Destructive,
        destructive_warning: Some("Disk formatting completely wipes storage partitions.".to_string()),
    };

    let mut events = VecDeque::from([
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        Event::Key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)),
    ]);

    let action = render_interactive_confirmation_with(
        &mut terminal,
        || {
            events.pop_front().ok_or_else(|| {
                ChelpError::Io(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "No more events",
                ))
            })
        },
        &resp,
    )
    .unwrap();

    assert_eq!(action, UserAction::Run("mkfs.ext4 /dev/sdb1".to_string()));
}
