# 🎯 Usage Tracker

Track AI provider usage across ChatGPT, Claude, Antigravity, OpenCode, Kilo Code, and Cursor.

Native desktop app written in Rust with [egui](https://github.com/emilk/egui).

![Rust](https://img.shields.io/badge/rust-1.74+-orange)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20Mac-blue)
![License](https://img.shields.io/badge/license-MIT-green)

## Features

- 📊 **Real-time usage** — ChatGPT (5h session + weekly), Claude (5h session + weekly)
- 🔗 **Account management** — Connect/disconnect providers from the GUI
- 🖥️ **Desktop app** — Full GUI with Overview, Connections, and Settings tabs
- 📌 **Compact widget** — Always-on-top mini window
- 🎯 **System tray** — Icon in hidden icons area (Windows)
- 🚀 **Auto-start** — Launch with Windows on boot
- 🎨 **6 providers** — Claude, ChatGPT, Antigravity, Kilo Code, Cursor, OpenCode

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs) (1.74+)

### Install

```bash
git clone https://github.com/juanb-casso/usage-tracker.git
cd usage-tracker
cargo build --release
```

### Usage

```bash
# Full desktop app
usage-tracker gui

# Compact widget (always on top)
usage-tracker widget

# System tray (Windows only)
usage-tracker tray

# Auto-start with Windows
usage-tracker install

# CLI check
usage-tracker check --provider all

# Watch mode (auto-refresh)
usage-tracker watch --interval 60
```

## Providers

| Provider | Data Source | Status |
|----------|-----------|--------|
| **Claude** | `~/.claude/.credentials.json` | ✅ HTTP API |
| **ChatGPT** | `~/.codex/auth.json` | ✅ HTTP API |
| **Antigravity** | Local language server probe | ⚠️ Needs app running |
| **Kilo Code** | Config detection | 🟡 No API |
| **Cursor** | `~/.cursor/` detection | 🟡 No API |
| **OpenCode** | Config detection | 🟡 No API |

## Configuration

Config file: `~/.config/usage-tracker/config.toml` (Linux/Mac) or `%APPDATA%\usage-tracker\config.toml` (Windows)

```toml
enabled_providers = ["claude", "chatgpt", "antigravity", "kilocode", "cursor", "opencode"]
refresh_secs = 300
```

## Project Structure

```
src/
├── main.rs            — CLI entry point + subcommands
├── gui.rs             — egui desktop app
├── tray.rs            — System tray (Windows)
├── provider.rs        — Provider trait + ProviderUsage struct
├── config.rs          — TOML config management
├── display.rs         — Terminal table + colors
└── providers/
    ├── mod.rs         — Factory functions
    ├── claude.rs      — Claude OAuth API
    ├── chatgpt.rs     — ChatGPT/Codex API
    ├── antigravity.rs — Local language server probe
    ├── kilocode.rs    — Kilo Code detection
    ├── cursor.rs      — Cursor detection
    └── opencode.rs    — OpenCode detection
```

## License

MIT
