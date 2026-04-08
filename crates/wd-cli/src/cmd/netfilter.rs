use std::process::ExitCode;

use clap::Args;

#[derive(Debug, Args)]
pub struct NetfilterCmd {}

impl NetfilterCmd {
    pub fn run(self) -> ExitCode {
        println!("netfilter: placeholder command surface");
        ExitCode::SUCCESS
    }
}
