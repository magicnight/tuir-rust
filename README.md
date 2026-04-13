# TUIR — Terminal UI for Reddit (Rust Rewrite)

**TUIR-rust** is a rewrite of the Python-based [tuir](https://github.com/proycon/tuir) terminal Reddit client in Rust.

## Project Lineage

```
rtv (Python, by Michael Lazar)
    │
    └──► tuir (Python fork, by proycon)
              │
              └──► tuir-rust (Rust rewrite, this project)
```

- **rtv** — Reddit Terminal Viewer, created by Michael Lazar, the original Python TUI Reddit client
- **tuir** — Terminal UI for Reddit, a fork of rtv maintained by proycon with bug fixes and updates
- **tuir-rust** — A ground-up rewrite in Rust, not a direct port

## Why Rewrite in Rust?

### Problems with the Python Version

1. **Slow startup** — Python's interpreter overhead means 1–2 seconds before the first screen renders
2. **Dependency hell** — The original tuir bundled an outdated PRAW 3.6.1 (fork), which is unmaintained and has known security issues
3. **API changes** — Reddit's API has evolved, but updating PRAW is difficult due to bundled dependencies and API breakage
4. **Python 2/3 transition** — The codebase carries legacy Python 2 compatibility cruft
5. **No binary distribution** — Requires Python environment, pip installation, and dependency management

### Benefits of Rust

| Aspect | Python tuir | Rust tuir-rust |
|--------|-------------|----------------|
| Startup time | 1–2 seconds | < 50ms |
| Binary size | N/A (needs interpreter) | Single ~5MB binary |
| Dependencies | PRAW 3.6.1 (bundled), urwid, requests | Statically linked |
| Type safety | Dynamic typing | Full compile-time checking |
| Distribution | PyPI / pip | Single executable |

### Non-Goals

This is **not** a feature-complete rewrite. The goal is to achieve feature parity for core browsing workflows (80%), with focus on:

- ✅ Browsing subreddits (hot, new, top)
- ✅ Reading comments with collapsible threads
- ✅ Voting
- ✅ Viewing inbox
- ✅ Managing subscriptions

Out of scope (at least initially):

- ❌ Posting / commenting
- ❌ Multi-account management improvements
- ❌ Plugin system

## Architecture

```
tuir-rust/
├── crates/
│   ├── tuir-core/     # Core library
│   │   ├── config.rs      # INI config parsing
│   │   ├── theme.rs       # Theme system (ANSI colors)
│   │   ├── oauth.rs       # OAuth2 authentication
│   │   ├── reddit/        # Reddit API client
│   │   │   ├── client.rs  # HTTP client + rate limiting
│   │   │   ├── endpoints.rs # API endpoints
│   │   │   ├── models.rs  # Submission, Comment, etc.
│   │   │   └── mock.rs    # Mock client for testing
│   │   ├── mailcap.rs    # Mailcap file parser
│   │   └── content.rs    # HTML → terminal renderer
│   │
│   ├── tuir-tui/       # Terminal UI (ratatui)
│   │   ├── app.rs       # Main event loop
│   │   ├── pages/       # Page implementations
│   │   │   ├── subreddit.rs
│   │   │   ├── submission.rs
│   │   │   ├── inbox.rs
│   │   │   └── subscription.rs
│   │   ├── widgets/     # Reusable ratatui widgets
│   │   ├── keymap.rs   # Vim-style keybindings
│   │   └── terminal.rs  # Terminal setup/teardown
│   │
│   └── tuir-cli/       # CLI entry point (clap)
│       └── main.rs
└── docs/
    └── ROADMAP.md      # Development plan
```

## Key Libraries

| Component | Library | Notes |
|-----------|---------|-------|
| TUI framework | [ratatui](https://github.com/ratatui/ratatui) | Successor to tui-rs, immediate-mode rendering |
| Terminal I/O | [crossterm](https://github.com/crossterm-rs/crossterm) | Cross-platform terminal manipulation |
| HTTP client | [reqwest](https://github.com/seanmonstar/reqwest) | Async HTTP with TLS |
| OAuth2 | [oauth2](https://github.com/ramosbugs/oauth2-rs) | Reddit OAuth flow |
| Config | [configparser](https://github.com/youknowone/configparser) | INI file parsing |

## Installation

Not yet available — this is a work in progress.

```bash
# Clone the repository
git clone https://github.com/yourusername/tuir-rust.git
cd tuir-rust

# Build
cargo build --release

# Run
./target/release/tuir --subreddit rust
```

## Configuration

TUIR-rust is designed to be **config-file compatible** with the original tuir. Copy your existing `~/.config/tuir/tuir.cfg` and it should work (with minor exceptions during early development).

Default config locations:
- `$XDG_CONFIG_HOME/tuir/tuir.cfg` (Linux/macOS)
- `%APPDATA%/tuir/tuir.cfg` (Windows)

### Key Configuration Options

```ini
[tuir]
subreddit = front          ; Default subreddit on startup
enable_media = False       ; Open external links via mailcap
ascii = False              ; ASCII-only mode
monochrome = False          ; Disable colors
persistent = True          ; Store OAuth token between sessions
history_size = 200         ; Max history entries

oauth_client_id = your_client_id
oauth_redirect_port = 65000
```

## Themes

TUIR-rust uses the same theme format as the original tuir. Built-in themes:

- **Solarized Dark** (default)
- **Solarized Light**
- **Molokai**
- **Papercolor**

Place custom themes in `~/.config/tuir/themes/` as `.cfg` files.

## Development Status

| Milestone | Status |
|-----------|--------|
| M0 — Scaffold | ✅ Complete |
| M1 — Config + Themes | ✅ Complete |
| M2 — Reddit API + OAuth | ✅ Complete |
| M3 — Core TUI | 🚧 In Progress |
| M4 — Comments + Voting | 🔜 Next |
| M5 — Inbox + Subscriptions | 🔜 Next |
| M6 — Mailcap + Media | 🔜 Next |
| M7 — Post/Comment | 🔜 Future |
| M8 — Polish + Release | 🔜 Future |

## Contributing

This project is in early development. Contributions are welcome, but please:

1. Read the [ROADMAP.md](docs/ROADMAP.md) for the overall plan
2. Check open issues before creating new ones
3. Ensure `cargo fmt` and `cargo clippy` pass before submitting PRs

## Related Projects

| Project | Language | Status |
|---------|----------|--------|
| [rtv](https://github.com/michael-lazar/rtv) | Python | Archived (read-only) |
| [tuir](https://github.com/proycon/tuir) | Python | Active (maintenance mode) |
| tuir-rust | Rust | Active (rewrite in progress) |

## License

MIT License. See [LICENSE](LICENSE).

## Acknowledgments

- **Michael Lazar** — Original author of rtv, the project that started terminal-based Reddit browsing
- **proycon** — Maintained tuir for years, keeping rtv alive after its original author stepped back
- **The Rust community** — For ratatui, crossterm, and the excellent ecosystem of TUI libraries
