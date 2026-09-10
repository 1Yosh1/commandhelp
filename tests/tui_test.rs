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

    // 1. Enter -> Run
    let key_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        handle_key_event(key_enter, cmd),
        Some(UserAction::Run(cmd.to_string()))
    );

    // 2. Tab -> Edit
    let key_tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(
        handle_key_event(key_tab, cmd),
        Some(UserAction::Edit(cmd.to_string()))
    );

    // 3. 'e' -> Edit
    let key_e = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);
    assert_eq!(
        handle_key_event(key_e, cmd),
        Some(UserAction::Edit(cmd.to_string()))
    );

    // 4. Esc -> Cancel
    let key_esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(handle_key_event(key_esc, cmd), Some(UserAction::Cancel));

    // 5. 'q' -> Cancel
    let key_q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    assert_eq!(handle_key_event(key_q, cmd), Some(UserAction::Cancel));

    // 6. Ctrl+C -> Cancel
    let key_ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(handle_key_event(key_ctrl_c, cmd), Some(UserAction::Cancel));

    // 7. Unhandled characters -> None
    let key_space = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
    assert_eq!(handle_key_event(key_space, cmd), None);

    let key_down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(handle_key_event(key_down, cmd), None);
}

#[test]
fn test_tui_render_safe_command_layout() {
    let backend = TestBackend::new(100, 20);
    let mut terminal = Terminal::new(backend).unwrap();

    let resp = AiCommandResponse {
        command: "kubectl get pods -A".to_string(),
        explanation: "Lists all pods across all Kubernetes namespaces".to_string(),
        safety_level: SafetyLevel::Safe,
        destructive_warning: None,
    };

    let res = draw_confirmation_ui(&mut terminal, &resp);
    assert!(res.is_ok());

    let buffer = terminal.backend().buffer();
    let rendered_text = format!("{:?}", buffer);

    assert!(rendered_text.contains("Suggested Command"));
    assert!(rendered_text.contains("kubectl get pods -A"));
    assert!(rendered_text.contains("Explanation"));
    assert!(rendered_text.contains("SAFE (Read-Only)"));
    assert!(rendered_text.contains("Run in shell"));
    assert!(rendered_text.contains("Edit on prompt"));
}

#[test]
fn test_tui_render_destructive_command_layout() {
    let backend = TestBackend::new(100, 20);
    let mut terminal = Terminal::new(backend).unwrap();

    let resp = AiCommandResponse {
        command: "kubectl delete namespace production".to_string(),
        explanation: "Destroys production namespace and all running workloads".to_string(),
        safety_level: SafetyLevel::Destructive,
        destructive_warning: Some("Irreversible production workload deletion".to_string()),
    };

    let res = draw_confirmation_ui(&mut terminal, &resp);
    assert!(res.is_ok());

    let buffer = terminal.backend().buffer();
    let rendered_text = format!("{:?}", buffer);

    assert!(rendered_text.contains("HIGH RISK / DESTRUCTIVE ACTION"));
    assert!(rendered_text.contains("Irreversible production workload deletion"));
}

#[test]
fn test_render_interactive_confirmation_with_mock_events() {
    let backend = TestBackend::new(100, 20);
    let mut terminal = Terminal::new(backend).unwrap();

    let resp = AiCommandResponse {
        command: "cargo check --release".to_string(),
        explanation: "Fast syntax and type checking".to_string(),
        safety_level: SafetyLevel::Safe,
        destructive_warning: None,
    };

    // Simulate event stream: first an unhandled Key (Char 'x'), then Tab (Edit)
    let mut events = VecDeque::from([
        Event::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
        Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
    ]);

    let action = render_interactive_confirmation_with(
        &mut terminal,
        || {
            events
                .pop_front()
                .ok_or_else(|| ChelpError::Io(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "No more events")))
        },
        &resp,
    );

    assert_eq!(action.unwrap(), UserAction::Edit("cargo check --release".to_string()));
}

