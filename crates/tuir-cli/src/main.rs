//! tuir-cli — Terminal UI for Reddit (CLI entry point)

use anyhow::Result;
use clap::{Parser, Subcommand};

/// ASCII art banner for tuir
const BANNER: &str = r#"
 ██████╗ ██╗  ██╗ ██████╗  ███████╗
 ██╔════╝ ██║  ██║ ██╔══██╗ ██╔════╝
 ██║      ███████║ ██████╔╝ ███████╗
 ██║      ██╔══██║ ██╔══██╗ ╚════██║
 ╚██████╗ ██║  ██║ ██║  ██║ ███████║
  ╚═════╝ ╚═╝  ╚═╝ ╚═╝  ╚═╝ ╚══════╝
"#;

/// Terminal UI for Reddit
#[derive(Parser, Debug)]
#[command(
    name = "tuir",
    about = "Terminal UI for Reddit",
    version,
    author,
    long_version = "Rust rewrite of tuir (https://github.com/proycon/tuir)"
)]
struct Cli {
    /// Subreddit to open on startup
    #[arg(short, long)]
    subreddit: Option<String>,

    /// Disable ASCII banner
    #[arg(long)]
    no_banner: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start OAuth authentication
    Auth {
        /// Username for multi-account mode
        #[arg(short, long)]
        user: Option<String>,
    },
    /// Open inbox directly
    Inbox,
    /// Open subscriptions
    Subscriptions,
    /// List available themes
    ListThemes,
}

impl Cli {
    /// Print the ASCII banner
    fn print_banner(&self) {
        if !self.no_banner {
            println!("{}", BANNER);
            println!();
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    if cli.verbose {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .init();
    }

    match &cli.command {
        Some(Commands::Auth { user }) => {
            cli.print_banner();
            do_auth(user.as_deref())?;
        }
        Some(Commands::Inbox) => {
            cli.print_banner();
            do_inbox(cli.subreddit.as_deref())?;
        }
        Some(Commands::Subscriptions) => {
            cli.print_banner();
            do_subscriptions()?;
        }
        Some(Commands::ListThemes) => {
            cli.print_banner();
            do_list_themes()?;
        }
        None => {
            cli.print_banner();
            if let Some(sub) = &cli.subreddit {
                tracing::info!("Starting TUI with subreddit: {}", sub);
            }
            do_tui(cli.subreddit.as_deref())?;
        }
    }

    Ok(())
}

/// Enter TUI mode
fn do_tui(subreddit: Option<&str>) -> Result<()> {
    println!("[TUI] Starting terminal interface...");
    if let Some(sub) = subreddit {
        println!("[TUI] Opening r/{}", sub);
    }
    println!("[TUI] TUI not yet implemented (M3 in progress)");
    println!("[TUI] Use --help to see available commands");
    Ok(())
}

/// OAuth authentication flow
fn do_auth(user: Option<&str>) -> Result<()> {
    println!("[AUTH] Starting OAuth authentication...");
    if let Some(u) = user {
        println!("[AUTH] Username: {}", u);
    }
    println!("[AUTH] Open the following URL in your browser:");
    println!();
    println!("  https://www.reddit.com/api/v1/authorize?...");
    println!();
    println!("[AUTH] Then paste the authorization code here.");
    println!("[AUTH] (This feature is stub - see M2 for implementation)");
    Ok(())
}

/// Open inbox
fn do_inbox(subreddit: Option<&str>) -> Result<()> {
    println!("[INBOX] Opening inbox...");
    if let Some(sub) = subreddit {
        println!("[INBOX] Opening r/{} inbox", sub);
    }
    println!("[INBOX] Inbox not yet implemented");
    Ok(())
}

/// Open subscriptions
fn do_subscriptions() -> Result<()> {
    println!("[SUBS] Listing subscriptions...");
    println!("[SUBS] Subscriptions not yet implemented");
    Ok(())
}

/// List available themes
fn do_list_themes() -> Result<()> {
    println!("[THEMES] Available themes:");
    println!();
    println!("  Built-in themes:");
    println!("    - solarized-dark  (default)");
    println!("    - solarized-light");
    println!("    - molokai");
    println!("    - papercolor");
    println!();
    println!("  Use --theme=<name> to select a theme");
    Ok(())
}
