// tests/dlp_test.rs
use chelp::ai::build_prompt;
use chelp::dlp::DlpRedactor;
use chelp::models::ShellContext;

#[test]
fn test_dlp_redacts_aws_keys() {
    let input = "export AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE and deploy";
    let redacted = DlpRedactor::redact(input);
    assert!(!redacted.contains("AKIAIOSFODNN7EXAMPLE"));
    assert!(redacted.contains("[REDACTED_AWS_KEY]"));
}

#[test]
fn test_dlp_redacts_github_tokens() {
    let input = "git clone https://ghp_abcdefghijklmnopqrstuvwxyz1234567890@github.com/org/repo.git";
    let redacted = DlpRedactor::redact(input);
    assert!(!redacted.contains("ghp_abcdefghijklmnopqrstuvwxyz1234567890"));
    assert!(redacted.contains("[REDACTED_GITHUB_TOKEN]"));
}

#[test]
fn test_dlp_redacts_slack_tokens() {
    let input = "curl -H 'Authorization: Bearer xoxb-123456789012-1234567890123-abcdefghijklmnopqrstuvwx' https://slack.com/api";
    let redacted = DlpRedactor::redact(input);
    assert!(!redacted.contains("xoxb-123456789012-1234567890123-abcdefghijklmnopqrstuvwx"));
    assert!(redacted.contains("[REDACTED_SLACK_TOKEN]"));
}

#[test]
fn test_dlp_redacts_openai_and_anthropic_keys() {
    let openai_input = "set OPENAI_API_KEY=sk-proj-abcdefghijklmnopqrstuvwxyz1234567890";
    let redacted = DlpRedactor::redact(openai_input);
    assert!(!redacted.contains("sk-proj-abcdefghijklmnopqrstuvwxyz1234567890"));
    assert!(redacted.contains("[REDACTED_AI_KEY]"));

    let anthropic_input = "export ANTHROPIC_API_KEY=sk-ant-api03-abcdefghijklmnopqrstuvwxyz1234567890";
    let redacted_ant = DlpRedactor::redact(anthropic_input);
    assert!(!redacted_ant.contains("sk-ant-api03-abcdefghijklmnopqrstuvwxyz1234567890"));
    assert!(redacted_ant.contains("[REDACTED_AI_KEY]"));
}

#[test]
fn test_dlp_redacts_private_keys() {
    let input = "ssh with key -----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA0m...\n-----END RSA PRIVATE KEY----- on port 22";
    let redacted = DlpRedactor::redact(input);
    assert!(!redacted.contains("MIIEowIBAAKCAQEA0m"));
    assert!(redacted.contains("[REDACTED_PRIVATE_KEY]"));
}

#[test]
fn test_dlp_redacts_passwords_and_secrets() {
    let input = "mysql -u root -ppassword=SuperSecretPass123! dbname";
    let redacted = DlpRedactor::redact(input);
    assert!(!redacted.contains("SuperSecretPass123!"));
    assert!(redacted.contains("[REDACTED_SECRET]"));
}

#[test]
fn test_dlp_integrated_into_build_prompt() {
    let ctx = ShellContext {
        os: "windows".to_string(),
        shell: "pwsh".to_string(),
        cwd: "C:\\Users\\admin".to_string(),
    };
    let prompt = build_prompt(
        "deploy with AWS key AKIA1111222233334444 and secret=MySecretPassword!",
        &ctx,
        &[],
    );

    assert!(!prompt.contains("AKIA1111222233334444"));
    assert!(!prompt.contains("MySecretPassword!"));
    assert!(prompt.contains("[REDACTED_AWS_KEY]"));
    assert!(prompt.contains("[REDACTED_SECRET]"));
}
