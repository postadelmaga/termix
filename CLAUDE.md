# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**Termix** is a Yakuake-style dropdown terminal for KDE Wayland. It wraps an existing terminal (foot or kitty) and provides:
- Animated slide-down/up toggle via a global shortcut (F12)
- Multiplexing via Zellij running inside the terminal
- KDE-native integration (KWin DBus, global shortcuts, system tray)

## Build

```bash
cargo build
cargo build --release
cargo run
```

cxx-qt requires Qt6 dev headers (`qt6-base`) and the build script in `build.rs` bridges Rust ↔ Qt via CMake under the hood.

## Architecture

```
src/
├── main.rs        — entry point, initializes Qt app and global shortcut listener
├── ui/            — Qt window (QWidget), dropdown animation, system tray
├── terminal/      — spawns and manages foot/kitty as a child process
└── config/        — reads/writes user config (shortcut key, terminal backend, height %)
```

**Key design decisions:**
- The terminal process (foot/kitty) is embedded or managed as a child — termix is a thin manager, not a terminal emulator
- KWin window management happens via **zbus** (DBus) calls to `org.kde.KWin`
- The Qt layer (cxx-qt) handles only the visible window, animation, and global shortcut registration
- Zellij runs inside the terminal for multiplexing — termix does not implement its own multiplexer

## Stack

| Layer | Technology |
|---|---|
| Core logic | Rust |
| Qt bridge | cxx-qt 0.7 |
| KDE/DBus | zbus 5 |
| Async runtime | tokio |
| Multiplexing | Zellij (external) |
| Terminal backend | foot or kitty (external) |

## Environment

- KDE Plasma 6 on Wayland
- Qt 6.10, kwindowsystem 6.24
