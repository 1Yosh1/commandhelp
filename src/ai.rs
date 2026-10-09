// src/ai.rs
use crate::config::AiConfig;
use crate::error::ChelpError;
use crate::models::{AiCommandResponse, CliCommandSchema, ShellContext};
use crate::safety::sanitize_and_verify;
use async_trait::async_trait;
use serde_json::Value;
use std::time::Duration;

/// Spec §7: generous provider timeout, but bounded so a hung endpoint cannot
/// hold a shell hook open forever.
const HTTP_TIMEOUT: Duration = Duration::from_secs(60);

pub fn build_prompt(user_query: &str, ctx: &ShellContext, schemas: &[CliCommandSchema]) -> String {
    let mut schema_summary = String::new();
    for s in schemas {
        schema_summary.push_str(&format!(
            "Tool Context: {} (Subcommands: {})\n",
            s.binary,
            s.subcommands.join(", ")
        ));
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

pub fn clean_json_response(raw: &str) -> &str {
    let trimmed = raw.trim();
    if let Some(stripped) = trimmed.strip_prefix("```json") {
        if let Some(end) = stripped.strip_suffix("```") {
            return end.trim();
        }
    } else if let Some(stripped) = trimmed.strip_prefix("```") {
        if let Some(end) = stripped.strip_suffix("```") {
            return end.trim();
        }
    }
    trimmed
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

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .unwrap_or_default()
}

/// The single place every provider's round-trip goes through: HTTP status
/// handling, JSON extraction through a JSON pointer, response decoding, and the
/// mandatory local safety verdict (spec §5.4).
///
/// `text_pointer` locates the assistant text inside the envelope; an empty
/// pointer means the body already *is* an `AiCommandResponse` (Pro gateway).
async fn resolve_via_http(
    provider: &str,
    client: &reqwest::Client,
    url: &str,
    headers: &[(&str, String)],
    body: Value,
    text_pointer: &str,
) -> Result<AiCommandResponse, ChelpError> {
    let mut request = client.post(url);
    for (name, value) in headers {
        request = request.header(*name, value.clone());
    }

    let response = request
        .json(&body)
        .send()
        .await
        .map_err(|e| ChelpError::AiProvider(format!("{} request failed: {}", provider, e)))?;

    let status = response.status();
    let raw_body = response
        .text()
        .await
        .map_err(|e| ChelpError::AiProvider(format!("{} returned no body: {}", provider, e)))?;
    let value: Value = serde_json::from_str(&raw_body).unwrap_or(Value::Null);

    if !status.is_success() {
        let message = value
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or(&raw_body);
        return Err(ChelpError::AiProvider(format!(
            "{} API error ({}): {}",
            provider, status, message
        )));
    }

    let mut parsed: AiCommandResponse = if text_pointer.is_empty() {
        serde_json::from_value(value.clone()).map_err(|e| {
            ChelpError::AiProvider(format!("Invalid response from {}: {}", provider, e))
        })?
    } else {
        let text = value
            .pointer(text_pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ChelpError::AiProvider(format!("Empty model response from {}", provider))
            })?;
        let cleaned = clean_json_response(text);
        serde_json::from_str(cleaned).map_err(|e| {
            ChelpError::AiProvider(format!(
                "Invalid response JSON from {}: {}. Raw: {}",
                provider, e, text
            ))
        })?
    };

    sanitize_and_verify(&mut parsed);
    Ok(parsed)
}

// -----------------------------------------------------------------------------
// 1. Google Gemini (default BYOK provider)
// -----------------------------------------------------------------------------
pub struct GeminiProvider {
    pub api_key: String,
    pub model: String,
    pub client: reqwest::Client,
}

impl GeminiProvider {
    pub fn new(api_key: String) -> Self {
        Self::with_model(api_key, "gemini-2.5-flash".to_string())
    }

    pub fn with_model(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: http_client(),
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
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );
        let body = serde_json::json!({
            "contents": [{ "parts": [{ "text": build_prompt(query, ctx, schemas) }] }],
            "generationConfig": { "response_mime_type": "application/json" }
        });
        resolve_via_http(
            "Gemini",
            &self.client,
            &url,
            &[],
            body,
            "/candidates/0/content/parts/0/text",
        )
        .await
    }
}

// -----------------------------------------------------------------------------
// 2. OpenAI, and every OpenAI-compatible endpoint (Groq, DeepSeek, OpenRouter)
// -----------------------------------------------------------------------------
pub struct OpenAiProvider {
    pub api_key: String,
    pub model: String,
    pub endpoint: String,
    pub client: reqwest::Client,
}

impl OpenAiProvider {
    pub fn new(api_key: String, model: Option<String>, endpoint: Option<String>) -> Self {
        Self {
            api_key,
            model: model.unwrap_or_else(|| "gpt-4o-mini".to_string()),
            endpoint: endpoint.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            client: http_client(),
        }
    }
}

#[async_trait]
impl AiProvider for OpenAiProvider {
    async fn resolve_intent(
        &self,
        query: &str,
        ctx: &ShellContext,
        schemas: &[CliCommandSchema],
    ) -> Result<AiCommandResponse, ChelpError> {
        let url = format!("{}/chat/completions", self.endpoint.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are CommandHelp AI. Output only valid raw JSON conforming to the requested schema. No markdown."
                },
                { "role": "user", "content": build_prompt(query, ctx, schemas) }
            ],
            "response_format": { "type": "json_object" }
        });
        let headers = [
            ("Authorization", format!("Bearer {}", self.api_key)),
            (
                "HTTP-Referer",
                "https://github.com/1Yosh1/commandhelp".to_string(),
            ),
            ("X-Title", "CommandHelp CLI".to_string()),
        ];
        resolve_via_http(
            "OpenAI",
            &self.client,
            &url,
            &headers,
            body,
            "/choices/0/message/content",
        )
        .await
    }
}

// -----------------------------------------------------------------------------
// 3. Anthropic Claude
// -----------------------------------------------------------------------------
pub struct AnthropicProvider {
    pub api_key: String,
    pub model: String,
    pub client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: Option<String>) -> Self {
        Self {
            api_key,
            model: model.unwrap_or_else(|| "claude-3-5-haiku-20241022".to_string()),
            client: http_client(),
        }
    }
}

#[async_trait]
impl AiProvider for AnthropicProvider {
    async fn resolve_intent(
        &self,
        query: &str,
        ctx: &ShellContext,
        schemas: &[CliCommandSchema],
    ) -> Result<AiCommandResponse, ChelpError> {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 1024,
            "messages": [{ "role": "user", "content": build_prompt(query, ctx, schemas) }]
        });
        let headers = [
            ("x-api-key", self.api_key.clone()),
            ("anthropic-version", "2023-06-01".to_string()),
            ("content-type", "application/json".to_string()),
        ];
        resolve_via_http(
            "Anthropic",
            &self.client,
            "https://api.anthropic.com/v1/messages",
            &headers,
            body,
            "/content/0/text",
        )
        .await
    }
}

// -----------------------------------------------------------------------------
// 4. Ollama (local, offline)
// -----------------------------------------------------------------------------
pub struct OllamaProvider {
    pub endpoint: String,
    pub model: String,
    pub client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(endpoint: Option<String>, model: Option<String>) -> Self {
        Self {
            endpoint: endpoint.unwrap_or_else(|| "http://localhost:11434".to_string()),
            model: model.unwrap_or_else(|| "qwen2.5-coder:latest".to_string()),
            client: http_client(),
        }
    }
}

#[async_trait]
impl AiProvider for OllamaProvider {
    async fn resolve_intent(
        &self,
        query: &str,
        ctx: &ShellContext,
        schemas: &[CliCommandSchema],
    ) -> Result<AiCommandResponse, ChelpError> {
        let url = format!("{}/api/generate", self.endpoint.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.model,
            "prompt": build_prompt(query, ctx, schemas),
            "stream": false,
            "format": "json"
        });
        resolve_via_http("Ollama", &self.client, &url, &[], body, "/response").await
    }
}

// -----------------------------------------------------------------------------
// 5. CommandHelp Pro (hosted gateway)
// -----------------------------------------------------------------------------
pub struct ProProvider {
    pub token: String,
    pub endpoint: String,
    pub client: reqwest::Client,
}

impl ProProvider {
    pub fn new(token: String, endpoint: Option<String>) -> Self {
        Self {
            token,
            endpoint: endpoint.unwrap_or_else(|| "https://api.commandhelp.dev".to_string()),
            client: http_client(),
        }
    }
}

#[async_trait]
impl AiProvider for ProProvider {
    async fn resolve_intent(
        &self,
        query: &str,
        ctx: &ShellContext,
        schemas: &[CliCommandSchema],
    ) -> Result<AiCommandResponse, ChelpError> {
        let url = format!("{}/api/query", self.endpoint.trim_end_matches('/'));
        let body = serde_json::json!({
            "prompt": query,
            "os": ctx.os,
            "shell": ctx.shell,
            "cwd": ctx.cwd,
            "schemas": schemas,
        });
        let headers = [("Authorization", format!("Bearer {}", self.token))];
        resolve_via_http("CommandHelp Pro", &self.client, &url, &headers, body, "").await
    }
}

// -----------------------------------------------------------------------------
// Factory: instantiate the provider named in the user's config
// -----------------------------------------------------------------------------
pub fn create_provider_from_config(cfg: &AiConfig) -> Result<Box<dyn AiProvider>, ChelpError> {
    match cfg.provider.to_lowercase().as_str() {
        "gemini" | "google" => {
            let key = cfg
                .api_key
                .clone()
                .or_else(|| std::env::var("GEMINI_API_KEY").ok())
                .ok_or_else(|| {
                    ChelpError::Config(
                        "Missing Gemini API Key. Run 'chelp config' or set GEMINI_API_KEY."
                            .to_string(),
                    )
                })?;
            let model = cfg
                .model
                .clone()
                .unwrap_or_else(|| "gemini-2.5-flash".to_string());
            Ok(Box::new(GeminiProvider::with_model(key, model)))
        }
        "openai" => {
            let key = cfg
                .api_key
                .clone()
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .ok_or_else(|| {
                    ChelpError::Config(
                        "Missing OpenAI API Key. Run 'chelp config' or set OPENAI_API_KEY."
                            .to_string(),
                    )
                })?;
            Ok(Box::new(OpenAiProvider::new(
                key,
                cfg.model.clone(),
                cfg.endpoint.clone(),
            )))
        }
        "anthropic" | "claude" => {
            let key = cfg
                .api_key
                .clone()
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
                .ok_or_else(|| {
                    ChelpError::Config(
                        "Missing Anthropic API Key. Run 'chelp config' or set ANTHROPIC_API_KEY."
                            .to_string(),
                    )
                })?;
            Ok(Box::new(AnthropicProvider::new(key, cfg.model.clone())))
        }
        "ollama" => Ok(Box::new(OllamaProvider::new(
            cfg.endpoint.clone(),
            cfg.model.clone(),
        ))),
        "pro" | "cloud" => {
            let token = cfg
                .api_key
                .clone()
                .or_else(|| std::env::var("CHELP_API_TOKEN").ok())
                .or_else(|| {
                    crate::auth::load_credentials()
                        .ok()
                        .flatten()
                        .map(|c| c.token)
                })
                .ok_or_else(|| {
                    ChelpError::Config(
                        "Missing CommandHelp Pro token. Run 'chelp login' to authenticate."
                            .to_string(),
                    )
                })?;
            Ok(Box::new(ProProvider::new(token, cfg.endpoint.clone())))
        }
        "custom" | "groq" | "deepseek" | "openrouter" => {
            let key = cfg
                .api_key
                .clone()
                .or_else(|| std::env::var("CUSTOM_API_KEY").ok())
                .unwrap_or_default();
            let endpoint = cfg.endpoint.clone().ok_or_else(|| {
                ChelpError::Config(
                    "Custom provider requires an endpoint URL (e.g. https://api.groq.com/openai/v1)"
                        .to_string(),
                )
            })?;
            Ok(Box::new(OpenAiProvider::new(
                key,
                cfg.model.clone(),
                Some(endpoint),
            )))
        }
        other => Err(ChelpError::Config(format!(
            "Unknown provider '{}'. Supported: gemini, openai, anthropic, ollama, pro, custom",
            other
        ))),
    }
}
