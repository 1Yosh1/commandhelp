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
