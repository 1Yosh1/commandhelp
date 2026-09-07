// src/ai.rs
use crate::config::AiConfig;
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

// -----------------------------------------------------------------------------
// 1. Google Gemini Provider
// -----------------------------------------------------------------------------
pub struct GeminiProvider {
    pub api_key: String,
    pub model: String,
    pub client: reqwest::Client,
}

impl GeminiProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "gemini-2.5-flash".to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_model(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
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
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
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
            .map_err(|e| ChelpError::AiProvider(format!("Gemini request failed: {}", e)))?;

        let json_res: serde_json::Value = res.json().await
            .map_err(|e| ChelpError::AiProvider(format!("Failed to parse Gemini response: {}", e)))?;

        if let Some(err) = json_res.get("error") {
            return Err(ChelpError::AiProvider(format!("Gemini API error: {}", err["message"].as_str().unwrap_or("unknown"))));
        }

        let text = json_res["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| ChelpError::AiProvider("Empty model response from Gemini".to_string()))?;

        let cleaned = clean_json_response(text);
        let mut ai_resp: AiCommandResponse = serde_json::from_str(cleaned)
            .map_err(|e| ChelpError::AiProvider(format!("Invalid response JSON from Gemini: {}. Raw was: {}", e, text)))?;

        sanitize_and_verify(&mut ai_resp);
        Ok(ai_resp)
    }
}

// -----------------------------------------------------------------------------
// 2. OpenAI & OpenAI-Compatible Provider (Groq, DeepSeek, OpenRouter)
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
            client: reqwest::Client::new(),
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
        let prompt = build_prompt(query, ctx, schemas);
        let url = format!("{}/chat/completions", self.endpoint.trim_end_matches('/'));

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are CommandHelp AI. Output only valid raw JSON conforming to the requested schema. No markdown."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "response_format": { "type": "json_object" }
        });

        let res = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| ChelpError::AiProvider(format!("OpenAI request failed: {}", e)))?;

        let json_res: serde_json::Value = res.json().await
            .map_err(|e| ChelpError::AiProvider(format!("Failed to parse OpenAI response: {}", e)))?;

        if let Some(err) = json_res.get("error") {
            return Err(ChelpError::AiProvider(format!("OpenAI API error: {}", err["message"].as_str().unwrap_or("unknown"))));
        }

        let text = json_res["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| ChelpError::AiProvider("Empty model response from OpenAI".to_string()))?;

        let cleaned = clean_json_response(text);
        let mut ai_resp: AiCommandResponse = serde_json::from_str(cleaned)
            .map_err(|e| ChelpError::AiProvider(format!("Invalid response JSON from OpenAI: {}. Raw: {}", e, text)))?;

        sanitize_and_verify(&mut ai_resp);
        Ok(ai_resp)
    }
}

// -----------------------------------------------------------------------------
// 3. Anthropic Claude Provider
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
            client: reqwest::Client::new(),
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
        let prompt = build_prompt(query, ctx, schemas);
        let url = "https://api.anthropic.com/v1/messages";

        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 1024,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ]
        });

        let res = self.client.post(url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ChelpError::AiProvider(format!("Anthropic request failed: {}", e)))?;

        let json_res: serde_json::Value = res.json().await
            .map_err(|e| ChelpError::AiProvider(format!("Failed to parse Anthropic response: {}", e)))?;

        if let Some(err) = json_res.get("error") {
            return Err(ChelpError::AiProvider(format!("Anthropic API error: {}", err["message"].as_str().unwrap_or("unknown"))));
        }

        let text = json_res["content"][0]["text"]
            .as_str()
            .ok_or_else(|| ChelpError::AiProvider("Empty model response from Anthropic".to_string()))?;

        let cleaned = clean_json_response(text);
        let mut ai_resp: AiCommandResponse = serde_json::from_str(cleaned)
            .map_err(|e| ChelpError::AiProvider(format!("Invalid response JSON from Anthropic: {}. Raw: {}", e, text)))?;

        sanitize_and_verify(&mut ai_resp);
        Ok(ai_resp)
    }
}

// -----------------------------------------------------------------------------
// 4. Ollama (Local & Free, Offline)
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
            client: reqwest::Client::new(),
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
        let prompt = build_prompt(query, ctx, schemas);
        let url = format!("{}/api/generate", self.endpoint.trim_end_matches('/'));

        let body = serde_json::json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false,
            "format": "json"
        });

        let res = self.client.post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ChelpError::AiProvider(format!("Ollama request failed (is Ollama running on {}?): {}", self.endpoint, e)))?;

        let json_res: serde_json::Value = res.json().await
            .map_err(|e| ChelpError::AiProvider(format!("Failed to parse Ollama response: {}", e)))?;

        let text = json_res["response"]
            .as_str()
            .ok_or_else(|| ChelpError::AiProvider("Empty model response from Ollama".to_string()))?;

        let cleaned = clean_json_response(text);
        let mut ai_resp: AiCommandResponse = serde_json::from_str(cleaned)
            .map_err(|e| ChelpError::AiProvider(format!("Invalid response JSON from Ollama: {}. Raw: {}", e, text)))?;

        sanitize_and_verify(&mut ai_resp);
        Ok(ai_resp)
    }
}

// -----------------------------------------------------------------------------
// Factory: Instantiate Provider from Config
// -----------------------------------------------------------------------------
pub fn create_provider_from_config(cfg: &AiConfig) -> Result<Box<dyn AiProvider>, ChelpError> {
    match cfg.provider.to_lowercase().as_str() {
        "gemini" | "google" => {
            let key = cfg.api_key.clone()
                .or_else(|| std::env::var("GEMINI_API_KEY").ok())
                .ok_or_else(|| ChelpError::Config("Missing Gemini API Key. Run 'chelp config' or set GEMINI_API_KEY.".to_string()))?;
            let model = cfg.model.clone().unwrap_or_else(|| "gemini-2.5-flash".to_string());
            Ok(Box::new(GeminiProvider::with_model(key, model)))
        }
        "openai" => {
            let key = cfg.api_key.clone()
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .ok_or_else(|| ChelpError::Config("Missing OpenAI API Key. Run 'chelp config' or set OPENAI_API_KEY.".to_string()))?;
            Ok(Box::new(OpenAiProvider::new(key, cfg.model.clone(), cfg.endpoint.clone())))
        }
        "anthropic" | "claude" => {
            let key = cfg.api_key.clone()
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
                .ok_or_else(|| ChelpError::Config("Missing Anthropic API Key. Run 'chelp config' or set ANTHROPIC_API_KEY.".to_string()))?;
            Ok(Box::new(AnthropicProvider::new(key, cfg.model.clone())))
        }
        "ollama" => {
            Ok(Box::new(OllamaProvider::new(cfg.endpoint.clone(), cfg.model.clone())))
        }
        "custom" | "groq" | "deepseek" | "openrouter" => {
            let key = cfg.api_key.clone()
                .or_else(|| std::env::var("CUSTOM_API_KEY").ok())
                .unwrap_or_default();
            let endpoint = cfg.endpoint.clone()
                .ok_or_else(|| ChelpError::Config("Custom provider requires an endpoint URL (e.g. https://api.groq.com/openai/v1)".to_string()))?;
            Ok(Box::new(OpenAiProvider::new(key, cfg.model.clone(), Some(endpoint))))
        }
        other => Err(ChelpError::Config(format!(
            "Unknown provider '{}'. Supported: gemini, openai, anthropic, ollama, custom",
            other
        ))),
    }
}
