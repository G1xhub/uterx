//! uterx — Terminal Desktop
//!
//! Main entry point: CLI parsing, config loading, terminal setup, event loop.

mod config;
mod event_loop;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "uterx", version, about = "uterx — Terminal Desktop")]
struct Cli {
    /// Path to config file (default: ~/.uterx/config.toml)
    #[arg(short, long)]
    config: Option<String>,

    /// Disable automatic session restore on launch
    #[arg(long = "no-restore")]
    no_restore: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage plugins
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
}

#[derive(Subcommand)]
enum PluginAction {
    /// Add/install a plugin
    Add {
        /// Plugin name or URL
        source: String,
    },
    /// Remove a plugin
    Remove {
        /// Plugin name
        name: String,
    },
    /// List installed plugins
    List,
    /// Update plugins
    Update {
        /// Plugin name (or omit for all)
        name: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("uterx=info".parse()?),
        )
        .init();

    let cli = Cli::parse();

    // Load configuration
    let cfg = config::AppConfig::load(cli.config.as_deref())?;

    // Handle subcommands
    match cli.command {
        Some(Commands::Plugin { action }) => {
            handle_plugin_command(action, &cfg)?;
            return Ok(());
        }
        None => {}
    }

    // Start the terminal UI
    tracing::info!("starting uterx terminal desktop");
    event_loop::run(cfg, !cli.no_restore).await
}

fn handle_plugin_command(
    action: PluginAction,
    cfg: &config::AppConfig,
) -> anyhow::Result<()> {
    let mut manager =
        uterx_plugin::PluginManager::new(cfg.plugins_dir());
    manager.scan()?;

    match action {
        PluginAction::Add { source } => {
            let installed = manager.install(&source)?;
            println!(
                "Installed plugin: {} v{}",
                installed.name, installed.version
            );
        }
        PluginAction::Remove { name } => {
            manager.remove(&name)?;
            println!("Removed plugin: {}", name);
        }
        PluginAction::List => {
            let plugins = manager.list();
            if plugins.is_empty() {
                println!("No plugins installed.");
            } else {
                for p in plugins {
                    println!(
                        "  {} v{} — {}",
                        p.name,
                        p.version,
                        p.description.as_deref().unwrap_or("")
                    );
                }
            }
        }
        PluginAction::Update { name } => {
            let report = manager.update(name.as_deref())?;
            if report.updated.is_empty() && report.skipped.is_empty() {
                println!("No plugins installed.");
            } else {
                for entry in report.updated {
                    println!("Updated plugin: {}", entry);
                }
                for entry in report.skipped {
                    println!("Skipped plugin: {}", entry);
                }
            }
        }
    }
    Ok(())
}
