// tests/ai_test.rs
use chelp::ai::build_prompt;
use chelp::models::{CliCommandSchema, ShellContext};

#[test]
fn test_build_prompt_includes_context() {
    let ctx = ShellContext {
        os: "windows".to_string(),
        shell: "pwsh".to_string(),
        cwd: "C:\\projects".to_string(),
    };

    let schema = CliCommandSchema {
        binary: "git".to_string(),
        subcommand_path: vec![],
        usage: "git [options] <command>".to_string(),
        description: "Fast version control system".to_string(),
        flags: vec![],
        subcommands: vec!["commit".to_string(), "push".to_string()],
        binary_mtime: 0,
        last_indexed: 0,
    };

    let prompt = build_prompt("commit with message wip", &ctx, &[schema]);
    assert!(prompt.contains("Target OS: windows"));
    assert!(prompt.contains("Target Shell: pwsh"));
    assert!(prompt.contains("Tool Context: git"));
}
