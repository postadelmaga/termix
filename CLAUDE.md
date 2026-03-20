# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**Termix** is a Yakuake-style dropdown terminal for KDE Wayland. It wraps an existing terminal (foot or kitty) and provides:
- Animated slide-down/up toggle via a global shortcut (F12)
- Multiplexing via Zellij running inside the terminal
- KDE-native integration via DBus (KWin, KGlobalAccel)

## Build

```bash
cargo build
cargo build --release
cargo run
```

No C++ or CMake required — pure Rust.

## Architecture

```
src/
├── main.rs        — entry point: loads config, inits window, spawns terminal, runs event loop
├── ui/
│   └── window.rs  — DropdownWindow: Wayland layer-shell surface (overlay, top-anchored)
├── terminal/
│   └── mod.rs     — Terminal: spawns/manages foot or kitty as a child process with zellij
└── config/
    └── mod.rs     — Config: loads/saves ~/.config/termix/config.toml (shortcut, backend, size)
```

**Key design decisions:**
- The terminal process (foot/kitty) is a child of termix — termix does not implement a terminal emulator
- The dropdown surface is a `zwlr_layer_shell_v1` Wayland overlay (OVERLAY layer, TOP|LEFT|RIGHT anchor) — this is the standard protocol for panels/dropdowns on Wayland, supported by KWin
- Global shortcut registration uses KDE DBus (`org.kde.kglobalaccel`) via zbus — no Qt required
- KWin window positioning for the child terminal window uses `org.kde.KWin` DBus scripting
- The Wayland event loop and the tokio async runtime run concurrently

## Stack

| Layer | Technology |
|---|---|
| Core / async | Rust + tokio |
| Wayland window | smithay-client-toolkit + wayland-protocols-wlr (layer-shell) |
| KDE DBus | zbus |
| Config | toml + serde |
| Multiplexing | Zellij (external process) |
| Terminal backend | foot or kitty (child process) |

## Environment

- KDE Plasma 6 on Wayland
- kwindowsystem 6.24, kwin 6.6

## Why not Qt/cxx-qt

cxx-qt 0.7.x has a proc-macro bug on Rust ≥ 1.87 that renders `#[cxx_qt::bridge]` unusable
(`non-foreign item macro in foreign item position: include`). The layer-shell approach avoids
Qt entirely and is more idiomatic for a Wayland-native app.
