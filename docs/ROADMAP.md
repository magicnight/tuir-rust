# tuir → Rust 重构：可行性评估与开发计划

## Context

用户目标：将 Python 项目 [proycon/tuir](https://github.com/proycon/tuir)（Reddit 终端 TUI 客户端，rtv 的 fork）用 Rust 重构，工作目录 `/Users/kevin/Projects/tuir-rust`（目前仅含 `.omc/` 空壳，无 Cargo 工程）。

重构动机：原项目基于 Python 2/3 + urwid + 捆绑的老旧 PRAW 3.6.1，维护负担重、启动慢、依赖复杂。Rust 版本可提供单二进制分发、更快的启动与渲染、更强的类型安全，同时兼容原有配置与 mailcap 生态。

本文档给出：(1) 可行性结论 (2) 架构映射 (3) 分阶段里程碑 (4) 风险与缓解 (5) 验证方法。这是计划阶段产出，不包含实际代码改动。

---

## 1. 可行性结论

**结论：可行，建议采用"分阶段渐进式"重构（Greenfield 新建 Cargo 项目，而非机械移植）。**

关键依据：

| 维度 | 状态 | 备注 |
|---|---|---|
| TUI 框架 | ✅ 成熟 | `ratatui` + `crossterm` 已是 Rust TUI 事实标准 |
| Reddit API | ⚠️ 需评估 | `roux` 不活跃，建议以 `reqwest` 直接走 Reddit REST + 自封装薄 client |
| OAuth2 (installed app flow) | ✅ | `oauth2` crate 支持 refresh token；本地回调用 `tiny_http` |
| 配置 (INI) | ✅ | `rust-ini` 或 `configparser` crate，可直接复用用户现有 `tuir.cfg` |
| mailcap 解析 | ⚠️ | 无现成 crate，需写 ~300 行小解析器（RFC 1524 子集） |
| 主题 | ✅ | INI/TOML 主题文件 → serde 反序列化 |
| HTML/Markdown 渲染 | ⚠️ | 需 `scraper` + 自写 terminal renderer（处理粗体/链接/代码块） |
| 剪贴板 | ✅ | `arboard` crate 跨平台 |
| 外部查看器 (mailcap 执行) | ✅ | `std::process::Command` |

**不建议做**：逐文件照搬 Python 模块结构。Rust 的所有权模型与 ratatui 的 immediate-mode 渲染要求完全不同于 urwid event-loop，UI 层必须重设计。

---

## 2. 架构映射

原 `tuir/` 模块 → Rust crate 结构（单 workspace，多 module）：

```
tuir-rust/
├─ Cargo.toml                 # workspace
├─ crates/
│  ├─ tuir-core/              # 数据模型 + API client + OAuth + config
│  │  ├─ src/
│  │  │  ├─ config.rs         # ← config.py          (rust-ini + serde)
│  │  │  ├─ theme.rs          # ← theme.py
│  │  │  ├─ oauth.rs          # ← oauth.py           (oauth2 + tiny_http)
│  │  │  ├─ reddit/           # ← packages/praw/     (自封装 REST client)
│  │  │  │  ├─ client.rs
│  │  │  │  ├─ models.rs      # Submission/Comment/Subreddit/Message
│  │  │  │  └─ endpoints.rs
│  │  │  ├─ content.rs        # ← content.py         (HTML→text 渲染)
│  │  │  ├─ mailcap.rs        # ← mime_parsers.py    (RFC 1524 解析器)
│  │  │  └─ clipboard.rs      # ← clipboard.py       (arboard)
│  ├─ tuir-tui/               # ratatui 前端
│  │  ├─ src/
│  │  │  ├─ app.rs            # 全局状态机 + 事件循环
│  │  │  ├─ pages/
│  │  │  │  ├─ mod.rs         # Page trait
│  │  │  │  ├─ subreddit.rs   # ← subreddit_page.py
│  │  │  │  ├─ submission.rs  # ← submission_page.py
│  │  │  │  ├─ inbox.rs       # ← inbox_page.py
│  │  │  │  ├─ subscription.rs# ← subscription_page.py
│  │  │  │  └─ help.rs
│  │  │  ├─ widgets/          # 复用的 ratatui 组件
│  │  │  ├─ keymap.rs         # Vim 风格键位
│  │  │  └─ terminal.rs       # ← terminal.py (crossterm 封装)
│  └─ tuir-cli/               # main binary
│     └─ src/main.rs          # 参数解析 (clap) + 启动
└─ assets/
   ├─ themes/                 # 复制原 .cfg 主题
   └─ mailcap.default
```

关键第三方 crate：`ratatui`、`crossterm`、`tokio`（async API/OAuth）、`reqwest`、`oauth2`、`tiny_http`、`serde`、`rust-ini`、`toml`、`clap`、`arboard`、`scraper`、`anyhow` / `thiserror`、`tracing`。

---

## 3. 分阶段里程碑

**M0 — 脚手架 (1–2 天)**
- `cargo new --bin` workspace 初始化；引入上述依赖；配置 CI (cargo fmt/clippy/test)。
- 读取并复用原 `tuir.cfg` 示例作为 fixture。

**M1 — 配置 + 主题加载 (3–5 天)**
- 实现 `config.rs`：XDG 路径、INI 解析、CLI flag 合并（对齐原 `--theme/--config/--enable-media`）。
- 实现 `theme.rs`：加载 Solarized/Molokai/Papercolor 主题文件，输出 ratatui `Style`。
- 单元测试：对原 `tuir/themes/*.cfg` 做回归解析。

**M2 — Reddit REST + OAuth (5–8 天)**
- 用 `oauth2` crate 实现 installed-app flow，`tiny_http` 监听 `http://localhost:6500/`。
- `reddit::client`：封装 `reqwest::Client`，自动 attach bearer token、处理 401 refresh、限流 (X-Ratelimit-*)。
- 最小端点集：`/hot`、`/r/{sub}/hot`、`/comments/{id}`、`/api/vote`、`/api/comment`、`/message/inbox`、`/subreddits/mine/subscriber`。
- 集成测试：用录制的 HTTP fixture (`wiremock` crate) 离线回放。

**M3 — 核心 TUI + Subreddit 页 (1 周)**
- `app.rs` 事件循环：crossterm poll + tick；全局 `AppState` + `Vec<Box<dyn Page>>` 栈。
- `Page` trait：`render(&self, frame, area)`、`handle_key(&mut self, key) -> PageAction`。
- 实现 `SubredditPage`：列表渲染、j/k/gg/G 导航、数字跳转、r 刷新、/ 搜索。

**M4 — Submission/评论树 + 投票 (1 周)**
- `SubmissionPage`：评论树折叠展开（c 折叠）、a/z 投票、m 标记已读。
- `content.rs`：HTML → 终端富文本（链接编号化、代码块高亮，使用 `syntect` 可选）。

**M5 — Inbox / Subscription / 多账户 (4–5 天)**
- InboxPage、SubscriptionPage 复用 M3 list 组件。
- 多账户：token 存储至 `$XDG_DATA_HOME/tuir/tokens/<user>.json`。

**M6 — mailcap + 外部查看器 (3–4 天)**
- `mailcap.rs`：解析 `~/.config/tuir/mailcap`，匹配 MIME，%s 替换。
- 启动外部进程时先 `terminal::suspend()` 再 `resume()`（对齐原 `terminal.py::suspend`）。

**M7 — 发帖/评论写入 + 编辑器集成 (3 天)**
- `$EDITOR` 打开临时文件，回填内容提交。

**M8 — 打磨与发布 (1 周)**
- 完整 help 页、错误提示、日志 (`tracing` + `tracing-appender`)。
- 跨平台测试 (macOS/Linux)；`cargo-dist` 构建 release 二进制。
- README、迁移指南（指明哪些原功能尚未实现）。

**总估时**：单人 5–7 周达到 feature parity 的 80%（不含 NSFW 媒体播放等边缘功能）。

---

## 4. 风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| Reddit API 政策变动 / 费率 | 高 | 尽早在 M2 跑通真实 token；限制默认 polling 频率 |
| `roux` 不可用 → 自封装工作量 | 中 | M2 即放弃 roux，直接 reqwest + 薄 model 层，只实现用到的端点 |
| urwid 事件模型迁移到 immediate-mode 导致 UI 状态复杂 | 中 | 采用单一 `AppState` + reducer 模式，避免散落的可变状态 |
| HTML → 终端渲染质量退化 | 中 | M4 用 `scraper` 遍历 DOM；先支持 p/a/code/pre/blockquote/ul/ol，其它降级为纯文本 |
| mailcap 行为差异 | 低 | 只实现原 tuir 实际使用的 test=/copiousoutput 子集，写对照测试 |
| Windows 支持 | 低 | 显式仅承诺 macOS + Linux（与原项目一致） |

---

## 5. 关键参考 & 复用资源

需要开发时直接对照的上游文件（阅读 https://github.com/proycon/tuir 对应路径）：
- `tuir/config.py`, `tuir/oauth.py`, `tuir/terminal.py` — 行为规格
- `tuir/themes/*.cfg` — 主题格式
- `tuir/templates/mailcap` — 默认 mailcap
- `tuir/docs/tuir.cfg` — 配置样例（作为 Rust 端解析 fixture）
- 测试套件 `tests/` — 作为 Rust 端集成测试用例的参考

---

## 6. 验证方法（端到端）

执行顺序（每阶段完成后）：

1. **构建**：`cargo build --release && cargo clippy -- -D warnings && cargo fmt --check`
2. **单元 + 集成测试**：`cargo test --workspace`（含 wiremock 回放）
3. **配置兼容性**：用原 tuir 用户的 `~/.config/tuir/tuir.cfg` 启动 rust 版，确认解析无错。
4. **OAuth 冒烟**：首次运行触发浏览器授权 → 回调 → 展示 front page；重启后无需再次登录。
5. **交互矩阵**（手工）：
   - 浏览 `/r/rust/hot`、翻页、刷新
   - 进入帖子 → 展开/折叠评论 → 投票 → 回复
   - 切换 inbox、订阅列表
   - 触发 mailcap 打开图片（`feh`/`imv`）并 suspend/resume
6. **回归对比**：并排运行原 Python tuir 与 Rust 版，抽样同一 subreddit 的渲染差异。
7. **性能基线**：启动到首屏时间目标 < 300 ms（原 Python 版约 1–2 s）。

---

## 7. 开启实施的第一步

批准本计划后，在 `/Users/kevin/Projects/tuir-rust` 下执行 M0：
- 初始化 workspace + 三个 crate
- 提交首个 commit：`chore: init tuir-rust workspace`
- 将本计划文件复制为仓库内 `docs/ROADMAP.md` 持续跟踪

不在本计划范围内（后续独立提案）：Web/Mastodon 桥接、移动端、插件系统、AI 摘要集成。

---

## 8. 当前进度（live）

最后更新：在 `feat(media): async download + spinner via worker thread` (`10c6610`) commit 之后。M9 相关的三项后续（GIF 帧循环、主题化评论深度色、异步下载范式）已随主干 M10+ 候选项并入。

| 里程碑 | 状态 | 关键 commit |
|---|---|---|
| M0 — 脚手架 | ✅ | `2872577` chore: init tuir-rust workspace |
| M1 — 配置 + 主题 | ✅ | `2c865f2` feat: implement config and theme loading (M1) |
| M2 — Reddit API + OAuth 骨架 | ✅ | `89a97bc` feat: implement Reddit API client and mock (M2) |
| M3 — 核心 TUI + Subreddit 页 | ✅ | `4c67eb3` feat: implement CLI with ASCII banner and clap subcommands (M3) |
| M4 — Submission/评论树 + 投票 | ✅ | `474b543` feat: implement SubredditPage with voting and navigation (M4) + `0c4265d` 评论树递归修复 |
| M5 — Inbox / Subscription / Message | ✅ | `4c3a644` feat: implement InboxPage and SubscriptionPage (M6) + `bc41428` MessagePage |
| M6 — 主题/视觉一致化 | ✅ | `4e32d22` 主题接入 SubredditPage、`98ca493` 全页面迁移、`77e4116` header 锚点统一 |
| M7 — OAuth token 全流程 | ✅ | `de5dac4` token 交换 + 持久化、`d0fb0a0` tiny_http 回调监听器 |
| M8 — Sort/Goto/Help/HTML 渲染 | ✅ | `5d9f084` 1-5 sort、`8c04a68` `/` goto、`f9310c2` HelpPage、`8ac7933` content 渲染 |
| **M9 — 媒体预览** | ✅ | `125a01a` 识别+config、`5068b4c` MediaPage 骨架、`99d8155` 内嵌 image 渲染、`4621966` mailcap 外部 viewer |

`tuir auth` + `~/.config/tuir/tuir.cfg` 配 `oauth_client_id` 即可登录真实 Reddit；未登录时所有页面 fallback 到 `MockRedditClient`。

---

## 9. 后续方向（M10+ 候选，未排序）

按价值 × 工程量分组的下一波候选项。每条都是独立可单独接的小 feature，不互相依赖。

### 🟢 小（半天内可完成）

- ✅ **Comment 深度颜色走主题** — `b978b96` feat(theme): route comment depth colors through AppTheme（`CursorBar1..4` 注入）
- **`auth_output` 瘦身** — cleanup #1 漏掉的，那一段还是连串 `lines.push(format!(...))`，改成结构化 builder 或 raw string。
- **`m` 键 mark all read** — Inbox 页的批量已读，调一次 `client.mark_read(name)` 循环。
- **`s` 键 save submission** — `/api/save` + `/api/unsave`，对应 Submission 的星标。
- ✅ **GIF 动画帧循环** — `0c66e70` feat(media): animated GIF frame loop with Page::tick hook

### 🟡 中（1-2 个 commit）

- **`/u/<username>` 用户 profile 页** — 原 tuir 经典页面：列出某用户的 submissions + comments，按 `u` 在 SubmissionPage 上触发，新增 `UserPage` 页面 + `RedditApi::user_overview(name)` trait 方法。
- ✅ **异步媒体下载 + spinner** — `10c6610` feat(media): async download + spinner via worker thread（确立 `Page::tick` + 后台 worker + `mpsc::Receiver` 范式，后续所有长操作页面共用）
- **Submission `r` 回复 / `e` 编辑 / `d` 删除** — M7 发帖路径。需要 `$EDITOR` 集成（写临时 markdown 文件 → 等编辑器退出 → 提交到 `/api/comment`）+ terminal suspend/resume（M9.4 已经做完）。
- **Multi-account 切换** — `tuir auth --user alice` 已支持，但 token 都写到同一个 `refresh-token` 文件。改成 `<data_dir>/tokens/<user>.json`，加个 `--account` CLI flag。

### 🔴 大（独立 milestone）

- **M10 — 搜索** — `/r/<sub>/search?q=…`、front-page 搜索、subreddit 搜索（不只是 goto，是真正的全文搜索结果列表）。需要新页面 + `RedditApi::search()` trait 方法 + 输入流水线复用 SubredditPage 的 goto prompt 模式。
- **M11 — 真实的 comment "load more"** — 当前评论树里的 `MoreComments` 节点显示为 `[+] Load more comments` 但按 Enter 不会展开。需要 `/api/morechildren` 端点 + 把返回的子树拼回到 flatten_tree 的对应位置。
- **M12 — Gallery 多图渲染** — Reddit gallery posts 是多张图片，`MediaKind::Gallery` 当前只能 mailcap 外开。inline 路径需要解析 `gallery_data` + `media_metadata` 字段、把每张图独立 download_to_cache、用一个左右翻页 widget 串起来。
- **M13 — 性能与发布** — `cargo dist` 的 release 二进制（macOS arm64/x86_64、Linux x86_64/arm64）、启动时间 < 50ms 验证、CI matrix。
- **M14 — Scrolling 评论 viewer** — 当前评论列表是单行截断，按 Enter 进入"展开评论 viewer"显示完整 markdown 渲染（复用 `content::render_plain_string` 的输出）。

### 📌 长期 / 超出本仓库范围

- 同步用户的 multireddit / 自定义 feeds
- Imgur / Reddit OAuth scope 扩展（gold / modlog / wiki edit）
- TUI 内嵌 markdown 编辑器（替代 `$EDITOR` shell out）
- AI 摘要 / 翻译集成（独立的 sidecar 进程，不内嵌依赖）

---

## 10. 决策日志

| 日期（提交时间） | 决策 | 替代方案与理由 |
|---|---|---|
| 2026-04-14 a579ee9 | `RedditApi` trait 抽象，pages 持 `Arc<dyn RedditApi>` | 让 mock/真实 client 在运行时切换；不为每个 page 写两份代码 |
| 2026-04-14 4e32d22 | 主题用 `Arc<AppTheme>` 而非 thread-local | 显式注入比隐式全局更可测；与现有 `Arc<dyn RedditApi>` 注入路径一致 |
| 2026-04-14 99d8155 | MediaPage 同步阻塞下载 | 与现有所有 `load_sync()` 模式一致；引入 spinner 是独立 milestone（异步媒体下载） |
| 2026-04-14 3e81854 | `ratatui-image` 关掉 `chafa-dyn` default feature | 保住单二进制分发；牺牲 chafa 后端的少量额外终端兼容性 |
| 2026-04-14 4621966 | mailcap 命令通过 `sh -c` 启动 | 让用户的 `feh %s args` 之类组合命令直接生效；URL 已 shell-quoted |
| 2026-04-15 0c66e70 | `Page::tick()` 默认 no-op + CLI 每帧调用 | 不想把时间驱动逻辑散落进 render；MediaPage 的 GIF 帧推进是第一个用户，后续 spinner/轮询都复用同一入口 |
| 2026-04-15 10c6610 | 媒体下载走独立 `std::thread` + 线程内单线程 `tokio` runtime，不引入全局 runtime | 单次下载不值得给整个 TUI 背一个长生命周期 runtime；`mpsc::Receiver` + `try_recv` 在 `tick()` 里消费即可，页面切走时 receiver 被 drop，worker 发送静默失败 |
