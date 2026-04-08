use std::process::ExitCode;

use clap::Args;

#[derive(Debug, Args)]
pub struct ReflectctlCmd {}

impl ReflectctlCmd {
    pub fn run(self) -> ExitCode {
        println!("reflectctl: placeholder command surface");
        ExitCode::SUCCESS
    }
}
