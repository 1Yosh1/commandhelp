// src/setup.rs
use crate::ai::create_provider_from_config;
use crate::config::{save_config, AiConfig, ChelpConfig};
use crate::error::ChelpError;
use crate::models::ShellContext;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

pub async fn run_setup() -> Result<(), ChelpError> {
    println!();
    println!("============================================================");
    println!("  CommandHelp (`chelp`) - Quick Setup Wizard");
    println!("============================================================");
    println!();

    // 1. Configure Shell Hook
    println!("[Step 1/2] Configuring Shell Integration...");
    let exe_path = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "chelp".to_string());

    if let Err(e) = setup_shell_profile(&exe_path) {
        println!("  [!] Could not automatically update profile: {}", e);
        println!("      You can manually add the hook by running: chelp init <shell>");
    }

    println!();
    // 2. Configure AI Provider
    println!("[Step 2/2] Configuring AI Provider (BYOK)...");
    run_config_wizard().await?;

    println!();
    println!("============================================================");
    println!("  ✨ Setup Complete! Ready to use chelp.");
    println!("============================================================");
    println!("  • To ask in plain English:  chelp query \"your request\"");
    println!("  • In your shell:             Press [Ctrl+Space] for inline AI help");
    println!("  • To change settings later:  chelp config");
    println!();

    Ok(())
}

fn setup_shell_profile(exe_path: &str) -> Result<(), ChelpError> {
    if cfg!(target_os = "windows") {
        let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users".to_string());
        let pwsh7_profile = PathBuf::from(&home).join("Documents\\PowerShell\\Microsoft.PowerShell_profile.ps1");
        let win_pwsh_profile = PathBuf::from(&home).join("Documents\\WindowsPowerShell\\Microsoft.PowerShell_profile.ps1");

        let target_profile = if pwsh7_profile.parent().map_or(false, |p| p.exists()) {
            pwsh7_profile
        } else {
            win_pwsh_profile
        };

        install_hook_to_file(&target_profile, &format!("Invoke-Expression (& \"{}\" init pwsh)", exe_path), "# CommandHelp Hook")?;
        println!("  ✔ PowerShell hook installed to: {:?}", target_profile);
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let zshrc = PathBuf::from(&home).join(".zshrc");
        let bashrc = PathBuf::from(&home).join(".bashrc");

        if zshrc.exists() {
            install_hook_to_file(&zshrc, &format!("eval \"$(\"{}\" init zsh)\"", exe_path), "# CommandHelp Hook")?;
            println!("  ✔ Zsh hook installed to: {:?}", zshrc);
        } else if bashrc.exists() {
            install_hook_to_file(&bashrc, &format!("eval \"$(\"{}\" init bash)\"", exe_path), "# CommandHelp Hook")?;
            println!("  ✔ Bash hook installed to: {:?}", bashrc);
        }
    }
    Ok(())
}

fn install_hook_to_file(path: &Path, hook_line: &str, comment: &str) -> Result<(), ChelpError> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let existing = if path.exists() {
        std::fs::read_to_string(path)?
    } else {
        String::new()
    };

    if !existing.contains(comment) {
        let mut new_content = existing;
        if !new_content.is_empty() && !new_content.ends_with('\n') {
            new_content.push('\n');
        }
        new_content.push_str(&format!("\n{}\n{}\n", comment, hook_line));
        std::fs::write(path, new_content)?;
    }
    Ok(())
}

pub async fn run_config_wizard() -> Result<(), ChelpError> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();

    println!("Select your AI Provider:");
    println!("  1) Google Gemini (Free tier available, ultra-fast) [Recommended]");
    println!("  2) OpenAI (GPT-4o, GPT-4o-mini)");
    println!("  3) Anthropic Claude (Claude 3.5 Haiku / Sonnet)");
    println!("  4) Ollama (100% Free & Local, offline, no API key required)");
    println!("  5) Custom / Other (Groq, DeepSeek, OpenRouter, Mistral)");
    print!("Choose [1-5] (default 1): ");
    let _ = io::stdout().flush();

    let mut choice = String::new();
    let _ = reader.read_line(&mut choice);
    let choice = choice.trim();

    let mut ai_config = AiConfig::default();

    match choice {
        "2" => {
            ai_config.provider = "openai".to_string();
            ai_config.model = Some("gpt-4o-mini".to_string());
            print!("Enter your OpenAI API Key: ");
            let _ = io::stdout().flush();
            let mut key = String::new();
            let _ = reader.read_line(&mut key);
            ai_config.api_key = Some(key.trim().to_string());
        }
        "3" => {
            ai_config.provider = "anthropic".to_string();
            ai_config.model = Some("claude-3-5-haiku-20241022".to_string());
            print!("Enter your Anthropic API Key: ");
            let _ = io::stdout().flush();
            let mut key = String::new();
            let _ = reader.read_line(&mut key);
            ai_config.api_key = Some(key.trim().to_string());
        }
        "4" => {
            ai_config.provider = "ollama".to_string();
            ai_config.model = Some("qwen2.5-coder:latest".to_string());
            print!("Enter Ollama endpoint (default http://localhost:11434): ");
            let _ = io::stdout().flush();
            let mut ep = String::new();
            let _ = reader.read_line(&mut ep);
            let ep = ep.trim();
            ai_config.endpoint = if ep.is_empty() {
                Some("http://localhost:11434".to_string())
            } else {
                Some(ep.to_string())
            };
            print!("Enter Ollama model (default qwen2.5-coder:latest): ");
            let _ = io::stdout().flush();
            let mut m = String::new();
            let _ = reader.read_line(&mut m);
            let m = m.trim();
            if !m.is_empty() {
                ai_config.model = Some(m.to_string());
            }
        }
        "5" => {
            ai_config.provider = "custom".to_string();
            print!("Enter Endpoint URL (e.g. https://api.groq.com/openai/v1): ");
            let _ = io::stdout().flush();
            let mut ep = String::new();
            let _ = reader.read_line(&mut ep);
            ai_config.endpoint = Some(ep.trim().to_string());

            print!("Enter API Key: ");
            let _ = io::stdout().flush();
            let mut key = String::new();
            let _ = reader.read_line(&mut key);
            ai_config.api_key = Some(key.trim().to_string());

            print!("Enter Model Name (e.g. llama-3.3-70b-versatile): ");
            let _ = io::stdout().flush();
            let mut m = String::new();
            let _ = reader.read_line(&mut m);
            ai_config.model = Some(m.trim().to_string());
        }
        _ => {
            // Default: Gemini
            ai_config.provider = "gemini".to_string();
            ai_config.model = Some("gemini-2.5-flash".to_string());
            print!("Enter your Gemini API Key (Get a free key at https://aistudio.google.com): ");
            let _ = io::stdout().flush();
            let mut key = String::new();
            let _ = reader.read_line(&mut key);
            ai_config.api_key = Some(key.trim().to_string());
        }
    }

    let config = ChelpConfig { ai: ai_config };
    let saved_path = save_config(&config)?;
    println!("  ✔ Configuration saved to: {:?}", saved_path);

    // Test the provider
    print!("  Testing AI connection... ");
    let _ = io::stdout().flush();
    match create_provider_from_config(&config.ai) {
        Ok(provider) => {
            let ctx = ShellContext {
                os: std::env::consts::OS.to_string(),
                shell: "shell".to_string(),
                cwd: ".".to_string(),
            };
            let start = std::time::Instant::now();
            match provider.resolve_intent("list files in directory", &ctx, &[]).await {
                Ok(resp) => {
                    println!("✔ Success! ({}ms)", start.elapsed().as_millis());
                    println!("    Test command generated: '{}'", resp.command);
                }
                Err(e) => {
                    println!("⚠ Warning: API test returned: {}", e);
                    println!("    Please verify your key or model name in {:?}", saved_path);
                }
            }
        }
        Err(e) => {
            println!("⚠ Warning: {}", e);
        }
    }

    Ok(())
}
