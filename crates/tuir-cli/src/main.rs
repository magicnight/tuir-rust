//! tuir-cli — Terminal UI for Reddit (CLI entry point)

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::event::{self, Event, KeyEventKind};
use std::time::Duration;
use tuir_tui::pages::subreddit::SubredditPage;
use tuir_tui::pages::Page;
use tuir_tui::terminal::{init, restore};

/// ASCII art banner for tuir
const BANNER: &str = r#"
    ██╗  ██╗ ██████╗ ██╗     ██╗      █████╗ ████████╗██╗ ██████╗ ███╗   ██╗
    ██║  ██║██╔═══██╗██║     ██║     ██╔══██╗╚══██╔══╝██║██╔═══██╗████╗  ██║
    ███████║██║   ██║██║     ██║     ███████║   ██║   ██║██║   ██║██╔██╗ ██║
    ██╔══██║██║   ██║██║     ██║     ██╔══██║   ██║   ██║██║   ██║██║╚██╗██║
    ██║  ██║╚██████╔╝███████╗███████╗██║  ██║   ██║   ██║╚██████╔╝██║ ╚████║
    ╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚══════╝╚═╝  ╚═╝   ╚═╝   ╚═╝ ╚═════╝ ╚═╝  ╚═══╝

     █████╗  ██████╗ ██████╗███████╗███████╗███████╗     ██████╗ ██╗   ██╗███████╗██████╗ 
    ██╔══██╗██╔════╝██╔════╝██╔════╝██╔════╝██╔════╝    ██╔═══██╗██║   ██║██╔════╝██╔══██╗
    ███████║██║     ██║     █████╗  ███████╗███████╗    ██║   ██║██║   ██║█████╗  ██████╔╝
    ██╔══██║██║     ██║     ██╔══╝  ╚════██║╚════██║    ██║   ██║╚██╗ ██╔╝██╔══╝  ██╔══██╗
    ██║  ██║╚██████╗╚██████╗███████╗███████║███████║    ╚██████╔╝ ╚████╔╝ ███████╗██║  ██║
    ╚═╝  ╚═╝ ╚═════╝ ╚═════╝╚══════╝╚══════╝╚══════╝     ╚═════╝   ╚═══╝  ╚══════╝╚═╝  ╚═╝
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

    // Initialize terminal
    let mut terminal = init()?;

    // Create and load subreddit page
    let mut page = SubredditPage::new(subreddit.unwrap_or(""));

    // Load data synchronously using blocking tokio runtime
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        page.load().await;
    });

    // Event loop
    loop {
        // Draw
        terminal.draw(|f| {
            page.render(f);
        })?;

        // Poll for input
        match event::poll(Duration::from_millis(100)) {
            Ok(true) => {
                if let Ok(Event::Key(key)) = event::read() {
                    if key.kind == KeyEventKind::Press {
                        let action = page.handle_key(key);
                        if action == tuir_tui::pages::PageAction::Quit {
                            break;
                        }
                    }
                }
            }
            Ok(false) => {}
            Err(_) => break,
        }
    }

    // Restore terminal
    restore()?;

    println!("[TUI] Exited TUI mode");
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
