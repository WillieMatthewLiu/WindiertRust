use clap::CommandFactory;

#[test]
fn cli_exposes_phase_one_commands() {
    let mut cmd = wd_cli::Cli::command();
    let help = cmd.render_long_help().to_string();

    assert!(help.contains("netdump"), "help did not contain netdump:\n{help}");
    assert!(
        help.contains("netfilter"),
        "help did not contain netfilter:\n{help}"
    );
    assert!(
        help.contains("flowtrack"),
        "help did not contain flowtrack:\n{help}"
    );
    assert!(
        help.contains("socketdump"),
        "help did not contain socketdump:\n{help}"
    );
    assert!(
        help.contains("reflectctl"),
        "help did not contain reflectctl:\n{help}"
    );
}
