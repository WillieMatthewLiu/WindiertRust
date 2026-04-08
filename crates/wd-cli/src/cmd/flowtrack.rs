use std::process::ExitCode;

use clap::Args;

#[derive(Debug, Args)]
pub struct FlowtrackCmd {}

impl FlowtrackCmd {
    pub fn run(self) -> ExitCode {
        println!("flowtrack: placeholder command surface");
        ExitCode::SUCCESS
    }
}
