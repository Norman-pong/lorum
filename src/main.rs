use clap::Parser;
use lorum::commands;

/// Unified MCP configuration manager for AI coding tools.
///
/// `lorum` manages MCP (Model Context Protocol) server configurations
/// across multiple AI coding tools such as Claude Code, Codex, Proma,
/// kimi, and trae.
#[derive(Parser)]
#[command(name = "lorum", version, about)]
struct Cli {
    /// Path to a custom configuration file.
    #[arg(long = "config", global = true)]
    config: Option<String>,

    /// Enable verbose output.
    #[arg(long = "verbose", global = true)]
    verbose: bool,

    /// Subcommand to execute.
    #[command(subcommand)]
    command: Commands,
}

/// Top-level subcommands for the lorum CLI.
#[derive(clap::Subcommand)]
enum Commands {
    /// Initialise a new lorum configuration.
    Init {
        /// Create a local (project-level) configuration instead of global.
        #[arg(long)]
        local: bool,
    },

    /// Import configuration from an existing AI coding tool.
    Import {
        /// Name of the tool to import from (e.g. "claude-code", "codex").
        #[arg(long = "from")]
        from: String,
    },

    /// Synchronise MCP configuration across tools.
    Sync {
        /// Show what would change without writing anything.
        #[arg(long)]
        dry_run: bool,

        /// Only sync the specified tools (defaults to all).
        #[arg(long = "tools", num_args = 1..)]
        tools: Vec<String>,

        /// Expand environment variable references in the config.
        #[arg(long = "expand-env")]
        expand_env: bool,
    },

    /// Validate the current configuration.
    Check,

    /// Show the status of each managed tool.
    Status,

    /// Display or modify resolved configuration.
    Config {
        /// Resolve environment variables in the output.
        #[arg(long = "resolve-env")]
        resolve_env: bool,

        /// Show the local (project-level) configuration.
        #[arg(long)]
        local: bool,

        /// Show the global (user-level) configuration.
        #[arg(long)]
        global: bool,
    },

    /// Manage configuration backups.
    Backup {
        #[command(subcommand)]
        action: BackupAction,
    },

    /// Manage MCP server entries.
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },

    /// Manage lifecycle hooks.
    Hook,

    /// Manage skills.
    Skill,
}

/// Subcommands for the `backup` command.
#[derive(clap::Subcommand)]
enum BackupAction {
    /// List available backups.
    List,

    /// Restore a tool's configuration from a backup.
    Restore {
        /// Name of the tool to restore.
        tool: String,
    },
}

/// Subcommands for the `mcp` command.
#[derive(clap::Subcommand)]
enum McpAction {
    /// Add a new MCP server entry.
    Add {
        /// Server name.
        name: String,
        /// Command to start the server.
        #[arg(long)]
        command: String,
        /// Arguments for the command.
        #[arg(long = "args", num_args = 1..)]
        args: Vec<String>,
        /// Environment variables (KEY=VALUE).
        #[arg(long = "env", num_args = 1..)]
        env: Vec<String>,
    },

    /// Remove an MCP server entry.
    Remove {
        /// Server name to remove.
        name: String,
    },

    /// List configured MCP servers.
    List,

    /// Edit an existing MCP server entry.
    Edit {
        /// Server name to edit.
        name: String,
        /// New command (optional).
        #[arg(long)]
        command: Option<String>,
        /// New arguments (optional, replaces all existing args).
        #[arg(long = "args", num_args = 1..)]
        args: Option<Vec<String>>,
        /// New environment variables (optional, replaces all existing env).
        #[arg(long = "env", num_args = 1..)]
        env: Option<Vec<String>>,
    },
}

impl Cli {
    /// Parse CLI arguments and dispatch to the appropriate subcommand.
    fn run(self) -> Result<(), lorum::error::LorumError> {
        match self.command {
            Commands::Init { local } => commands::run_init(self.config.as_deref(), local),
            Commands::Import { from } => commands::run_import(&from, self.config.as_deref()),
            Commands::Sync {
                dry_run,
                tools,
                expand_env,
            } => commands::run_sync(dry_run, &tools, expand_env, self.config.as_deref()),
            Commands::Check => commands::run_check(self.config.as_deref()),
            Commands::Status => commands::run_status(self.config.as_deref()),
            Commands::Config {
                resolve_env,
                local,
                global,
            } => commands::run_config(resolve_env, local, global, self.config.as_deref()),
            Commands::Backup { action } => match action {
                BackupAction::List => commands::run_backup_list(self.config.as_deref()),
                BackupAction::Restore { tool } => {
                    commands::run_backup_restore(&tool, self.config.as_deref())
                }
            },
            Commands::Mcp { action } => match action {
                McpAction::Add {
                    name,
                    command,
                    args,
                    env,
                } => {
                    commands::mcp::run_mcp_add(&name, &command, &args, &env, self.config.as_deref())
                }
                McpAction::Remove { name } => {
                    commands::mcp::run_mcp_remove(&name, self.config.as_deref())
                }
                McpAction::List => commands::mcp::run_mcp_list(self.config.as_deref()),
                McpAction::Edit {
                    name,
                    command,
                    args,
                    env,
                } => commands::mcp::run_mcp_edit(
                    &name,
                    command.as_deref(),
                    args.as_deref(),
                    env.as_deref(),
                    self.config.as_deref(),
                ),
            },
            Commands::Hook => commands::run_hook(),
            Commands::Skill => commands::run_skill(),
        }
    }
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = cli.run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
