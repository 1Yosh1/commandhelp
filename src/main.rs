// src/main.rs
use chelp::ai::create_provider_from_config;
use chelp::config::{get_config_dir, load_config};
use chelp::error::ChelpError;
use chelp::ipc::{send_ipc_request, start_daemon, IpcRequest, IpcResponse};
use chelp::models::ShellContext;
use chelp::setup::{run_config_wizard, run_setup};
use chelp::shell::generate_hook_script;
use chelp::storage::SchemaStore;
use chelp::tui::{render_interactive_confirmation, UserAction};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "chelp",
    about = "Universal AI-Powered CLI Assistant & Autocomplete Engine",
    version = "0.1.0"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 🚀 1-Step Setup: automatically configure shell integration & AI provider
    Setup,
    /// ⚙️ Configure your AI provider (Gemini, OpenAI, Claude, Ollama, Custom)
    Config,
    /// Start the background daemon
    Daemon {
        #[arg(long)]
        detached: bool,
    },
    /// Request completion for current command line buffer
    Complete {
        buffer: String,
    },
    /// Query the AI assistant with natural language
    Query {
        prompt: String,
    },
    /// Generate shell hook script (pwsh, zsh, bash)
    Init {
        shell: String,
    },
}

fn get_db_path() -> PathBuf {
    get_config_dir().join("data.db")
}

#[tokio::main]
async fn main() -> Result<(), ChelpError> {
    let cli = Cli::parse();
    let socket_name = "chelp-ipc";

    match cli.command {
        Commands::Setup => {
            run_setup().await?;
        }
        Commands::Config => {
            run_config_wizard().await?;
        }
        Commands::Daemon { detached: _ } => {
            let store = SchemaStore::new(&get_db_path())?;
            println!("Starting chelp daemon on {}", socket_name);
            let handle = start_daemon(store, socket_name).await?;
            handle.await.map_err(|e| ChelpError::Ipc(e.to_string()))?;
        }
        Commands::Complete { buffer } => {
            if let Ok(IpcResponse::Suggestions { flags }) = send_ipc_request(socket_name, &IpcRequest::Complete { buffer }).await {
                for f in flags {
                    if let Some(long) = f.long {
                        println!("{}\t{}", long, f.description);
                    }
                }
            }
        }
        Commands::Query { prompt } => {
            let config = load_config()?;
            let provider = match create_provider_from_config(&config.ai) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    eprintln!("Run 'chelp setup' or 'chelp config' to select an AI provider and enter your key.");
                    return Ok(());
                }
            };

            let ctx = ShellContext {
                os: std::env::consts::OS.to_string(),
                shell: std::env::var("SHELL").unwrap_or_else(|_| "pwsh".to_string()),
                cwd: std::env::current_dir()?.to_string_lossy().to_string(),
            };

            let resp = provider.resolve_intent(&prompt, &ctx, &[]).await?;

            match render_interactive_confirmation(&resp)? {
                UserAction::Run(cmd) | UserAction::Edit(cmd) => {
                    println!("{}", cmd);
                }
                UserAction::Cancel => {}
            }
        }
        Commands::Init { shell } => {
            let script = generate_hook_script(&shell)?;
            println!("{}", script);
        }
    }

    Ok(())
}
