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
    version = "0.1.1"
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
    /// 🔑 Authenticate your terminal with CommandHelp Pro via browser
    Login {
        #[arg(long, default_value = "4321")]
        port: u16,
    },
    /// Generate shell hook script (pwsh, zsh, bash)
    Init {
        shell: String,
    },
    /// 📚 Manage team & local command runbooks and recipes
    Recipe {
        #[command(subcommand)]
        action: RecipeCommands,
    },
    /// 🔍 Index workspace scripts, Makefiles, package.json for zero-latency completion
    IndexRepo {
        #[arg(long, default_value = ".")]
        path: String,
    },
}

#[derive(Subcommand)]
enum RecipeCommands {
    /// 📋 List all available recipes (workspace + global)
    List,
    /// ➕ Add a new command recipe to workspace (.chelp/recipes.toml) or global
    Add {
        name: String,
        #[arg(short, long)]
        cmd: String,
        #[arg(short, long)]
        desc: String,
        #[arg(short, long)]
        tag: Vec<String>,
        #[arg(long)]
        global: bool,
    },
    /// ▶️ Run a command recipe with interactive confirmation
    Run {
        name: String,
    },
    /// 🗑️ Remove a command recipe by name
    Remove {
        name: String,
        #[arg(long)]
        global: bool,
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
        Commands::Login { port } => {
            let _ = chelp::auth::run_cli_login(port, 120, None).await?;
        }
        Commands::Init { shell } => {
            let script = generate_hook_script(&shell)?;
            println!("{}", script);
        }
        Commands::Recipe { action } => {
            let cwd = std::env::current_dir()?;
            match action {
                RecipeCommands::List => {
                    let list = chelp::recipes::load_recipes(Some(&cwd))?;
                    if list.is_empty() {
                        println!("No recipes found. Create one with 'chelp recipe add <name> --cmd <command> --desc <description>'");
                    } else {
                        println!("Available Runbook Recipes:");
                        println!("{:-<60}", "");
                        for r in list {
                            let (safety, _) = r.resolved_safety();
                            let safety_str = match safety {
                                chelp::models::SafetyLevel::Safe => "SAFE",
                                chelp::models::SafetyLevel::Caution => "CAUTION",
                                chelp::models::SafetyLevel::Destructive => "HIGH RISK / DESTRUCTIVE",
                            };
                            println!("• {}", r.name);
                            println!("  Command:     {}", r.command);
                            println!("  Description: {}", r.description);
                            if !r.tags.is_empty() {
                                println!("  Tags:        {}", r.tags.join(", "));
                            }
                            println!("  Safety:      {}", safety_str);
                            println!();
                        }
                    }
                }
                RecipeCommands::Add {
                    name,
                    cmd,
                    desc,
                    tag,
                    global,
                } => {
                    let recipe = chelp::recipes::Recipe {
                        name: name.clone(),
                        command: cmd,
                        description: desc,
                        tags: tag,
                        safety_level: None,
                    };
                    let saved_path = chelp::recipes::save_recipe(recipe, global, Some(&cwd))?;
                    println!("✔ Saved recipe '{}' to {}", name, saved_path.display());
                }
                RecipeCommands::Run { name } => {
                    if let Some(cmd) = chelp::recipes::run_recipe(&name, Some(&cwd))? {
                        println!("{}", cmd);
                    }
                }
                RecipeCommands::Remove { name, global } => {
                    if chelp::recipes::remove_recipe(&name, global, Some(&cwd))? {
                        println!("✔ Removed recipe '{}'", name);
                    } else {
                        println!("Recipe '{}' not found.", name);
                    }
                }
            }
        }
        Commands::IndexRepo { path } => {
            let store = SchemaStore::new(&get_db_path())?;
            let target_path = std::path::Path::new(&path);
            let count = chelp::repo_indexer::index_workspace(target_path, &store)?;
            println!("✔ Indexed {} project targets/scripts into local store.", count);
        }
    }

    Ok(())
}
