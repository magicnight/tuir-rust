//! tuir-cli — Terminal UI for Reddit (CLI entry point)

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, Event, KeyEventKind},
    terminal::size as terminal_size,
};
use std::sync::Arc;
use std::time::Duration;
use tuir_core::oauth::StoredToken;
use tuir_core::reddit::{MockRedditClient, RedditApi, RedditClient};
use tuir_core::{config::Config, OAuth};
use tuir_tui::pages::{
    help::HelpPage, inbox::InboxPage, message::MessagePage, submission::SubmissionPage,
    subreddit::SubredditPage, subscription::SubscriptionPage, Page, PageAction, PageKind,
};
use tuir_tui::terminal::{init, restore, TerminalType};

const HERO_PIXEL_WIDTH: usize = 2;
const COMPACT_PIXEL_WIDTH: usize = 2;
const MIN_3D_WIDTH: u16 = 72;
const HERO_TOP_ROWS: [&str; 5] = [
    "########  ##  ##  ####  ###### ",
    "   ##     ##  ##   ##   ##   ##",
    "   ##     ##  ##   ##   #####  ",
    "   ##     ##  ##   ##   ## ##  ",
    "   ##      ####   ####  ##  ## ",
];

const HERO_BOTTOM_ROWS: [&str; 5] = [
    "######   ##  ##   ####   ########",
    "##   ##  ##  ##  ##         ##   ",
    "######   ##  ##   ####      ##   ",
    "## ##    ##  ##      ##     ##   ",
    "##  ##    ####    ####      ##   ",
];

const COMPACT_TOP_ROWS: [&str; 3] = [
    "##### ## ## ### #### ",
    "  #   ## ##  #  ## ##",
    "  #    ###  ### ## ##",
];

const COMPACT_BOTTOM_ROWS: [&str; 3] = [
    "#### ## ## ### ####",
    "## # ## ##  ##  ## ",
    "## ## ###  ###  ## ",
];

fn banner_for_width(width: Option<u16>) -> String {
    match width {
        Some(width) if width >= MIN_3D_WIDTH => {
            render_banner(&HERO_TOP_ROWS, &HERO_BOTTOM_ROWS, HERO_PIXEL_WIDTH)
        }
        _ => render_banner(&COMPACT_TOP_ROWS, &COMPACT_BOTTOM_ROWS, COMPACT_PIXEL_WIDTH),
    }
}

#[cfg(test)]
fn longest_visible_line_width(banner: &str) -> usize {
    banner.lines().map(visible_line_width).max().unwrap_or_default()
}

#[cfg(test)]
fn visible_line_width(line: &str) -> usize {
    let bytes = line.as_bytes();
    let mut width = 0;
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == 0x1b && bytes.get(index + 1) == Some(&b'[') {
            index += 2;
            while index < bytes.len() && !bytes[index].is_ascii_alphabetic() {
                index += 1;
            }
            if index < bytes.len() {
                index += 1;
            }
            continue;
        }

        width += 1;
        index += 1;
    }

    width
}

fn render_banner(top_rows: &[&str], bottom_rows: &[&str], pixel_width: usize) -> String {
    let mut banner = String::from("\n");
    banner.push_str(&render_block_rows(top_rows, 215, 172, pixel_width));
    banner.push('\n');
    banner.push_str(&render_block_rows(bottom_rows, 196, 160, pixel_width));
    banner.push('\n');
    banner.push_str("\x1b[38;5;220m  Terminal UI for Reddit -- Rust Rewrite\x1b[0m\n\n");
    banner
}

fn render_block_rows(rows: &[&str], face_color: u8, shadow_color: u8, pixel_width: usize) -> String {
    let mut rendered = String::new();
    for row in rows {
        rendered.push_str(&render_mask_row(row, face_color, pixel_width, 0));
        rendered.push('\n');
        rendered.push_str(&render_mask_row(row, shadow_color, pixel_width, 1));
        rendered.push('\n');
    }

    rendered
}

fn render_mask_row(row: &str, color: u8, pixel_width: usize, indent_cells: usize) -> String {
    let mut rendered = " ".repeat(indent_cells * pixel_width);

    for cell in row.as_bytes() {
        if *cell == b'#' {
            rendered.push_str(&pixel_fill(color, pixel_width));
        } else {
            rendered.push_str(&" ".repeat(pixel_width));
        }
    }

    rendered
}

fn pixel_fill(color: u8, pixel_width: usize) -> String {
    format!("\x1b[38;5;{color}m{}\x1b[0m", "#".repeat(pixel_width))
}

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
            let width = terminal_size().ok().map(|(width, _)| width);
            println!("{}", banner_for_width(width));
            println!();
        }
    }
}

struct TerminalRestoreGuard;

impl Drop for TerminalRestoreGuard {
    fn drop(&mut self) {
        let _ = restore();
    }
}

enum AppPage {
    Subreddit(SubredditPage),
    Submission(SubmissionPage),
    Message(MessagePage),
    Inbox(InboxPage),
    Subscription(SubscriptionPage),
    Help(HelpPage),
}

impl AppPage {
    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> PageAction {
        match self {
            Self::Subreddit(page) => page.handle_key(key),
            Self::Submission(page) => page.handle_key(key),
            Self::Message(page) => page.handle_key(key),
            Self::Inbox(page) => page.handle_key(key),
            Self::Subscription(page) => page.handle_key(key),
            Self::Help(page) => page.handle_key(key),
        }
    }

    fn is_help(&self) -> bool {
        matches!(self, Self::Help(_))
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

/// Build a Reddit client backend based on config + persisted token state.
///
/// Returns a real [`RedditClient`] with a fresh access token when a valid
/// token is on disk; otherwise falls back to [`MockRedditClient`] so the TUI
/// still starts without credentials.
fn build_client(config: &Config) -> Arc<dyn RedditApi> {
    let token_path = Config::token_file();
    if !token_path.exists() {
        tracing::debug!("no token file at {:?}, using mock client", token_path);
        return Arc::new(MockRedditClient::new());
    }

    let token = match StoredToken::load(&token_path) {
        Ok(token) => token,
        Err(err) => {
            tracing::warn!("failed to load token file: {err}; falling back to mock");
            return Arc::new(MockRedditClient::new());
        }
    };

    if token.is_expired() {
        tracing::warn!("stored token is expired; falling back to mock (refresh not wired yet)");
        return Arc::new(MockRedditClient::new());
    }

    if config.reddit.oauth_client_id.as_deref().unwrap_or("").is_empty() {
        tracing::warn!("oauth_client_id missing from config; falling back to mock");
        return Arc::new(MockRedditClient::new());
    }

    let mut client = RedditClient::new();
    client.set_token(token.access_token);
    tracing::info!("using real Reddit client backed by stored token");
    Arc::new(client)
}

/// Enter TUI mode
fn do_tui(subreddit: Option<&str>) -> Result<()> {
    println!("[TUI] Starting terminal interface...");
    if let Some(sub) = subreddit {
        println!("[TUI] Opening r/{}", sub);
    }

    let config = Config::load().unwrap_or_default();
    let client = build_client(&config);
    let mut page = SubredditPage::with_client(subreddit.unwrap_or(""), client);
    page.load_sync();
    run_app(AppPage::Subreddit(page))?;

    println!("[TUI] Exited TUI mode");
    Ok(())
}

/// OAuth authentication flow
fn do_auth(user: Option<&str>) -> Result<()> {
    let config = Config::load()?;
    println!("{}", auth_output(&config, user));
    Ok(())
}

fn auth_output(config: &Config, user: Option<&str>) -> String {
    let mut lines = vec!["[AUTH] Starting OAuth authentication...".to_string()];

    if let Some(user) = user {
        lines.push(format!("[AUTH] Username: {user}"));
    }

    let client_id = config.reddit.oauth_client_id.clone().unwrap_or_default();
    if client_id.trim().is_empty() {
        lines.push("[AUTH] OAuth is not configured.".to_string());
        lines.push(format!(
            "[AUTH] Add `oauth_client_id` to {}/tuir.cfg",
            Config::config_dir().display()
        ));
        lines.push("[AUTH] Minimum config keys: oauth_client_id, oauth_redirect_uri, oauth_scope".to_string());
        lines.push(format!(
            "[AUTH] Token file will be stored at {}",
            Config::token_file().display()
        ));
        return lines.join("\n");
    }

    let scopes = config
        .reddit
        .oauth_scope
        .split(',')
        .map(str::trim)
        .filter(|scope| !scope.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    let oauth = OAuth::new(
        client_id,
        config.reddit.oauth_redirect_uri.clone(),
        scopes,
    );

    lines.push(format!(
        "[AUTH] Redirect URI: {}",
        config.reddit.oauth_redirect_uri
    ));
    lines.push(format!(
        "[AUTH] Token file: {}",
        Config::token_file().display()
    ));
    lines.push("[AUTH] Open the following URL in your browser:".to_string());
    lines.push(String::new());
    lines.push(format!("  {}", oauth.auth_url()));
    lines.push(String::new());
    lines.push("[AUTH] Token exchange is not implemented yet; this command currently validates config and prepares the browser step.".to_string());

    lines.join("\n")
}

/// Open inbox
fn do_inbox(subreddit: Option<&str>) -> Result<()> {
    println!("[INBOX] Opening inbox...");
    if let Some(sub) = subreddit {
        println!("[INBOX] Ignoring subreddit filter for inbox: r/{}", sub);
    }

    let config = Config::load().unwrap_or_default();
    let client = build_client(&config);
    let mut page = InboxPage::with_client(client);
    page.load_sync();
    run_app(AppPage::Inbox(page))?;
    Ok(())
}

/// Open subscriptions
fn do_subscriptions() -> Result<()> {
    println!("[SUBS] Listing subscriptions...");

    let config = Config::load().unwrap_or_default();
    let client = build_client(&config);
    let mut page = SubscriptionPage::with_client(client);
    page.load_sync();
    run_app(AppPage::Subscription(page))?;
    Ok(())
}

fn run_app(initial_page: AppPage) -> Result<()> {
    let mut terminal = init()?;
    let _restore_guard = TerminalRestoreGuard;

    event_loop(&mut terminal, initial_page)
}

fn event_loop(terminal: &mut TerminalType, initial_page: AppPage) -> Result<()> {
    let mut stack = vec![initial_page];

    loop {
        let Some(current_page) = stack.last_mut() else {
            break;
        };

        terminal.draw(|f| {
            match current_page {
                AppPage::Subreddit(page) => page.render(f),
                AppPage::Submission(page) => page.render(f),
                AppPage::Message(page) => page.render(f),
                AppPage::Inbox(page) => page.render(f),
                AppPage::Subscription(page) => page.render(f),
                AppPage::Help(page) => page.render(f),
            }
        })?;

        match event::poll(Duration::from_millis(100)) {
            Ok(true) => {
                if let Ok(Event::Key(key)) = event::read() {
                    if key.kind == KeyEventKind::Press {
                        if is_help_shortcut(&key) && !current_page.is_help() {
                            stack.push(AppPage::Help(HelpPage::new()));
                            continue;
                        }
                        let action = current_page.handle_key(key);
                        match action {
                            PageAction::Quit => break,
                            PageAction::Back => {
                                if stack.len() > 1 {
                                    stack.pop();
                                } else {
                                    break;
                                }
                            }
                            PageAction::Switch(kind) => {
                                if let Some(next_page) = build_switched_page(current_page, kind) {
                                    stack.push(next_page);
                                }
                            }
                            PageAction::None => {}
                        }
                    }
                }
            }
            Ok(false) => {}
            Err(_) => break,
        }
    }

    Ok(())
}

fn is_help_shortcut(key: &crossterm::event::KeyEvent) -> bool {
    matches!(
        key.code,
        crossterm::event::KeyCode::Char('?')
    )
}

fn build_switched_page(current_page: &AppPage, kind: PageKind) -> Option<AppPage> {
    match (current_page, kind) {
        (AppPage::Subreddit(page), PageKind::Submission) => {
            let idx = page.list_state.selected()?;
            let submission = page.submissions.get(idx)?.clone();
            let mut next_page = SubmissionPage::with_client(submission, Arc::clone(&page.client));
            next_page.load_sync();
            Some(AppPage::Submission(next_page))
        }
        (AppPage::Subscription(page), PageKind::Subreddit) => {
            let idx = page.list_state.selected()?;
            let subreddit = page.subreddits.get(idx)?.display_name.clone();
            let mut next_page = SubredditPage::with_client(&subreddit, Arc::clone(&page.client));
            next_page.load_sync();
            Some(AppPage::Subreddit(next_page))
        }
        (AppPage::Inbox(page), PageKind::Message) => {
            let idx = page.list_state.selected()?;
            let message = page.messages.get(idx)?.clone();
            Some(AppPage::Message(MessagePage::new(message)))
        }
        (AppPage::Message(page), PageKind::Submission) => {
            let submission_id = page
                .message
                .reply_to
                .as_deref()
                .and_then(|reply_to| reply_to.strip_prefix("t3_"))
                .unwrap_or("mock_inbox");
            let subreddit = page.message.subreddit.as_deref().unwrap_or("messages");
            let submission = tuir_core::reddit::models::Submission {
                id: submission_id.to_string(),
                name: format!("t3_{submission_id}"),
                title: page.message.subject.clone(),
                author: page.message.author.clone(),
                subreddit: subreddit.to_string(),
                score: 0,
                num_comments: 0,
                permalink: format!("/r/{subreddit}/comments/{submission_id}/inbox"),
                url: format!("https://reddit.com/r/{subreddit}/comments/{submission_id}/"),
                selftext: page.message.body.clone(),
                created_utc: page.message.created_utc,
                distinguished: None,
                edited: tuir_core::reddit::models::EditedField::Bool(false),
                link_flair_text: None,
                author_flair_text: page.message.author_flair_text.clone(),
                over_18: false,
                pinned: false,
                spoiler: false,
                stickied: false,
                saved: false,
                hidden: false,
                likes: None,
                url_full: None,
            };
            let mut next_page = SubmissionPage::new(submission);
            next_page.load_sync();
            Some(AppPage::Submission(next_page))
        }
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use super::{
        auth_output, banner_for_width, build_switched_page, longest_visible_line_width,
        render_banner, AppPage, COMPACT_BOTTOM_ROWS, COMPACT_PIXEL_WIDTH, COMPACT_TOP_ROWS,
        HERO_BOTTOM_ROWS, HERO_PIXEL_WIDTH, HERO_TOP_ROWS, MIN_3D_WIDTH,
    };
    use tuir_core::{config::Config, reddit::models::Message};
    use tuir_tui::pages::{inbox::InboxPage, message::MessagePage, PageKind};

    fn hero_banner() -> String {
        render_banner(&HERO_TOP_ROWS, &HERO_BOTTOM_ROWS, HERO_PIXEL_WIDTH)
    }

    fn compact_banner() -> String {
        render_banner(&COMPACT_TOP_ROWS, &COMPACT_BOTTOM_ROWS, COMPACT_PIXEL_WIDTH)
    }

    #[test]
    fn defaults_to_compact_banner_when_width_is_unknown() {
        assert_eq!(banner_for_width(None), compact_banner());
    }

    #[test]
    fn uses_compact_banner_below_threshold() {
        assert_eq!(banner_for_width(Some(MIN_3D_WIDTH - 1)), compact_banner());
    }

    #[test]
    fn uses_3d_banner_at_threshold_and_above() {
        assert_eq!(banner_for_width(Some(MIN_3D_WIDTH)), hero_banner());
        assert_eq!(banner_for_width(Some(MIN_3D_WIDTH + 20)), hero_banner());
    }

    #[test]
    fn hero_banner_contains_branding_and_palette() {
        let banner = hero_banner();
        assert!(banner.contains("Terminal UI for Reddit -- Rust Rewrite"));
        assert!(banner.contains("\x1b[38;5;215m##\x1b[0m"));
        assert!(banner.contains("\x1b[38;5;196m##\x1b[0m"));
    }

    #[test]
    fn hero_banner_uses_visible_foreground_blocks() {
        let banner = hero_banner();
        assert!(!banner.contains("\x1b[48;5;"));
        assert!(banner.contains("##"));
    }

    #[test]
    fn banner_variants_stay_within_expected_width_budgets() {
        let hero_banner = hero_banner();
        let compact_banner = compact_banner();
        assert!(longest_visible_line_width(&hero_banner) >= 68);
        assert!(longest_visible_line_width(&hero_banner) <= 80);
        assert!(longest_visible_line_width(&compact_banner) <= 60);
    }

    #[test]
    fn hero_threshold_covers_rendered_width() {
        let hero_banner = hero_banner();
        assert!(usize::from(MIN_3D_WIDTH) >= longest_visible_line_width(&hero_banner));
    }

    #[test]
    fn auth_output_guides_when_client_id_is_missing() {
        let config = Config::default();

        let output = auth_output(&config, None);

        assert!(output.contains("OAuth is not configured"));
        assert!(output.contains("oauth_client_id"));
        assert!(output.contains("tuir.cfg"));
    }

    #[test]
    fn auth_output_prints_real_url_when_configured() {
        let mut config = Config::default();
        config.reddit.oauth_client_id = Some("client123".to_string());
        config.reddit.oauth_redirect_uri = "http://127.0.0.1:65000/".to_string();
        config.reddit.oauth_redirect_port = 65000;
        config.reddit.oauth_scope = "read,vote".to_string();

        let output = auth_output(&config, Some("demo-user"));

        assert!(output.contains("Username: demo-user"));
        assert!(output.contains("client123"));
        assert!(output.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A65000%2F"));
        assert!(output.contains("scope=read%2Cvote"));
    }

    #[test]
    fn inbox_switch_builds_message_page() {
        let mut inbox = InboxPage::new();
        inbox.messages = vec![Message {
            id: "mock_msg_1".to_string(),
            name: "t4_mock_msg_1".to_string(),
            subject: "Inbox subject".to_string(),
            author: "reddit".to_string(),
            body: "Hello".to_string(),
            body_html: None,
            created_utc: 1_700_000_000.0,
            dest: "mock_user".to_string(),
            new: true,
            reply_to: Some("t3_rust_1".to_string()),
            subreddit: Some("rust".to_string()),
            author_flair_text: None,
            was_comment: true,
        }];
        inbox.list_state.select(Some(0));

        let next_page = build_switched_page(&AppPage::Inbox(inbox), PageKind::Message);

        assert!(matches!(next_page, Some(AppPage::Message(_))));
    }

    #[test]
    fn message_switch_builds_submission_page() {
        let message = Message {
            id: "mock_msg_1".to_string(),
            name: "t4_mock_msg_1".to_string(),
            subject: "Inbox subject".to_string(),
            author: "reddit".to_string(),
            body: "Hello".to_string(),
            body_html: None,
            created_utc: 1_700_000_000.0,
            dest: "mock_user".to_string(),
            new: true,
            reply_to: Some("t3_rust_1".to_string()),
            subreddit: Some("rust".to_string()),
            author_flair_text: None,
            was_comment: true,
        };

        let next_page = build_switched_page(&AppPage::Message(MessagePage::new(message)), PageKind::Submission);

        assert!(matches!(next_page, Some(AppPage::Submission(_))));
    }
}
