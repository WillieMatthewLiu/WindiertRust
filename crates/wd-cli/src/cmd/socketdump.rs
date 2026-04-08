use std::process::ExitCode;

use clap::Args;

#[derive(Debug, Args)]
pub struct SocketdumpCmd {}

impl SocketdumpCmd {
    pub fn run(self) -> ExitCode {
        println!("socketdump: placeholder command surface");
        ExitCode::SUCCESS
    }
}
