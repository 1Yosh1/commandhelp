# Task 6: AI Provider Engine (BYOK: Gemini / Ollama) & Context Prompting

**Plan File:** `docs/superpowers/plans/2026-09-06-ai-cli-assistant.md`  
**Spec File:** `docs/superpowers/specs/2026-09-06-ai-cli-assistant-design.md`

## Files
- Create: `src/ai.rs`
- Test: `tests/ai_test.rs`

## Interfaces
- Consumes: `CliCommandSchema`, `ShellContext`, `AiCommandResponse`, `sanitize_and_verify` from `models.rs` and `safety.rs`
- Produces:
  - `AiProvider` trait with `resolve_intent(&self, prompt: &str, ctx: &ShellContext, schemas: &[CliCommandSchema]) -> Result<AiCommandResponse, ChelpError>`
  - `GeminiProvider::new(api_key: String)`
  - `build_prompt(prompt: &str, ctx: &ShellContext, schemas: &[CliCommandSchema]) -> String`

## Steps

### Step 1: Write the failing test

```rust
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
```

### Step 2: Run test to verify it fails

Run: `cargo test --test ai_test`  
Expected: FAIL (module `chelp::ai` not found)

### Step 3: Write minimal implementation

```rust
// src/ai.rs
use crate::error::ChelpError;
use crate::models::{AiCommandResponse, CliCommandSchema, ShellContext};
use crate::safety::sanitize_and_verify;
use async_trait::async_trait;

pub fn build_prompt(user_query: &str, ctx: &ShellContext, schemas: &[CliCommandSchema]) -> String {
    let mut schema_summary = String::new();
    for s in schemas {
        schema_summary.push_str(&format!("Tool Context: {} (Subcommands: {})\n", s.binary, s.subcommands.join(", ")));
        for f in &s.flags {
            if let Some(long) = &f.long {
                schema_summary.push_str(&format!("  {} : {}\n", long, f.description));
            }
        }
    }

    format!(
r#"You are CommandHelp AI, an expert CLI assistant.
Target OS: {}
Target Shell: {}
Working Directory: {}

Available Tool Specs:
{}

User Intent: "{}"

Respond strictly with a JSON object matching this schema:
{{
  "command": "<exact runnable command line string>",
  "explanation": "<1-2 sentence explanation>",
  "safety_level": "safe" | "caution" | "destructive",
  "destructive_warning": null or "<warning message>"
}}
No markdown formatting, no code backticks. Just the raw JSON object."#,
        ctx.os, ctx.shell, ctx.cwd, schema_summary, user_query
    )
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    async fn resolve_intent(
        &self,
        query: &str,
        ctx: &ShellContext,
        schemas: &[CliCommandSchema],
    ) -> Result<AiCommandResponse, ChelpError>;
}

pub struct GeminiProvider {
    pub api_key: String,
    pub client: reqwest::Client,
}

impl GeminiProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl AiProvider for GeminiProvider {
    async fn resolve_intent(
        &self,
        query: &str,
        ctx: &ShellContext,
        schemas: &[CliCommandSchema],
    ) -> Result<AiCommandResponse, ChelpError> {
        let prompt = build_prompt(query, ctx, schemas);
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={}",
            self.api_key
        );

        let body = serde_json::json!({
            "contents": [{
                "parts": [{ "text": prompt }]
            }],
            "generationConfig": {
                "response_mime_type": "application/json"
            }
        });

        let res = self.client.post(&url).json(&body).send().await
            .map_err(|e| ChelpError::AiProvider(format!("HTTP request failed: {}", e)))?;

        let json_res: serde_json::Value = res.json().await
            .map_err(|e| ChelpError::AiProvider(format!("Failed to parse response: {}", e)))?;

        let text = json_res["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| ChelpError::AiProvider("Empty model response".to_string()))?;

        let mut ai_resp: AiCommandResponse = serde_json::from_str(text)
            .map_err(|e| ChelpError::AiProvider(format!("Invalid response JSON: {}", e)))?;

        sanitize_and_verify(&mut ai_resp);
        Ok(ai_resp)
    }
}
```

Add `pub mod ai;` to `src/lib.rs`.

### Step 4: Run test to verify it passes

Run: `cargo test --test ai_test`  
Expected: PASS

### Step 5: Commit

```bash
git add src/ai.rs tests/ai_test.rs src/lib.rs
git commit -m "feat: implement AI prompt builder and Gemini provider"
```
