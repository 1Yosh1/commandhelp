// src/main.rs
use chelp::ai::{AiProvider, GeminiProvider};
use chelp::error::ChelpError;
use chelp::ipc::{send_ipc_request, start_daemon, IpcRequest, IpcResponse};
use chelp::models::ShellContext;
use chelp::shell::generate_hook_script;
use chelp::storage::SchemaStore;
use chelp::tui::{render_interactive_confirmation, UserAction};
use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "chelp", about = "Universal AI-Powered CLI Assistant & Autocomplete Engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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
    /// Configure API keys and models
    Config,
}

fn get_db_path() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("dev", "chelp", "chelp") {
        let dir = proj_dirs.data_dir();
        let _ = std::fs::create_dir_all(dir);
        dir.join("data.db")
    } else {
        PathBuf::from("data.db")
    }
}

#[tokio::main]
async fn main() -> Result<(), ChelpError> {
    let cli = Cli::parse();
    let socket_name = "chelp-ipc";

    match cli.command {
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
            let api_key = std::env::var("GEMINI_API_KEY").unwrap_or_default();
            if api_key.is_empty() {
                eprintln!("Error: GEMINI_API_KEY environment variable is not set.");
                eprintln!("Set GEMINI_API_KEY or configure local Ollama endpoint in ~/.chelp/config.toml to enable AI queries.");
                return Ok(());
            }

            let provider = GeminiProvider::new(api_key);
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
        Commands::Config => {
            println!("Database location: {:?}", get_db_path());
            println!("Set GEMINI_API_KEY or configure local Ollama endpoint in ~/.chelp/config.toml");
        }
    }

    Ok(())
}
