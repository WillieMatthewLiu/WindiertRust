use std::process::ExitCode;

use clap::{Parser, Subcommand};

pub mod cmd {
    pub mod common;
    pub mod flowtrack;
    pub mod netdump;
    pub mod netfilter;
    pub mod reflectctl;
    pub mod socketdump;
}
pub mod error;
pub mod fixtures;
pub mod output;
pub mod runtime;

#[derive(Debug, Parser)]
#[command(name = "wd-cli", about = "WinDivert phase-one tooling surface")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    pub fn run(self) -> ExitCode {
        self.command.run()
    }
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Netdump(cmd::netdump::NetdumpCmd),
    Netfilter(cmd::netfilter::NetfilterCmd),
    Flowtrack(cmd::flowtrack::FlowtrackCmd),
    Socketdump(cmd::socketdump::SocketdumpCmd),
    Reflectctl(cmd::reflectctl::ReflectctlCmd),
}

impl Commands {
    fn run(self) -> ExitCode {
        match self {
            Commands::Netdump(cmd) => cmd.run(),
            Commands::Netfilter(cmd) => cmd.run(),
            Commands::Flowtrack(cmd) => cmd.run(),
            Commands::Socketdump(cmd) => cmd.run(),
            Commands::Reflectctl(cmd) => cmd.run(),
        }
    }
}
