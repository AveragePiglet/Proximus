<div align="center">

# Proximus Workspace

A desktop workspace that wraps Claude Code in a native Tauri app with multi-tab terminals, a model-rewrite proxy, live memory graph visualization, theming, and project scaffolding.

Built with **Tauri 2 · React 19 · Rust · xterm.js · Cytoscape**

[![Made for Claude Code](https://img.shields.io/badge/Made_for-Claude_Code-blueviolet?style=flat-square)](https://docs.anthropic.com/en/docs/claude-code)
[![Tauri 2](https://img.shields.io/badge/Tauri-2-24C8D8?style=flat-square&logo=tauri&logoColor=white)](https://v2.tauri.app)
[![React](https://img.shields.io/badge/React-19-61DAFB?style=flat-square&logo=react&logoColor=black)](https://react.dev)
[![Rust](https://img.shields.io/badge/Rust-2021-DEA584?style=flat-square&logo=rust&logoColor=black)](https://www.rust-lang.org)

> **🚧 Work in Progress** — This project is under active development. Core features are functional but expect rough edges, missing polish, and breaking changes. Contributions and feedback welcome!

</div>

---

## Who is this for?

- **Solo developers** who use Claude Code daily and want a proper workspace instead of juggling terminal windows
- **Power users** who run multiple Claude sessions at once and need tabs, context tracking, and session recovery
- **Teams exploring AI-assisted development** who want a managed environment with built-in proxy routing, structured logs, and project scaffolding
- **Anyone tired of burning through Claude API credits** — Proximus routes Claude Code through GitHub Copilot's API, so you get Claude's capabilities on Copilot's usage limits instead of draining your Anthropic quota
- **Anyone using GitHub Copilot's API** who needs a transparent model-rewrite layer without modifying their Claude setup
- **Developers building with memory systems** who want a live graph view of their project's knowledge base instead of reading raw TOML

If you've ever wished Claude Code came in an app with tabs, a sidebar, and a dashboard — that's Proximus.

---

## Screenshots

<p align="center">
  <img src="assets/screenshots/Terminal.png" alt="Terminal View" width="800">
  <br><em>Claude Code running in a full ConPTY terminal</em>
</p>

<p align="center">
  <img src="assets/screenshots/Terminal-with-Logs.png" alt="Terminal with Logs Panel" width="800">
  <br><em>Terminal with the structured logs sidebar open</em>
</p>

<p align="center">
  <img src="assets/screenshots/Projects-Page.png" alt="Projects Page" width="800">
  <br><em>Project launcher and scaffolding view</em>
</p>

---

## What is Proximus?

Proximus Workspace is a native desktop application that turns Claude Code into a full IDE-like experience. Instead of running Claude in a bare terminal, Proximus gives you:

- **Tabbed terminal sessions** — Run multiple Claude Code instances side by side with native PTY support (ConPTY on Windows, PTY on macOS/Linux)
- **Terminal keyboard shortcuts** — Ctrl+C (copy selection), Ctrl+V (paste), Ctrl+Z (undo typing in time-grouped chunks)
- **Model rewrite proxy** — Transparently routes Claude through GitHub Copilot's API, rewriting model names on the fly
- **Live memory graph** — Visualize your project's knowledge graph in real-time with Cytoscape, click into nodes for detail
- **Project scaffolding** — Spin up new projects pre-loaded with memory systems, skills, and conventions
- **Memory migration** — Detects existing AI memory files (Cursor rules, AGENTS.md, CLAUDE.md, ADRs, etc.) and offers to migrate them into the structured .node-memory system
- **Context tracking** — Statusline integration shows context window usage per session
- **Structured logging** — Captures backend events in a sidebar panel with auto-scrolling
- **Theme system** — 14 built-in themes (10 dark, 4 light) with live terminal recoloring and localStorage persistence
- **Quick actions** — One-click access to common Claude Code commands

## How the Proxy Chain Works

Proximus doesn't call the Anthropic API directly. Instead it spins up a local proxy chain on startup:

1. **copilot-api** (`:4141` release / `:4151` dev) — GitHub Copilot's local API server, authenticated with your Copilot subscription
2. **model-rewrite-proxy** (`:4142` release / `:4152` dev) — A built-in Rust HTTP proxy that intercepts requests and rewrites model names (`claude-sonnet-4-20250514` → Copilot's internal model IDs), with full SSE streaming support
3. **Claude Code** connects to the rewrite proxy thinking it's talking to Anthropic — but it's going through Copilot

This means **zero Anthropic API costs**. You use Claude Code exactly as normal, but all usage counts against your GitHub Copilot plan instead. The proxy is transparent — no config changes needed in Claude Code itself.

## How the Memory System Works

Every project scaffolded by Proximus gets a `.node-memory/` directory — a graph-based knowledge store in plain TOML:

- **graph.toml** — Nodes (modules, plans, bugs) and edges (relationships between them), each with L0/L1/L2 summaries at increasing detail
- **state.toml** — Current task, branch, known issues — what Claude picks up when it starts a new session
- **invariants.toml** — Hard rules that never decay (e.g. "proxy must sit between Claude and copilot-api")
- **journal/** — Weekly append-only log of what changed and why
- **nodes/** — Deep detail files for each node, loaded on demand

The sidebar's **Memory Graph** view renders this live with Cytoscape — you can see your project's knowledge structure, click nodes to inspect them, and watch it update as Claude works.

## Architecture

```
┌─────────────────────────────────────────────┐
│              Proximus Workspace              │
│  ┌────────┐ ┌──────────┐ ┌───────────────┐  │
│  │ TabBar │ │ Toolbar  │ │  StatusBar    │  │
│  └────┬───┘ └────┬─────┘ └───────┬───────┘  │
│       │          │               │           │
│  ┌────▼──────────▼───────────────▼────────┐  │
│  │          Terminal (xterm.js)            │  │
│  │            PTY ↔ Claude Code              │  │
│  └────────────────────────────────────────┘  │
│                                              │
│  ┌─────────────┐  ┌──────────────────────┐   │
│  │  Sidebar    │  │  Quick Actions       │   │
│  │ ┌─────────┐ │  └──────────────────────┘   │
│  │ │ Memory  │ │                             │
│  │ │ Graph   │ │  ┌──────────────────────┐   │
│  │ ├─────────┤ │  │  Logs Panel          │   │
│  │ │Projects │ │  └──────────────────────┘   │
│  │ ├─────────┤ │                             │
│  │ │ Logs    │ │                             │
│  │ ├─────────┤ │                             │
│  │ │ Theme   │ │                             │
│  │ └─────────┘ │                             │
│  └─────────────┘                             │
└──────────────────────────────────────────────┘
         │
         │ spawns & manages
         ▼
┌──────────────────┐     ┌──────────────────┐
│  copilot-api     │────▶│ model-rewrite    │
│  :4141           │     │ proxy :4142      │
│  (npx)           │     │ (built-in Rust)  │
└──────────────────┘     └──────────────────┘
```

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop framework | Tauri 2 |
| Frontend | React 19, TypeScript, Vite 7 |
| Terminal | xterm.js 6 + ConPTY (Windows) / PTY (macOS/Linux) |
| Graph visualization | Cytoscape.js |
| Backend | Rust 2021 (tokio, hyper, portable-pty, notify, serde) |
| Proxy | Built-in Rust HTTP proxy (hyper) |
| Memory | TOML-based graph (custom format) |

## Project Structure

```
├── src/                        # React frontend
│   ├── App.tsx                 # Root layout — toolbar + sidebar + terminal
│   ├── components/
│   │   ├── Terminal.tsx        # xterm.js terminal with native PTY bridge
│   │   ├── TabBar.tsx          # Multi-tab session management
│   │   ├── Toolbar.tsx         # Top toolbar controls
│   │   ├── Sidebar.tsx         # Collapsible sidebar container
│   │   ├── MemoryGraphView.tsx # Live Cytoscape graph visualization
│   │   ├── NodeDetail.tsx      # Graph node inspector panel
│   │   ├── ProjectsView.tsx    # Project launcher / scaffolding UI
│   │   ├── MigrationDialog.tsx # Memory migration popup (detect & convert existing AI memory)
│   │   ├── SettingsDialog.tsx  # Modal settings dialog (theme picker + future settings)
│   │   ├── LogsPanel.tsx       # Timestamped log viewer
│   │   ├── QuickActions.tsx    # One-click Claude Code commands
│   │   ├── SettingsPanel.tsx   # Theme picker sidebar tab
│   │   ├── StatusBar.tsx       # Bottom bar — context stats + process info
│   │   └── StatusBadge.tsx     # Session state indicator
│   └── hooks/
│       ├── useMemoryGraph.ts   # Fetches & watches graph.toml / state.toml
│       └── useProcessStatus.ts # Polls process health (proxy, copilot-api)
│   themes.ts                   # 14 theme definitions + applyTheme() + listener system
│
├── src-tauri/src/              # Rust backend
│   ├── lib.rs                  # Tauri command registration (11 commands)
│   ├── process_manager.rs      # copilot-api lifecycle + port management (cross-platform)
│   ├── model_rewriter.rs       # Built-in HTTP proxy for model name rewriting
│   ├── pty.rs                  # PTY spawn, I/O piping, resize (ConPTY on Windows, PTY on Unix)
│   ├── memory.rs               # TOML graph parser + file watcher
│   ├── tab_store.rs            # Tab state persistence across sessions
│   ├── scaffold.rs             # Embedded project template extraction + memory detection
│   └── logging.rs              # Circular log buffer (500 entries)
│
├── assets/screenshots/         # App screenshots
├── public/                     # Static assets
├── dev.bat                     # Dev mode launcher (Windows)
├── build.bat                   # Production build script (Windows)
├── vite.config.ts              # Vite configuration
└── package.json                # Frontend dependencies
```

## Prerequisites

### All Platforms
- **Node.js 18+** — Required for copilot-api
- **Rust toolchain** — [rustup.rs](https://rustup.rs)
- **GitHub Copilot** access for the proxy chain

### Windows
- **Windows 10/11** — ConPTY is used for terminal emulation
- **MSVC build tools** — `vcvarsall.bat x64` (install via Visual Studio Build Tools)

### macOS
- **Xcode Command Line Tools** — `xcode-select --install`
- macOS 10.15+ recommended

### Linux
- **System dependencies** — Tauri requires several packages (varies by distro):
  ```bash
  # Debian/Ubuntu
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
    libssl-dev libayatana-appindicator3-dev librsvg2-dev

  # Fedora
  sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file \
    libappindicator-gtk3-devel librsvg2-devel

  # Arch
  sudo pacman -S webkit2gtk-4.1 base-devel curl wget file openssl \
    appmenu-gtk-module librsvg libappindicator-gtk3
  ```

## Getting Started

```bash
# Clone the repo
git clone https://github.com/<your-username>/Proximus.git
cd Proximus/app

# Install frontend dependencies
npm install

# Run in dev mode (hot-reload frontend + Rust backend)
npm run tauri dev

# Build production binary
npm run tauri build
```

### Platform-Specific Build Notes

**Windows:**
```powershell
# Using the helper scripts
.\dev.bat          # Dev mode
.\build.bat        # Production build
# Output: src-tauri/target/release/bundle/msi/*.msi
```

**macOS:**
```bash
npm run tauri build
# Output: src-tauri/target/release/bundle/dmg/*.dmg
#         src-tauri/target/release/bundle/macos/*.app
```

**Linux:**
```bash
npm run tauri build
# Output: src-tauri/target/release/bundle/deb/*.deb
#         src-tauri/target/release/bundle/appimage/*.AppImage
```

> **Cross-compilation:** Tauri does not support cross-compiling — you must build on the target platform. Use CI (e.g. GitHub Actions) with runners for each OS to produce all platform binaries.

## Key Tauri Commands

| Command | Description |
|---------|------------|
| `spawn_pty` | Start a new Claude Code terminal session |
| `write_pty` / `resize_pty` | Terminal I/O and resize |
| `start_processes` / `stop_processes` | Manage proxy chain lifecycle |
| `get_memory_graph` / `get_memory_state` | Read `.node-memory/` TOML files |
| `scaffold_project` | Extract project template to a new directory |
| `get_log_entries` | Retrieve structured log buffer |
| `get_statusline_stats` | Context window usage from statusline |

## Terminal Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+C` | Copy selected text (passes SIGINT when no selection) |
| `Ctrl+V` | Paste from clipboard |
| `Ctrl+Z` | Undo last typing chunk (keystrokes grouped by 600ms pauses; paste undoes as one block) |

## Roadmap

- [x] **Phase 1** — Core shell: Tauri app, PTY terminal, process management, proxy chain
- [x] **Phase 2** — Memory & UI: Live graph visualization, sidebar panels, logs, scaffolding
- [x] **Phase 3** — Multi-agent workspace: Parallel Claude sessions
- [x] **Phase 4** — CLI Mode Toggle: Switch between Claude CLI + Copilot Proxy and Copilot CLI directly

## Known Issues

| Issue | Status |
|-------|--------|
| Claude Code ASCII animation pollutes xterm scrollback | Open |
| Status badge doesn't reflect actual PTY state | Open |
| LogsPanel and MemoryGraphView use hard-coded colors (not theme-aware) | Open |
| Auto-migration `Enter` delay uses fixed 2s sleep — may be unreliable on slow machines | Open |

## Patch Notes

### v0.9 — CLI Mode Toggle (2026-04-24)

**New Features**
- **CLI Mode toggle** — Settings now has a Terminal section where you can switch between two modes:
  - **Claude CLI + Copilot Proxy** — the original mode: Claude Code runs through the model-rewrite proxy backed by GitHub Copilot's API
  - **Copilot CLI** — launches the `@github/copilot` terminal CLI directly, using your existing Copilot auth token with no proxy chain needed
- **Automatic file sync** — Switching modes translates your project config between formats:
  - Claude → Copilot: `CLAUDE.md` → `.github/copilot-instructions.md`, `.claude/skills/*/SKILL.md` → `.github/prompts/*.prompt.md`
  - Copilot → Claude: reverse direction, strips YAML frontmatter, adds import headers
- **Auto-seed on first open** — When opening a project in Copilot mode for the first time, if `.github/copilot-instructions.md` is missing but `CLAUDE.md` exists, the Copilot files are generated automatically before the terminal launches (and vice-versa). No manual `/init` needed.
- **Copilot auto-login** — The GitHub OAuth token stored by `copilot-api` is injected as `GITHUB_TOKEN`/`GH_TOKEN` into the Copilot CLI's PTY environment, so it starts authenticated without prompting for `/login`.
- **Auto-open Settings on launch** — If not signed in to Copilot, Settings opens automatically on startup.
- **Copilot model selection** — A new Copilot model dropdown in Settings lets you choose which model the CLI launches with (`--model` flag). Includes the full current model list: GPT-5.4, GPT-5.4 Mini/Nano, GPT-5.x Codex variants, GPT-4.1, GPT-4o, Claude Opus/Sonnet, Gemini 2.5 Pro/3.1.
- **Scan for latest models** — A ⟳ Refresh button next to the Copilot model list scans the installed CLI's `app.js` at runtime and extracts all model IDs — no app update needed when Copilot ships new models.
- **Claude model refresh** — A matching ⟳ Refresh button re-runs `claude models --json` live to pick up newly released Claude models.
- **Active mode models shown first** — In Settings → Models, the active CLI mode's model section always appears at the top; the inactive one is dimmed below it.
- **Dependency check updated** — Startup dependency dialog now also checks for the `@github/copilot` CLI package and offers to install it.
- **`sync_cli_files` Tauri command** — Exposes file sync to the frontend, callable on demand when switching modes.

**Settings UX**
- CLI Mode toggle moved above Skip Permission Prompts in the Terminal section
- Skip Permission Prompts toggle is disabled and explained when Copilot mode is active (the flag is Claude-only)
- Tabs are closed and the project picker is shown when CLI mode changes, so the next tab opens fresh in the new mode

**Bug Fixes**
- Fixed black bar between terminal and QuickActions bar caused by xterm.js row snapping — QuickActions overlaps the gap with `margin-top: -14px`, and the terminal wrapper clips the scrollbar with `clipPath: inset(0 0 14px 0)` to prevent overlap
- Fixed context token count showing cumulative `total_input_tokens` instead of current usage — now derived from `used_percentage × context_window_size`
- Fixed proxy chain starting even in Copilot mode — toolbar now reads `cli_mode` from persisted settings before deciding whether to launch proxies
- Fixed crash when switching Copilot → Claude — `cleanup_orphans()` was blocking the Tokio runtime with `std::thread::sleep`; moved to `spawn_blocking`
- Fixed process status badges not updating after mode switch — `apply_cli_mode` now emits `process-status` stopped events after calling `stop_all()`
- Fixed restored tabs (from `reopen_tab`) bypassing the auto-seed logic — both `spawn_tab_pty` and `reopen_tab` PTY paths now run the file sync check

---

### v0.8.2 — Model Rewrite Fix + Auth & Dependency Checks (2026-04-21)

**New Features**
- **Dependency detection on startup** — Proximus now checks that `claude` (Claude Code CLI) and `copilot-api` are installed before launching the proxy chain. If either is missing, a dialog appears listing the missing packages with their install commands. Click **Install** to run `npm install -g` automatically, or **Skip** to proceed anyway.
- New Tauri commands: `check_dependencies`, `install_dependencies`
- New `DependencyDialog.tsx` component integrated into `Toolbar.tsx` startup flow

**Fixes**
- **Auth flow no longer stuck on "Starting auth flow..."** — `start_copilot_auth` was only reading stdout, but `copilot-api auth` outputs the device code to stderr. Now reads both streams in parallel, so the device code and GitHub URL always appear in the Settings dialog.
- `child.wait()` moved to a dedicated thread so neither the stdout nor stderr reader blocks the other
- **Model version mapping now preserves the actual version number** — Previously the rewrite proxy hardcoded model targets (e.g. all Opus variants → `claude-opus-4.7`), which sent the wrong model ID to Copilot's API resulting in `model_not_supported` errors. The proxy now dynamically converts Claude's dash format to Copilot's dot format: `claude-opus-4-6` → `claude-opus-4.6`, `claude-opus-4-7` → `claude-opus-4.7`, etc.
- Fixed in both the built-in Rust proxy (`model_rewriter.rs`) and the standalone JS proxy (`model-rewrite-proxy.js`)
- Future-proof: any new model versions will work automatically without code changes

---

### v0.8.1 — GitHub Copilot Auth UI (2026-04-21)

**New Features**
- **Account section in Settings** — The Settings dialog now has an Account section at the top showing your GitHub Copilot connection status (● Connected / ○ Not connected) with Sign In, Re-authenticate, and Sign Out buttons.
- **Device flow UI** — Clicking Sign In launches `npx copilot-api auth`, streams stdout to the UI, and automatically opens the GitHub device authorisation page in your browser. The device code is displayed in the settings panel with an "Open GitHub ↗" button as a fallback.
- **Proxy restart auto-auth** — If the proxy restarts while you're signed out (e.g. after clicking the Restart button), the Settings dialog opens automatically and the browser tab pops up — no manual navigation required.
- **Sign out** — Deletes the Copilot token from all known candidate paths and stops the running proxy so the cached session is invalidated immediately.

**Fixes**
- Token path detection now checks four candidate locations in priority order, with `~/.local/share/copilot-api/github_token` (the XDG path used by current copilot-api versions) first — previously the app was always checking the wrong path and showing "Not connected" even when authenticated.
- Auth status check now requires the token file to have `>10` chars of content, preventing a false "Connected" badge when copilot-api creates an empty placeholder during the device flow handshake.
- Closing and reopening the Settings dialog no longer shows a stale "Connected" state from a previous incomplete auth — the pending auth process is cancelled on dialog unmount.
- Fixed repeated Sign In clicks opening multiple browser tabs — there is now exactly one `openUrl` call site (in `App.tsx`) guarded by a ref, regardless of how many event listeners are active.

**Backend**
- `AppState` gains `pending_auth_pid: Mutex<Option<u32>>` — stores the PID of the running auth process so `cancel_copilot_auth` can kill it by PID (via `taskkill /T /F` on Windows)
- New Tauri commands: `get_copilot_auth_status`, `start_copilot_auth`, `cancel_copilot_auth`, `sign_out_copilot`
- `copilot_token_candidates()` checks `~/.local/share`, `%LOCALAPPDATA%`, `~/.copilot-api`, and `%APPDATA%`

**Frontend**
- `SettingsDialog` lifted from `ProjectsView` into `App.tsx` — single `showSettings` state shared across the whole app
- `App.tsx` installs a permanent `copilot-auth-output` event listener that stays active even when Settings is closed; buffers `pendingAuthCode`/`pendingAuthUrl` and passes them as props to `SettingsDialog` so the device code is pre-populated on mount
- `AccountSection` cancels any pending auth process on unmount via `cancel_copilot_auth`

---

### v0.8 — Memory Rename & Auto-Migration (2026-04-21)

**Breaking Changes**
- **`.claude-memory/` renamed to `.node-memory/`** — All backend paths, scaffold templates, frontend references, and the project CLAUDE.md protocol now use `.node-memory/`. Existing projects with `.claude-memory/` are migrated automatically.

**New Features**
- **Auto-migration on tab open** — When a project with a legacy `.claude-memory/` folder is opened, Proximus atomically renames it to `.node-memory/` (with a copy-then-delete fallback for cross-device moves) and updates CLAUDE.md references. No user action required.
- **Memory structure sync** — On every tab open, Proximus diffs the project's `.node-memory/` against the current template and silently creates anything missing — new subdirs (`plans/`, `cold/`, `tools/`, `prompts/`) and files (`validate.py`, bootstrap/migrate prompts). A toast lists what was added.
- **`plans/` folder** — All implementation plans now live in `.node-memory/plans/` instead of the project root. CLAUDE.md protocol updated with a Plans section and `active_plan` field in `state.toml`.
- **`prompts/` folder** — Scaffold template now ships `bootstrap.md`, `bootstrap-repo.md`, `claude-md-protocol.md`, and `migrate.md` as ready-to-paste Claude Code prompts for every new project.
- **Settings Dialog** — A new modal settings dialog is accessible via the purple ⚙ Settings button in the Projects view. Houses the theme picker and provides a home for future settings sections.
- **Single-instance guard is release-only** — In debug builds the single-instance mutex no longer fires, preventing the dev window from being killed when a production instance is already running.

**Backend**
- `create_tab` now returns `CreateTabResult { tab_id, memory_migrated, dirs_added }` instead of a plain `String`
- New `scaffold::migrate_legacy_memory()` — atomic rename with `copy_dir_all` fallback; also runs `update_claude_md_references()` after rename
- New `scaffold::ensure_memory_structure()` — diffs project `.node-memory/` against template, creates missing dirs/files, returns list of additions
- New `sync_memory_structure` Tauri command — exposes structure sync to the frontend directly
- `update_claude_md_references()` now covers `.claude-memory/` → `.node-memory/` as an explicit replacement pair

**Frontend**
- `useTabStore` reads `memory_migrated` and `dirs_added` from `create_tab` result and fires toasts for each
- `ProjectsView` renders `SettingsDialog` as a modal on ⚙ button click
- `MigrationDialog` calls `sync_memory_structure` after scaffolding in both Migrate and Start Fresh flows, ensuring new template additions are always present
- `MigrationDialog` prompts now reference `.node-memory/` throughout

---

### v0.7 — Single Instance & Port Separation (2026-04-20)

**New Features**
- **Single-instance enforcement** — Launching Proximus when it's already running focuses the existing window instead of spawning a duplicate (via `tauri-plugin-single-instance`)
- **Dev/release port separation** — Debug builds use ports 4151/4152, release builds use 4141/4142, so both can run simultaneously without conflicts
- **Custom titlebar** — Replaced the native OS title bar with an integrated custom titlebar featuring drag-to-move, minimize, maximize/restore, and close buttons — styled to match the app theme

**Improvements**
- **Hidden xterm cursor** — The blinking cursor is now hidden during Claude streaming output to prevent visual jumpiness (cursor layer set to `display: none`, cursor color transparent)

### v0.6 — Cross-Platform Support (2026-04-20)

**New Features**
- **macOS and Linux support** — Process management, PTY spawning, and orphan cleanup now use platform-appropriate APIs (`lsof`/`kill` on Unix, `netstat`/`taskkill` on Windows)
- **Conditional compilation** — Windows-specific APIs (`CommandExt`, `creation_flags`) are gated behind `#[cfg(windows)]` so the Rust backend compiles cleanly on all platforms

**Changes**
- PTY shell selection: `cmd` on Windows, `bash` on macOS/Linux
- Screen clear command: `cls` on Windows, `clear` on macOS/Linux
- `copilot-api` spawned via `cmd /c npx` on Windows, `npx` directly on Unix

### v0.5 — Built-in Proxy & Log Cleanup (2026-04-20)

**Breaking Changes**
- **Model-rewrite proxy is now built into the Rust binary** — No longer requires `model-rewrite-proxy.js` or Node.js for the proxy layer (Node.js still needed for copilot-api). The exe is now self-contained for proxy functionality.

**Improvements**
- **Streaming SSE support** — The built-in proxy streams responses through instead of buffering, fixing timeouts on long Claude responses
- **Simplified logs panel** — Removed filter chips and source/level tags; logs now show clean timestamped output with color-highlighted warnings/errors
- **Removed phantom "claude" status badge** — Toolbar only shows copilot-proxy and model-rewriter badges (both backed by real status tracking)

### v0.4 — Theme System (2026-04-20)

**New Features**
- **14 built-in themes** — 10 dark (Tokyo Night, Catppuccin, Dracula, Nord, One Dark, Gruvbox, Solarized Dark, Rosé Pine, Kanagawa, GitHub Dark) and 4 light (Tokyo Night Light, Catppuccin Latte, GitHub Light, Solarized Light)
- **Live terminal recoloring** — Switching themes updates xterm.js ANSI colors in real-time without restarting the session
- **Theme tab in sidebar** — Visual theme picker with preview swatches in the Settings panel
- **CSS variable architecture** — All UI colors reference `var()` tokens; zero hard-coded hex in components (except LogsPanel/MemoryGraphView — known issue)
- **localStorage persistence** — Selected theme survives app restarts
- **Listener system** — `onThemeChange()` API lets any component react to theme switches

### v0.3 — Memory Migration (2026-04-19)

**New Features**
- **Memory Migration Dialog** — When adding a project with existing AI memory/context files (`.cursorrules`, `CLAUDE.md`, `AGENTS.md`, `.ai/`, ADRs, etc.) but no `.node-memory/`, a dialog offers three choices:
  - **Migrate** — Scaffolds `.node-memory/` and sends existing file contents to Claude for LLM-driven conversion into the structured TOML graph
  - **Start Fresh** — Scaffolds a blank `.node-memory/` ignoring existing files
  - **Skip** — No memory system created
- Detection is smart: only triggers on files with >10 lines (ignores empty stubs) and skips projects that already have `.node-memory/` or have no AI memory at all

**Backend**
- New Tauri commands: `detect_project_memory`, `scaffold_project_cmd`, `get_migration_file_contents`
- `create_tab` now defers scaffolding when existing memory is detected

### v0.2 — Terminal Input Overhaul (2026-04-19)

**New Features**
- **Ctrl+Z Undo** — Erases typing in time-grouped chunks (600ms grouping window). Each Ctrl+Z removes an entire burst of keystrokes. Pasted text always undoes as one block.
- **Terminal Shortcuts table** added to README

**Bug Fixes**
- **Fixed double-paste on Ctrl+V** — Browser paste event was firing alongside the custom clipboard handler, causing text to appear twice. Fixed with `event.preventDefault()` on the keydown event.

**Previous (v0.1)**
- Initial release: Tauri app, multi-tab PTY terminals, model-rewrite proxy chain, live memory graph, project scaffolding, context tracking, structured logging, quick actions, session recovery, Ctrl+C/V clipboard support, open-folder button on tabs

## License

[MIT + Commons Clause](../LICENSE) — Free to use, modify, and integrate commercially. Cannot be resold as a standalone product or service.

---

<div align="center">
<sub>Built with coffee and Claude</sub>
</div>
