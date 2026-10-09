// src/main.rs
use chelp::ai::create_provider_from_config;
use chelp::complete;
use chelp::config::{get_db_path, load_config};
use chelp::error::ChelpError;
use chelp::ipc::{
    cleanup_socket, send_ipc_request, spawn_daemon_detached, start_daemon, IpcRequest,
    IpcResponse, COMPLETE_BUDGET, DEFAULT_SOCKET_NAME,
};
use chelp::log::log;
use chelp::models::ShellContext;
use chelp::setup::{run_config_wizard, run_setup};
use chelp::shell::generate_hook_script;
use chelp::storage::SchemaStore;
use chelp::tui::{render_interactive_confirmation, UserAction};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "chelp",
    about = "Universal AI-Powered CLI Assistant & Autocomplete Engine",
    version = "0.1.2"
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
        /// Spawn the daemon in the background and return immediately
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
        #[arg(long)]
        out_file: Option<String>,
    },
    /// 🔑 Authenticate your terminal with CommandHelp Pro via browser
    Login {
        #[arg(long, default_value = "4321")]
        port: u16,
    },
    /// Generate shell hook script (pwsh, zsh, bash, fish)
    Init {
        shell: String,
    },
    /// Manage local CLI schema cache
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },
    /// 🛡️ Audit privacy, zero telemetry, and local data retention
    Privacy,
    /// 🔍 Parse and dump CLI command schema AST (for tool authors and debugging)
    DumpAst {
        binary: String,
        #[arg(default_value = "")]
        subcommand: String,
    },
}

#[derive(Subcommand)]
enum CacheAction {
    /// Clear all cached schemas from local database
    Clear,
    /// Show cache storage path and schema count
    Info,
}

/// Socket name shared by client and daemon (`CHELP_SOCKET` overrides it so
/// tests never fight a developer's running daemon).
fn socket_name() -> String {
    std::env::var("CHELP_SOCKET").unwrap_or_else(|_| DEFAULT_SOCKET_NAME.to_string())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        // Surface failures with their Display message instead of a Debug dump.
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

async fn run() -> Result<(), ChelpError> {
    let socket_name = socket_name();
    let cli = Cli::parse();

    match cli.command {
        Commands::Setup => run_setup().await?,
        Commands::Config => run_config_wizard().await?,

        Commands::Daemon { detached } => {
            if detached {
                // Spec §2.1/§2.2: spawn only when nothing is listening yet, so
                // every shell startup stays a no-op after the first one.
                if !chelp::ipc::is_running(&socket_name).await {
                    spawn_daemon_detached();
                }
                return Ok(());
            }
            let store = SchemaStore::new(&get_db_path())?;
            println!("chelpd listening on '{}'", socket_name);
            let handle = start_daemon(store, &socket_name).await?;
            let _ = handle.await;
            cleanup_socket(&socket_name);
        }

        Commands::Complete { buffer } => {
            let request = IpcRequest::Complete { buffer: buffer.clone() };
            match send_ipc_request(&socket_name, &request, COMPLETE_BUDGET).await {
                Ok(IpcResponse::Suggestions { lines }) => {
                    for line in lines {
                        println!("{}", line);
                    }
                }
                other => {
                    let detail = match other {
                        Ok(IpcResponse::Error { message }) => message,
                        Ok(_) => "unexpected daemon response".to_string(),
                        Err(e) => e.to_string(),
                    };
                    // Diagnostics go to the log (spec §2.3) and, when invoked by
                    // hand, to stderr — hooks silence stderr so typing never stutters.
                    log("client", &format!("complete {:?} -> {}", buffer, detail));
                    eprintln!("chelp: {}", detail);
                }
            }
        }

        Commands::Query { prompt, out_file } => {
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
                shell: std::env::var("SHELL")
                    .unwrap_or_else(|_| if cfg!(windows) { "pwsh" } else { "sh" }.to_string()),
                cwd: std::env::current_dir()?.to_string_lossy().to_string(),
            };

            // Spec §5.3: attach the parsed flags of any tool the prompt names.
            let schemas = SchemaStore::new(&get_db_path())
                .map(|store| complete::schemas_for_prompt(&store, &prompt))
                .unwrap_or_default();

            let resp = provider.resolve_intent(&prompt, &ctx, &schemas).await?;

            let (action, command) = match render_interactive_confirmation(&resp)? {
                UserAction::Run(cmd) => ("run", cmd),
                UserAction::Edit(cmd) => ("edit", cmd),
                UserAction::Cancel => return Ok(()),
            };

            match out_file {
                // Hooks read `action\ncommand`; the TUI itself never reaches them.
                Some(path) => std::fs::write(path, format!("{}\n{}", action, command))?,
                None => println!("{}", command),
            }
        }

        Commands::Login { port } => {
            let _ = chelp::auth::run_cli_login(port, 120, None).await?;
        }

        Commands::Init { shell } => {
            println!("{}", generate_hook_script(&shell)?);
        }

        Commands::Cache { action } => {
            let store = SchemaStore::new(&get_db_path())?;
            match action {
                CacheAction::Clear => {
                    let count = store.clear()?;
                    println!("Cleared {} cached CLI schema(s) from {}.", count, get_db_path().display());
                }
                CacheAction::Info => {
                    let count = store.count_schemas()?;
                    println!("Cache path: {}", get_db_path().display());
                    println!("Cached CLI schemas: {}", count);
                }
            }
        }

        Commands::Privacy => {
            println!("🛡️  CommandHelp (chelp) Privacy & Data Retention Audit");
            println!("------------------------------------------------------");
            println!("• Telemetry: ZERO. chelp makes 0 outbound pings, tracking, or update checks.");
            println!("• Offline Mode: 100% local when configured with Ollama (supports Unix socket & TCP).");
            println!("• Local Storage: Only parsed CLI flag schemas from `--help` are stored (~/.chelp/data.db).");
            println!("• Command History: NEVER logged, stored, or sent over network.");
            println!("• Credentials: Supports CHELP_API_KEY environment variable (compatible with 1Password / Bitwarden CLI).");
            println!("• Purge: Run `chelp cache clear` anytime to purge all local data.");
        }

        Commands::DumpAst { binary, subcommand } => {
            let sub: Vec<String> = if subcommand.trim().is_empty() {
                vec![]
            } else {
                subcommand.split_whitespace().map(String::from).collect()
            };
            match chelp::crawler::crawl_command_help(&binary, &sub) {
                Ok(help_text) => {
                    let schema = chelp::parser::parse_help_output(&binary, &sub, &help_text)?;
                    println!("{}", serde_json::to_string_pretty(&schema).map_err(|e| ChelpError::Parser(e.to_string()))?);
                }
                Err(e) => {
                    eprintln!("Failed to parse help for '{}': {}", binary, e);
                }
            }
        }
    }

    Ok(())
}
