use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "simplestt",
    version,
    about = "Minimal speech-to-text overlay",
    long_about = "Press F9 to toggle recording. Speak naturally. Text is typed into the active window."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Run overlay (default)", alias = "r")]
    Run,

    #[command(about = "Start in background")]
    Start,

    #[command(about = "Stop running instance")]
    Stop,

    #[command(about = "Restart (stop + start)")]
    Restart,

    #[command(about = "Show running status")]
    Status,

    #[command(about = "Interactive configuration menu")]
    Config,

    #[command(about = "Install globally to PATH")]
    Install,

    #[command(about = "Uninstall from system")]
    Uninstall,
}
