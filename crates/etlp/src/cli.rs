//! CLI argument parsing for the `etlp` binary.

use clap::{Parser, Subcommand};

/// embyToLocalPlayer rewritten in Rust.
#[derive(Debug, Parser)]
#[command(name = "etlp", version, about)]
pub struct Cli {
    /// Path to the configuration directory (default: current directory).
    #[arg(long, short = 'C', value_name = "DIR")]
    pub config_dir: Option<std::path::PathBuf>,

    /// Path to the configuration file (overrides --config-dir search).
    #[arg(long, value_name = "FILE")]
    pub config_file: Option<std::path::PathBuf>,

    /// HTTP listen address.
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// HTTP listen port.
    #[arg(long, short = 'p', default_value_t = 58000)]
    pub port: u16,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Optional subcommands for third-party service management.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Trigger the Trakt.tv OAuth 2.0 Device Flow and persist the token.
    TraktAuth,
    /// Mark an episode as watched on Bangumi (bgm.tv).
    BgmMarkPlayed {
        /// Bangumi subject ID (the series, e.g. `363612`).
        #[arg(long)]
        subject_id: u64,
        /// Episode sort number (1-based) to mark.
        #[arg(long)]
        ep: u32,
        /// Access token (falls back to the value in the config).
        #[arg(long)]
        token: Option<String>,
    },
}
