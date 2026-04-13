//! tuir-cli — Terminal UI for Reddit (CLI entry point)

use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "tuir", about = "Terminal UI for Reddit", version, author)]
struct Args {
    /// Subreddit to open on startup (e.g. "rust", "all")
    #[arg(short, long)]
    subreddit: Option<String>,

    /// Configuration file path
    #[arg(short, long)]
    config: Option<std::path::PathBuf>,

    /// Theme name or path to theme file
    #[arg(short, long)]
    theme: Option<String>,

    /// Enable media preview in terminal
    #[arg(short, long)]
    enable_media: bool,

    /// Disable media preview
    #[arg(short, long)]
    disable_media: bool,

    /// Log to file instead of stderr
    #[arg(short, long)]
    log_file: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    println!(
        "tuir v{} — CLI stub, TUI not yet wired up",
        env!("CARGO_PKG_VERSION")
    );
    println!("Args: {args:?}");
    Ok(())
}
