// Clippy
#![deny(clippy::unwrap_used)] // use context/with_context
#![deny(clippy::expect_used)] // use context/with_context
// Features
#![feature(slice_pattern)]
#![feature(try_blocks)]
#![feature(unwrap_infallible)]
#![feature(iter_intersperse)]
#![feature(exact_size_is_empty)]
#![feature(is_some_and)]

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::error;
use tracing_subscriber::prelude::*;
mod cloudflare;
mod cmd;
mod config;
mod inventory;
mod io;

/// Cloudflare DDNS command line utility
#[derive(Parser, Debug)]
#[clap(about, author, version, name = "cddns")]
struct Args {
    #[clap(subcommand)]
    action: Subcommands,
    /// A config file to use. [default: $XDG_CONFIG_HOME/cddns/config.toml]
    #[clap(short, long, env = "CDDNS_CONFIG", value_name = "file")]
    pub config: Option<PathBuf>,
    #[clap(short)]
    pub v: bool,
}

impl Args {
    #[tracing::instrument(level = "trace", skip(self))]
    pub async fn run(self) -> Result<()> {
        match self.action {
            Subcommands::Config(inner) => inner.run(self.config).await,
            Subcommands::Verify(inner) => inner.run(self.config).await,
            Subcommands::List(inner) => inner.run(self.config).await,
            Subcommands::Inventory(inner) => inner.run(self.config).await,
        }
    }
}

#[derive(Subcommand, Debug)]
enum Subcommands {
    Config(cmd::config::ConfigCmd),
    Verify(cmd::verify::VerifyCmd),
    List(cmd::list::ListCmd),
    Inventory(cmd::inventory::InventoryCmd),
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let verbose = args.v;

    #[cfg(windows)]
    if let Err(err) = ansi_term::enable_ansi_support() {
        eprintln!("error enabling ANSI support: {:?}", err);
    }

    // Enable tracing/logging
    tracing_subscriber::registry()
        // Filter spans based on the RUST_LOG env var.
        .with(tracing_subscriber::EnvFilter::new(if verbose {
            "error,debug"
        } else {
            "error,info"
        }))
        // Format tracing
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_level(true)
                .compact(),
        )
        // Install this registry as the global tracing registry.
        .try_init()
        .context("error initializing logging")?;

    if let Err(e) = args.run().await {
        if verbose {
            error!("{:?}", e);
        } else {
            error!("{}", e);
            eprintln!("Enable verbose logging (-v) for the full stack trace.");
        }
        std::process::exit(1);
    }
    Ok(())
}
