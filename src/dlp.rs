// src/dlp.rs
use regex::Regex;

pub struct DlpRedactor;

impl DlpRedactor {
    /// Scrub sensitive credentials, tokens, private keys, and passwords from text
    pub fn redact(text: &str) -> String {
        let mut result = text.to_string();

        // 1. Private keys (PEM format)
        let private_key_re = Regex::new(r"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----").unwrap();
        result = private_key_re.replace_all(&result, "[REDACTED_PRIVATE_KEY]").to_string();

        // 2. AWS Access Key IDs (AKIA, ASIA, etc.)
        let aws_key_re = Regex::new(r"\b(AKIA|ABIA|ACCA|ASIA)[0-9A-Z]{16}\b").unwrap();
        result = aws_key_re.replace_all(&result, "[REDACTED_AWS_KEY]").to_string();

        // 3. GitHub Personal Access Tokens
        let gh_token_re = Regex::new(r"\b(ghp_[a-zA-Z0-9]{36}|github_pat_[a-zA-Z0-9_]{82})\b").unwrap();
        result = gh_token_re.replace_all(&result, "[REDACTED_GITHUB_TOKEN]").to_string();

        // 4. Slack tokens
        let slack_token_re = Regex::new(r"\bxox[baprs]-[0-9a-zA-Z-]{10,}\b").unwrap();
        result = slack_token_re.replace_all(&result, "[REDACTED_SLACK_TOKEN]").to_string();

        // 5. OpenAI API keys
        let openai_key_re = Regex::new(r"\bsk-(?:proj-)?[a-zA-Z0-9_-]{20,}\b").unwrap();
        result = openai_key_re.replace_all(&result, "[REDACTED_AI_KEY]").to_string();

        // 6. Anthropic API keys
        let anthropic_key_re = Regex::new(r"\bsk-ant-[a-zA-Z0-9_-]{20,}\b").unwrap();
        result = anthropic_key_re.replace_all(&result, "[REDACTED_AI_KEY]").to_string();

        // 7. Generic Bearer tokens
        let bearer_re = Regex::new(r"(?i)\bBearer\s+[a-zA-Z0-9_\-\.]{25,}\b").unwrap();
        result = bearer_re.replace_all(&result, "Bearer [REDACTED_BEARER_TOKEN]").to_string();

        // 8. Passwords and credentials in assignments (e.g. password=supersecret, -ppassword=secret123, api_key="secret123")
        let secret_quoted_re = Regex::new(r#"(?i)(?:\b|-p)(password|passwd|pwd|secret|api_key|apikey)[=:\s]+["'][^"'\r\n\[]{4,}["']"#).unwrap();
        result = secret_quoted_re.replace_all(&result, "$1=[REDACTED_SECRET]").to_string();

        let secret_unquoted_re = Regex::new(r#"(?i)(?:\b|-p)(password|passwd|pwd|secret|api_key|apikey)[=:\s]+[^\[\]\s"']{4,}"#).unwrap();
        result = secret_unquoted_re.replace_all(&result, "$1=[REDACTED_SECRET]").to_string();

        result
    }
}
