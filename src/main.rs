mod audio;
mod cli;
mod commands;
mod config;
mod hotkey;
mod injector;
mod overlay;
mod stt;

use clap::Parser;
use cli::Cli;
use cli::Commands;

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Run) | None => commands::run::execute(),
        Some(Commands::Start) => commands::manage::start(),
        Some(Commands::Stop) => commands::manage::stop(),
        Some(Commands::Restart) => commands::manage::restart(),
        Some(Commands::Status) => commands::manage::status(),
        Some(Commands::Config) => commands::config_menu::execute(),
        Some(Commands::Install) => commands::install::install(),
        Some(Commands::Uninstall) => commands::install::uninstall(),
    }
}
