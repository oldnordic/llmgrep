mod cli;
mod commands;
mod dispatch;
mod display;

#[cfg(test)]
mod cli_tests;

use clap::Parser;
use cli::{emit_error, Cli};
use dispatch::dispatch;

fn main() {
    llmgrep::platform::check_platform_support();

    let cli = Cli::parse();
    let cmd_name = dispatch::command_name(&cli);
    let tel = llmgrep::query::telemetry::TelemetryGuard::new(cmd_name);
    let tel = if cli.record { tel.with_record() } else { tel };

    let result = dispatch(&cli);

    match &result {
        Ok(()) => tel.record("ok", 0),
        Err(_) => tel.record("error", 0),
    }

    if let Err(err) = result {
        emit_error(&cli, &err);
        std::process::exit(1);
    }
}
