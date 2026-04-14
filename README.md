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
| TUI framework | [ratatui](https://github.com/ratatui/ratatui) 0.30 | Immediate-mode rendering |
| Terminal I/O | [crossterm](https://github.com/crossterm-rs/crossterm) 0.29 | Cross-platform terminal manipulation |
| Inline images | [ratatui-image](https://github.com/benjajaja/ratatui-image) 10 + [image](https://github.com/image-rs/image) 0.25 | Kitty / iTerm2 / Sixel / half-block |
| HTTP client | [reqwest](https://github.com/seanmonstar/reqwest) 0.12 | Async HTTP with TLS |
| OAuth2 callback | [tiny_http](https://github.com/tiny-http/tiny-http) | Localhost listener for installed-app flow |
| HTML rendering | [scraper](https://github.com/causal-agent/scraper) | DOM walker for `body_html` / `selftext_html` |
| Config | [configparser](https://github.com/youknowone/configparser) | INI file parsing |

## What works today

The browse loop is end-to-end functional — both against the bundled `MockRedditClient` (no credentials required) and against the real Reddit API once you've completed `tuir auth`.

- **Subreddit listing** — hot/new/top/controversial/rising via `1`–`5`, `a`/`z` voting, `r` refresh
- **`/` goto prompt** — type any subreddit name (with or without `r/` prefix), Enter to jump, Esc to cancel
- **Submission view** — proper recursive comment tree with `c` collapse, HTML body rendering for both `selftext_html` and comment `body_html`
- **Inline media preview** — press `i` on a submission with image media. Auto-detects [kitty](https://sw.kovidgoyal.net/kitty/graphics-protocol/) / [iTerm2](https://iterm2.com/documentation-images.html) / [Sixel](https://en.wikipedia.org/wiki/Sixel) / Unicode half-blocks. Set `media_style = retro` in your config to **force half-blocks even on capable terminals** for that classic [browsh](https://github.com/browsh-org/browsh) aesthetic.
- **Animated GIF playback** — multi-frame GIFs decode every frame and loop in place via the page-tick driver, with browser-style 100ms minimum delay
- **Async media loading** — downloads run on a worker thread; the page shows a braille spinner while bytes stream in, no UI freezes
- **External viewer** — press `o` to hand any URL to your mailcap-resolved viewer (`feh`, `mpv`, `xdg-open`, …) with proper terminal suspend/resume
- **Inbox + Subscription pages** — `u` toggle read/unread, navigate into individual messages
- **HelpPage** — `?` from any page shows the full keybinding reference
- **Theme system** — Solarized Dark/Light, Molokai, Papercolor built-in; user themes load from `~/.config/tuir/themes/<name>.cfg`
- **OAuth flow** — `tuir auth` opens a localhost callback listener on port 65000, captures the code, exchanges it for a refresh token, and persists it. After that, `tuir` automatically uses the real Reddit backend on every launch.

## Installation

```bash
# Clone the repository
git clone https://github.com/magicnight/tuir-rust.git
cd tuir-rust

# Build
cargo build --release

# Run with the mock backend (no credentials)
./target/release/tuir --subreddit rust

# Or authenticate first to use the real Reddit API
./target/release/tuir auth
```

> Pre-built binaries via `cargo dist` are tracked under M13 in [docs/ROADMAP.md](docs/ROADMAP.md).

## Configuration

TUIR-rust is designed to be **config-file compatible** with the original tuir. Copy your existing `~/.config/tuir/tuir.cfg` and it should work (with minor exceptions during early development).

Default config locations:
- `$XDG_CONFIG_HOME/tuir/tuir.cfg` (Linux/macOS)
- `%APPDATA%/tuir/tuir.cfg` (Windows)

### Key Configuration Options

```ini
[tuir]
subreddit       = front                       ; Default subreddit on startup
theme           = solarized-dark              ; Built-in or ~/.config/tuir/themes/<name>.cfg
enable_media    = false                       ; Open external links via mailcap
media_style     = auto                        ; auto | retro | off  (M9)
ascii           = false                       ; ASCII-only mode
monochrome      = false                       ; Disable colors
persistent      = true                        ; Store OAuth token between sessions
history_size    = 200                         ; Max history entries

; OAuth (Reddit installed-app flow)
oauth_client_id     = your_client_id_here
oauth_redirect_uri  = http://127.0.0.1:65000/
oauth_redirect_port = 65000
oauth_scope         = identity,read,vote,mysubreddits,privatemessages,subscribe
```

### `media_style` cheatsheet

| Value | Behavior |
|---|---|
| `auto` *(default)* | Probe the terminal for the best image protocol (kitty / iTerm2 / Sixel). Falls back to half-blocks. |
| `retro` | **Force** Unicode half-block rendering even on capable terminals. The deliberate browsh / 90s-screencap aesthetic. |
| `off` | Skip inline rendering entirely; the media page just shows the URL. Press `o` to open externally. |

### Mailcap

External viewer dispatch is RFC 1524 mailcap-compatible. Drop entries into `~/.config/tuir/mailcap` (highest priority), `~/.mailcap`, or `/etc/mailcap`:

```
# ~/.config/tuir/mailcap
image/*; feh %s
video/*; mpv %s
text/html; firefox %s
*/*; xdg-open %s
```

Wildcards (`image/*`) and the universal catch-all (`*/*`) are honored, in that priority order.

## Themes

TUIR-rust uses the same theme format as the original tuir. Built-in themes:

- **Solarized Dark** (default)
- **Solarized Light**
- **Molokai**
- **Papercolor**

Place custom themes in `~/.config/tuir/themes/` as `.cfg` files.

## Development Status

| Milestone | Status | Highlights |
|-----------|--------|------------|
| M0 — Scaffold | ✅ | Three-crate workspace, cargo build / test / clippy clean |
| M1 — Config + Themes | ✅ | INI parser, four built-in themes, user theme override |
| M2 — Reddit API + OAuth skeleton | ✅ | `RedditApi` trait, mock + real client, wiremock-tested OAuth |
| M3 — Core TUI + Subreddit page | ✅ | ratatui event loop, page stack, navigation, voting |
| M4 — Submission + comment tree | ✅ | Recursive flatten, collapse, HTML body rendering, sort 1-5 |
| M5 — Inbox / Subscription / Message | ✅ | All three pages with mark-read, navigation, message viewer |
| M6 — Theme & visual consistency | ✅ | Every page reads from `Arc<AppTheme>`; unified header anchor |
| M7 — OAuth full flow | ✅ | `tuir auth` runs the localhost callback listener and stores the token |
| M8 — Sort / Goto / Help / HTML | ✅ | `1-5` sort, `/` goto prompt, `?` global help, content renderer |
| **M9 — Inline media preview** | ✅ | image+ratatui-image inline render, `media_style = retro`, mailcap external viewer, animated GIF frame loop, async download with spinner |
| M10+ — Search / `load more` / Gallery / Posting / Release | 🔜 | See [docs/ROADMAP.md §9](docs/ROADMAP.md) for the full backlog |

**Tests:** 125 unit/integration tests across the workspace, all green. `cargo clippy --all-targets -- -D warnings` is enforced.

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
