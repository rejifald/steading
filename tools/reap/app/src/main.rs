//! One binary (ADR 0001 rule 14): with a subcommand it is the CLI, without one it is the
//! menu-bar app.

mod commands;
mod logic;
mod osascript;
mod state;
mod tray;
mod window;
mod worker;

fn main() {
    // Finder and launchd may pass `-psn_…`; anything else is a CLI call.
    let args: Vec<_> = std::env::args_os().collect();
    let is_cli = args
        .iter()
        .skip(1)
        .any(|arg| !arg.to_string_lossy().starts_with("-psn_"));
    if is_cli {
        std::process::exit(reap_core::cli::run(args));
    }
    tray::run();
}
