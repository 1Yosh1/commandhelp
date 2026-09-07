// tests/tui_test.rs
use chelp::tui::UserAction;

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
