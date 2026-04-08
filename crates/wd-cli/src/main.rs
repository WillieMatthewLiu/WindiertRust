use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = wd_cli::Cli::parse();
    cli.run()
}
