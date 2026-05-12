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
    command: Option<Commands>,

    /// Skip the welcome message when no config exists.
    #[arg(long = "no-welcome", global = true)]
    no_welcome: bool,
}

/// Top-level subcommands for the lorum CLI.
#[derive(clap::Subcommand)]
enum Commands {
    /// Initialise a new lorum configuration.
    Init {
        /// Create a local (project-level) configuration instead of global.
        #[arg(long)]
        local: bool,

        /// Skip interactive prompts and auto-import detected tools.
        #[arg(long)]
        yes: bool,
    },

    /// Import configuration from an existing AI coding tool.
    Import {
        /// Name of the tool to import from (e.g. "claude-code", "codex").
        #[arg(long = "from")]
        from: String,

        /// Show what would be imported without writing anything.
        #[arg(long)]
        dry_run: bool,
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

    /// Manage project-level AI coding rules.
    Rule {
        #[command(subcommand)]
        action: RuleAction,
    },

    /// Manage lifecycle hooks.
    Hook {
        #[command(subcommand)]
        action: HookAction,
    },

    /// Manage AI skills.
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
}

/// Subcommands for the `backup` command.
#[derive(clap::Subcommand)]
enum BackupAction {
    /// List available backups.
    List,

    /// Create a backup for one or more tools.
    Create {
        /// Tool name(s) to backup.
        #[arg(num_args = 0..)]
        tools: Vec<String>,
        /// Backup all tools.
        #[arg(long)]
        all: bool,
    },

    /// Restore a tool's configuration from a backup.
    Restore {
        /// Name of the tool to restore.
        tool: String,
        /// Specific backup file to restore from.
        #[arg(long)]
        backup: Option<String>,
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

/// Subcommands for the `rule` command.
#[derive(clap::Subcommand)]
enum RuleAction {
    /// Create an empty .lorum/RULES.md template.
    Init,

    /// Add a new rule section.
    Add {
        /// Section name (## heading).
        name: String,
        /// Section content.
        #[arg(long)]
        content: String,
    },

    /// Remove a rule section by name.
    Remove {
        /// Section name to remove.
        name: String,
    },

    /// Edit an existing rule section's content.
    Edit {
        /// Section name to edit.
        name: String,
        /// New section content (replaces existing).
        #[arg(long)]
        content: String,
    },

    /// List all rule section names.
    List,

    /// Display rule content.
    Show {
        /// Section name to show (omit to show all).
        name: Option<String>,
    },

    /// Synchronise rules to target tools.
    Sync {
        /// Show what would change without writing.
        #[arg(long)]
        dry_run: bool,
        /// Only sync the specified tools (defaults to all).
        #[arg(long = "tools", num_args = 1..)]
        tools: Vec<String>,
    },

    /// Import rules from a target tool into .lorum/RULES.md.
    Import {
        /// Tool name to import from (cursor, windsurf, codex).
        #[arg(long = "from")]
        from: String,
    },
}

/// Subcommands for the `skill` command.
#[derive(clap::Subcommand)]
enum SkillAction {
    /// List all skills in the unified directory.
    List,

    /// Show the content of a skill.
    Show {
        /// Skill name.
        name: String,
    },

    /// Import a skill directory into the unified storage.
    Add {
        /// Skill name (must match frontmatter name).
        name: String,
        /// Source directory path.
        #[arg(long = "from")]
        from: String,
    },

    /// Remove a skill from the unified storage.
    Remove {
        /// Skill name to remove.
        name: String,
    },

    /// Synchronise skills to target tools.
    Sync {
        /// Show what would change without writing.
        #[arg(long)]
        dry_run: bool,
        /// Only sync the specified tools (defaults to all).
        #[arg(long = "tools", num_args = 1..)]
        tools: Vec<String>,
    },
}

/// Subcommands for the `hook` command.
#[derive(clap::Subcommand)]
enum HookAction {
    /// Add a hook handler for an event.
    Add {
        /// Event name (e.g., pre-tool-use, post-tool-use).
        event: String,
        /// Matcher pattern (e.g., tool name or regex).
        #[arg(long)]
        matcher: String,
        /// Command to execute.
        #[arg(long)]
        command: String,
        /// Optional timeout in seconds.
        #[arg(long)]
        timeout: Option<u64>,
        /// Handler type: command, http, prompt, agent, mcp_tool.
        #[arg(long)]
        handler_type: Option<String>,
    },

    /// Remove a hook handler or an entire event.
    Remove {
        /// Event name to remove from.
        event: String,
        /// Specific matcher to remove (omit to remove entire event).
        #[arg(long)]
        matcher: Option<String>,
    },

    /// List all configured hooks.
    List,

    /// Synchronise hooks to target tools.
    Sync {
        /// Show what would change without writing.
        #[arg(long)]
        dry_run: bool,
        /// Only sync the specified tools (defaults to all).
        #[arg(long = "tools", num_args = 1..)]
        tools: Vec<String>,
    },
}

impl Cli {
    /// Parse CLI arguments and dispatch to the appropriate subcommand.
    fn run(self) -> Result<(), lorum::error::LorumError> {
        match self.command {
            None => {
                let config_path = self.config.as_deref().map(std::path::Path::new);
                let has_config = lorum::config::resolve_effective_config_from_cwd(config_path)
                    .map(|cfg| {
                        !cfg.mcp.servers.is_empty()
                            || !cfg.hooks.events.is_empty()
                            || lorum::config::find_project_config(
                                &std::env::current_dir().unwrap_or_default(),
                            )
                            .is_some()
                    })
                    .unwrap_or(false);
                if !has_config && !self.no_welcome {
                    println!("Welcome to lorum! You haven't created a configuration file yet.\n");
                    println!("Quick start:");
                    println!("  lorum init              Create initial config");
                    println!("  lorum init --local      Create project-level config");
                    println!("  lorum import --from all Import from existing tools");
                    std::process::exit(0);
                }
                std::process::exit(1);
            }
            Some(Commands::Init { local, yes }) => {
                commands::run_init(self.config.as_deref(), local, yes)
            }
            Some(Commands::Import { from, dry_run }) => {
                commands::run_import(&from, dry_run, self.config.as_deref())
            }
            Some(Commands::Sync {
                dry_run,
                tools,
                expand_env,
            }) => commands::run_sync(dry_run, &tools, expand_env, self.config.as_deref()),
            Some(Commands::Check) => commands::run_check(self.config.as_deref()),
            Some(Commands::Status) => commands::run_status(self.config.as_deref()),
            Some(Commands::Config {
                resolve_env,
                local,
                global,
            }) => commands::run_config(resolve_env, local, global, self.config.as_deref()),
            Some(Commands::Backup { action }) => match action {
                BackupAction::List => commands::run_backup_list(self.config.as_deref()),
                BackupAction::Create { tools, all } => {
                    commands::run_backup_create(&tools, all, self.config.as_deref())
                }
                BackupAction::Restore { tool, backup } => {
                    commands::run_backup_restore(&tool, backup.as_deref(), self.config.as_deref())
                }
            },
            Some(Commands::Mcp { action }) => match action {
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
            Some(Commands::Rule { action }) => match action {
                RuleAction::Init => commands::rule::run_rule_init(),
                RuleAction::Add { name, content } => commands::rule::run_rule_add(&name, &content),
                RuleAction::Remove { name } => commands::rule::run_rule_remove(&name),
                RuleAction::Edit { name, content } => {
                    commands::rule::run_rule_edit(&name, &content)
                }
                RuleAction::List => commands::rule::run_rule_list(),
                RuleAction::Show { name } => commands::rule::run_rule_show(name.as_deref()),
                RuleAction::Sync { dry_run, tools } => {
                    commands::rule::run_rule_sync(dry_run, &tools)
                }
                RuleAction::Import { from } => commands::rule::run_rule_import(&from),
            },
            Some(Commands::Hook { action }) => match action {
                HookAction::Add {
                    event,
                    matcher,
                    command,
                    timeout,
                    handler_type,
                } => commands::hook::run_hook_add(
                    &event,
                    &matcher,
                    &command,
                    timeout,
                    handler_type.as_deref(),
                    self.config.as_deref(),
                ),
                HookAction::Remove { event, matcher } => commands::hook::run_hook_remove(
                    &event,
                    matcher.as_deref(),
                    self.config.as_deref(),
                ),
                HookAction::List => commands::hook::run_hook_list(self.config.as_deref()),
                HookAction::Sync { dry_run, tools } => {
                    commands::hook::run_hook_sync(dry_run, &tools, self.config.as_deref())
                }
            },
            Some(Commands::Skill { action }) => match action {
                SkillAction::List => commands::skill::run_skill_list(None),
                SkillAction::Show { name } => commands::skill::run_skill_show(&name, None),
                SkillAction::Add { name, from } => {
                    commands::skill::run_skill_add(&name, &from, None)
                }
                SkillAction::Remove { name } => commands::skill::run_skill_remove(&name, None),
                SkillAction::Sync { dry_run, tools } => {
                    commands::skill::run_skill_sync(dry_run, &tools, None)
                }
            },
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
