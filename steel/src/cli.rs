//! Command-line interface for the `steel` binary.
//!
//! With no subcommand the binary runs as a Minecraft server (the default and
//! historical behavior). Subcommands are operator-facing maintenance tools
//! that run synchronously without spinning up the runtimes.

use std::path::Path;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use steel_core::network::ip_access_policy::{
    IP_ACCESS_POLICY, STEEL_IP_BANS_PATH, VANILLA_BANNED_IPS_PATH, init_ip_access_policy,
    read_vanilla_banned_ips_file,
};

#[derive(Parser, Debug)]
#[command(name = "steel", version, about = "Steel Minecraft server", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Convert data from vanilla formats to Steel formats.
    Convert(ConvertArgs),
}

#[derive(Args, Debug)]
pub struct ConvertArgs {
    /// What to convert.
    #[arg(value_enum)]
    pub kind: ConvertKind,

    /// Overwrite Steel's existing data instead of refusing.
    #[arg(long)]
    pub force: bool,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum ConvertKind {
    /// Vanilla `config/banned-ips.json` → Steel `config/ip-bans.toml`.
    BannedIp,
}

impl Command {
    pub fn run(self) -> ExitCode {
        match self {
            Self::Convert(args) => match args.kind {
                ConvertKind::BannedIp => convert_banned_ip(args.force),
            },
        }
    }
}

/// Imports vanilla's `banned-ips.json` into Steel's `ip-bans.toml`.
///
/// Loads any existing Steel bans first so the shadowban list is preserved.
/// Refuses to overwrite a non-empty existing ban list unless `force` is set.
fn convert_banned_ip(force: bool) -> ExitCode {
    init_ip_access_policy(&[]);
    let policy = &*IP_ACCESS_POLICY;

    let existing = policy.get_banned_ips();
    if !existing.is_empty() && !force {
        eprintln!(
            "{STEEL_IP_BANS_PATH} already contains {} ban entr{}; refusing to overwrite. Re-run with --force to replace them.",
            existing.len(),
            if existing.len() == 1 { "y" } else { "ies" }
        );
        return ExitCode::FAILURE;
    }

    let imported = match read_vanilla_banned_ips_file(Path::new(VANILLA_BANNED_IPS_PATH)) {
        Ok(Some(bans)) => bans,
        Ok(None) => {
            eprintln!("vanilla bans file {VANILLA_BANNED_IPS_PATH} not found");
            return ExitCode::FAILURE;
        }
        Err(e) => {
            eprintln!("failed to read {VANILLA_BANNED_IPS_PATH}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let count = imported.len();
    policy.replace_banned_ips(imported);
    policy.save_config();
    println!("Converted {count} entries from {VANILLA_BANNED_IPS_PATH} into {STEEL_IP_BANS_PATH}");
    ExitCode::SUCCESS
}
