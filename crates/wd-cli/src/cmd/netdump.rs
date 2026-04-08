use std::process::ExitCode;

use clap::Args;

#[derive(Debug, Args)]
pub struct NetdumpCmd {}

impl NetdumpCmd {
    pub fn run(self) -> ExitCode {
        println!("netdump: placeholder command surface");
        ExitCode::SUCCESS
    }
}
