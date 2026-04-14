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
use tuir_tui::theme::AppTheme;

const MIN_HERO_WIDTH: u16 = 72;

/// Hero banner: ANSI Shadow TUIR-RUST (6 rows × 69 cols).
const HERO_ROWS: [&str; 6] = [
    "████████╗██╗   ██╗██╗██████╗       ██████╗ ██╗   ██╗███████╗████████╗",
    "╚══██╔══╝██║   ██║██║██╔══██╗      ██╔══██╗██║   ██║██╔════╝╚══██╔══╝",
    "   ██║   ██║   ██║██║██████╔╝█████╗██████╔╝██║   ██║███████╗   ██║   ",
    "   ██║   ██║   ██║██║██╔══██╗╚════╝██╔══██╗██║   ██║╚════██║   ██║   ",
    "   ██║   ╚██████╔╝██║██║  ██║      ██║  ██║╚██████╔╝███████║   ██║   ",
    "   ╚═╝    ╚═════╝ ╚═╝╚═╝  ╚═╝      ╚═╝  ╚═╝ ╚═════╝ ╚══════╝   ╚═╝   ",
];

/// Rust-gradient palette applied row-by-row to the hero banner.
const HERO_COLORS: [u8; 6] = [220, 214, 208, 202, 196, 160];

/// Compact fallback banner: Standard figlet TUIR-RUST (5 rows × 52 cols).
const COMPACT_ROWS: [&str; 5] = [
    r" _____ _   _ ___ ____        ____  _   _ ____ _____ ",
    r"|_   _| | | |_ _|  _ \ _____|  _ \| | | / ___|_   _|",
    r"  | | | | | || || |_) |_____| |_) | | | \___ \ | |  ",
    r"  | | | |_| || ||  _ <      |  _ <| |_| |___) || |  ",
    r"  |_|  \___/|___|_| \_\     |_| \_\\___/|____/ |_|  ",
];

const COMPACT_COLORS: [u8; 5] = [220, 208, 202, 196, 160];

fn banner_for_width(width: Option<u16>) -> String {
    match width {
        Some(width) if width >= MIN_HERO_WIDTH => render_banner(&HERO_ROWS, &HERO_COLORS),
        _ => render_banner(&COMPACT_ROWS, &COMPACT_COLORS),
    }
}

fn render_banner(rows: &[&str], colors: &[u8]) -> String {
    let mut banner = String::from("\n");
    for (row, color) in rows.iter().zip(colors.iter()) {
        banner.push_str(&format!("\x1b[38;5;{color};1m{row}\x1b[0m\n"));
    }
    banner.push('\n');
    banner.push_str(
        "\x1b[38;5;208;1m          Terminal UI for Reddit — Rust Rewrite\x1b[0m\n\n",
    );
    banner
}

#[cfg(test)]
fn longest_visible_line_width(banner: &str) -> usize {
    banner.lines().map(visible_line_width).max().unwrap_or_default()
}

#[cfg(test)]
fn visible_line_width(line: &str) -> usize {
    let mut width = 0usize;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            while let Some(&c) = chars.peek() {
                chars.next();
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        width += 1;
    }
    width
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

    /// Borrow the active page's theme so children inherit it on switch.
    fn theme(&self) -> Arc<AppTheme> {
        match self {
            Self::Subreddit(p) => Arc::clone(&p.theme),
            Self::Submission(p) => Arc::clone(&p.theme),
            Self::Message(p) => Arc::clone(&p.theme),
            Self::Inbox(p) => Arc::clone(&p.theme),
            Self::Subscription(p) => Arc::clone(&p.theme),
            Self::Help(p) => Arc::clone(&p.theme),
        }
    }
}

/// Wire tracing to a daily-rotated file under the tuir data directory.
///
/// Returns a [`WorkerGuard`] that must be kept alive until `main` exits —
/// dropping it flushes buffered log lines. We route everything to a file
/// because the TUI owns stderr; writing log lines to the terminal would
/// corrupt the ratatui draw loop.
fn init_logging(verbose: bool) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let data_dir = Config::data_dir();
    if let Err(err) = std::fs::create_dir_all(&data_dir) {
        eprintln!(
            "[tuir] warning: could not create log dir {}: {err}",
            data_dir.display()
        );
        return None;
    }

    let file_appender = tracing_appender::rolling::daily(&data_dir, "tuir.log");
    let (writer, guard) = tracing_appender::non_blocking(file_appender);
    let level = if verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    let result = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_max_level(level)
        .try_init();

    if let Err(err) = result {
        // Another crate beat us to the global subscriber — not fatal.
        eprintln!("[tuir] warning: logging already initialized: {err}");
    }

    Some(guard)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // All diagnostics go to a daily-rotated log file. Writing to stderr would
    // corrupt the TUI, and the sub-commands that run without a TUI benefit
    // from the same destination so users have a single place to tail.
    let _log_guard = init_logging(cli.verbose);

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

/// Resolve the configured theme into an [`AppTheme`], falling back to the
/// built-in legacy palette when no theme is configured or the configured
/// name resolves to nothing.
fn build_theme(config: &Config) -> Arc<AppTheme> {
    match config.resolve_theme() {
        Some(core_theme) => {
            tracing::info!("loaded theme: {}", core_theme.name);
            Arc::new(AppTheme::from_core(&core_theme))
        }
        None => {
            tracing::debug!("no theme configured; using built-in defaults");
            Arc::new(AppTheme::default())
        }
    }
}

/// Enter TUI mode
fn do_tui(subreddit: Option<&str>) -> Result<()> {
    println!("[TUI] Starting terminal interface...");
    if let Some(sub) = subreddit {
        println!("[TUI] Opening r/{}", sub);
    }

    let config = Config::load().unwrap_or_default();
    let client = build_client(&config);
    let theme = build_theme(&config);
    let mut page = SubredditPage::with_client(subreddit.unwrap_or(""), client);
    page.set_theme(Arc::clone(&theme));
    page.load_sync();
    run_app(AppPage::Subreddit(page))?;

    println!("[TUI] Exited TUI mode");
    Ok(())
}

/// OAuth authentication flow
fn do_auth(user: Option<&str>) -> Result<()> {
    let config = Config::load()?;
    println!("{}", auth_output(&config, user));

    let client_id = config.reddit.oauth_client_id.clone().unwrap_or_default();
    if client_id.trim().is_empty() {
        return Ok(());
    }

    let scopes: Vec<String> = config
        .reddit
        .oauth_scope
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    let oauth = OAuth::new(
        client_id,
        config.reddit.oauth_redirect_uri.clone(),
        scopes,
    );

    let state = format!("tuir-{}", std::process::id());
    let auth_url = oauth.auth_url_with_state(&state);

    println!();
    println!("[AUTH] Open the following URL in your browser:");
    println!();
    println!("  {auth_url}");
    println!();
    println!(
        "[AUTH] Waiting for callback on 127.0.0.1:{}...",
        config.reddit.oauth_redirect_port
    );

    let code = wait_for_callback(config.reddit.oauth_redirect_port, &state)?;
    println!("[AUTH] Authorization code received, exchanging for token...");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let http = reqwest::Client::new();
    let token = rt.block_on(oauth.exchange_code(&http, &code))?;

    let token_path = Config::token_file();
    token.save(&token_path)?;
    println!("[AUTH] Token stored at {}", token_path.display());
    println!("[AUTH] You are now logged in. Launch `tuir` to use the real Reddit backend.");

    Ok(())
}

/// Block until the OAuth provider redirects the browser to our localhost
/// listener, then extract and return the authorization `code` query param.
///
/// `expected_state` is compared against the `state` query param to protect
/// against cross-site callback injection.
fn wait_for_callback(port: u16, expected_state: &str) -> Result<String> {
    let server = tiny_http::Server::http(format!("127.0.0.1:{port}"))
        .map_err(|e| anyhow::anyhow!("failed to bind callback server: {e}"))?;

    let mut requests = server.incoming_requests();
    if let Some(request) = requests.next() {
        let outcome = parse_callback_query(request.url(), expected_state);
        let (body, result) = match &outcome {
            Ok(code) => (
                "<html><body style='font-family: system-ui; padding: 2rem;'>\
                <h2>✅ tuir-rust authorization complete</h2>\
                <p>You can close this tab and return to your terminal.</p>\
                </body></html>"
                    .to_string(),
                Ok(code.clone()),
            ),
            Err(err) => (
                format!(
                    "<html><body style='font-family: system-ui; padding: 2rem;'>\
                    <h2>❌ tuir-rust authorization failed</h2>\
                    <p>{err}</p></body></html>"
                ),
                Err(anyhow::anyhow!(err.clone())),
            ),
        };

        let response = tiny_http::Response::from_string(body)
            .with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
                    .unwrap(),
            );
        let _ = request.respond(response);

        return result;
    }

    Err(anyhow::anyhow!("callback server closed before receiving a request"))
}

/// Pure parser used by `wait_for_callback`. Extracts the `code` query param
/// from a request URL, validating `state` and surfacing `error` params.
fn parse_callback_query(url: &str, expected_state: &str) -> std::result::Result<String, String> {
    let parsed = url::Url::parse(&format!("http://localhost{url}"))
        .map_err(|e| format!("invalid callback URL: {e}"))?;
    let params: std::collections::HashMap<String, String> =
        parsed.query_pairs().into_owned().collect();

    if let Some(err) = params.get("error") {
        return Err(format!("Reddit returned error: {err}"));
    }

    let code = params
        .get("code")
        .ok_or_else(|| "callback URL missing `code` parameter".to_string())?;
    let state = params
        .get("state")
        .ok_or_else(|| "callback URL missing `state` parameter".to_string())?;
    if state != expected_state {
        return Err(format!(
            "state mismatch: expected {expected_state}, got {state}"
        ));
    }

    Ok(code.clone())
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
    let theme = build_theme(&config);
    let mut page = InboxPage::with_client(client);
    page.set_theme(Arc::clone(&theme));
    page.load_sync();
    run_app(AppPage::Inbox(page))?;
    Ok(())
}

/// Open subscriptions
fn do_subscriptions() -> Result<()> {
    println!("[SUBS] Listing subscriptions...");

    let config = Config::load().unwrap_or_default();
    let client = build_client(&config);
    let theme = build_theme(&config);
    let mut page = SubscriptionPage::with_client(client);
    page.set_theme(Arc::clone(&theme));
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
                            let inherited = current_page.theme();
                            let mut help = HelpPage::new();
                            help.set_theme(inherited);
                            stack.push(AppPage::Help(help));
                            continue;
                        }
                        let action = current_page.handle_key(key);
                        match action {
                            PageAction::Quit => {
                                if stack.len() > 1 {
                                    stack.pop();
                                } else {
                                    break;
                                }
                            }
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
            next_page.set_theme(Arc::clone(&page.theme));
            next_page.load_sync();
            Some(AppPage::Submission(next_page))
        }
        (AppPage::Subscription(page), PageKind::Subreddit) => {
            let idx = page.list_state.selected()?;
            let subreddit = page.subreddits.get(idx)?.display_name.clone();
            let mut next_page = SubredditPage::with_client(&subreddit, Arc::clone(&page.client));
            next_page.set_theme(Arc::clone(&page.theme));
            next_page.load_sync();
            Some(AppPage::Subreddit(next_page))
        }
        (AppPage::Inbox(page), PageKind::Message) => {
            let idx = page.list_state.selected()?;
            let message = page.messages.get(idx)?.clone();
            let mut next_page = MessagePage::new(message);
            next_page.set_theme(Arc::clone(&page.theme));
            Some(AppPage::Message(next_page))
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
                selftext_html: page.message.body_html.clone(),
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
            next_page.set_theme(Arc::clone(&page.theme));
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
        parse_callback_query, render_banner, AppPage, COMPACT_COLORS, COMPACT_ROWS, HERO_COLORS,
        HERO_ROWS, MIN_HERO_WIDTH,
    };
    use tuir_core::{config::Config, reddit::models::Message};
    use tuir_tui::pages::{inbox::InboxPage, message::MessagePage, PageKind};

    fn hero_banner() -> String {
        render_banner(&HERO_ROWS, &HERO_COLORS)
    }

    fn compact_banner() -> String {
        render_banner(&COMPACT_ROWS, &COMPACT_COLORS)
    }

    #[test]
    fn defaults_to_compact_banner_when_width_is_unknown() {
        assert_eq!(banner_for_width(None), compact_banner());
    }

    #[test]
    fn uses_compact_banner_below_threshold() {
        assert_eq!(
            banner_for_width(Some(MIN_HERO_WIDTH - 1)),
            compact_banner()
        );
    }

    #[test]
    fn uses_hero_banner_at_threshold_and_above() {
        assert_eq!(banner_for_width(Some(MIN_HERO_WIDTH)), hero_banner());
        assert_eq!(banner_for_width(Some(MIN_HERO_WIDTH + 20)), hero_banner());
    }

    #[test]
    fn hero_banner_contains_branding_and_gradient_palette() {
        let banner = hero_banner();
        assert!(banner.contains("Terminal UI for Reddit — Rust Rewrite"));
        for color in HERO_COLORS {
            assert!(
                banner.contains(&format!("\x1b[38;5;{color};1m")),
                "missing palette entry {color}"
            );
        }
    }

    #[test]
    fn hero_banner_fully_spells_tuir_rust() {
        // Sanity check: the ANSI Shadow letters include enough ║/╗/╝ glyphs
        // across all six rows that we exercise every letter of TUIR-RUST.
        let banner = hero_banner();
        assert!(banner.contains("████████╗"));
        assert!(banner.contains("██████╔╝"));
        assert!(banner.contains("╚═════╝"));
    }

    #[test]
    fn banner_rows_are_uniform_width() {
        let hero_width = visible_line_width(HERO_ROWS[0]);
        for row in HERO_ROWS.iter().skip(1) {
            assert_eq!(visible_line_width(row), hero_width);
        }
        let compact_width = visible_line_width(COMPACT_ROWS[0]);
        for row in COMPACT_ROWS.iter().skip(1) {
            assert_eq!(visible_line_width(row), compact_width);
        }
    }

    #[test]
    fn banner_variants_stay_within_expected_width_budgets() {
        let hero = hero_banner();
        let compact = compact_banner();
        assert!(longest_visible_line_width(&hero) >= 60);
        assert!(longest_visible_line_width(&hero) <= 80);
        assert!(longest_visible_line_width(&compact) >= 45);
        assert!(longest_visible_line_width(&compact) <= 60);
    }

    #[test]
    fn hero_threshold_covers_rendered_width() {
        let hero = hero_banner();
        assert!(
            usize::from(MIN_HERO_WIDTH) >= longest_visible_line_width(&hero),
            "MIN_HERO_WIDTH must exceed visible hero width"
        );
    }

    use super::visible_line_width;

    #[test]
    fn parse_callback_query_returns_code_on_matching_state() {
        let code = parse_callback_query("/?code=abc123&state=nonce-1", "nonce-1").unwrap();
        assert_eq!(code, "abc123");
    }

    #[test]
    fn parse_callback_query_rejects_state_mismatch() {
        let err = parse_callback_query("/?code=abc&state=wrong", "nonce-1").unwrap_err();
        assert!(err.contains("state mismatch"));
    }

    #[test]
    fn parse_callback_query_surfaces_reddit_error() {
        let err = parse_callback_query("/?error=access_denied&state=n", "n").unwrap_err();
        assert!(err.contains("access_denied"));
    }

    #[test]
    fn parse_callback_query_requires_code_param() {
        let err = parse_callback_query("/?state=n", "n").unwrap_err();
        assert!(err.contains("missing `code`"));
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
