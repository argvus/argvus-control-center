# ARGVUS Control Center

[![CI](https://github.com/argvus/argvus-control-center/actions/workflows/ci.yml/badge.svg)](https://github.com/argvus/argvus-control-center/actions/workflows/ci.yml)
[![Release](https://github.com/argvus/argvus-control-center/actions/workflows/release.yml/badge.svg)](https://github.com/argvus/argvus-control-center/actions/workflows/release.yml)
[![License](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)

## Introduction

`argvus-control-center` is the keyboard-first settings application for the
ARGVUS Linux desktop. It provides a single terminal user interface (TUI) for
system configuration, hardware information, connectivity, services, boot,
packages, storage, diagnostics, appearance and user preferences.

The project combines shared ARGVUS navigation, semantic themes and translated
content in a responsive full-screen terminal application. Mouse interaction is
supported where useful, while every primary workflow remains accessible from
the keyboard.

## Requirements

- Rust 1.95.0 (`rust-toolchain.toml`)
- A terminal supported by Crossterm
- ARGVUS Control Center resources under `/etc/argvus/control-center`
- The sibling `argvus-i18n` checkout when building the ARGVUS workspace locally
- Optional system tools are detected by each domain and are not required for
  the application to start

## Build and test

From this repository:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --release --workspace
```

The equivalent project shortcuts are:

```bash
make check
make release
```

## Run

```bash
./target/release/argvus-control-center
```

Direct routes are also available:

```bash
./target/release/argvus-control-center apps
./target/release/argvus-control-center fonts
./target/release/argvus-control-center network wifi
./target/release/argvus-control-center about
```

Use `--help` to see all supported routes.

## Keyboard controls

- `↑` / `↓` or `j` / `k`: navigate
- `Enter` or `→`: open or apply
- `/`: focus global search on Home, or local search where supported
- `Esc` or `←`: clear, cancel or go back
- `Tab` / `Shift+Tab`: move between tabs or actions
- `?`: contextual help
- `q`: quit

The Home search updates results as you type and opens registered pages directly
through their real Control Center routes. While the Home search is active, type
any printable character—including `j` and `k`—and use `↑` / `↓` to select a
result. Mouse clicks remain supported for focusing the search field and
selecting interface elements.

## Architecture

This repository is a Cargo workspace with focused crate boundaries:

- `argvus-control-center`: binary, Home, CLI and global routing
- `argvus-control-center-core`: shared capabilities, jobs, validation and search registry
- `argvus-tui`: terminal lifecycle, shared chrome and reusable TUI primitives
- `argvus-theme`: semantic ARGVUS theme resolution
- `argvus-control-center-settings`: default applications, fonts and system settings
- `argvus-control-center-*`: domain-specific pages and backends
- `argvus-about`: system information, project content and links

Domain backends keep probes and system operations outside the render loop.
Optional capabilities degrade to an unavailable state instead of preventing the
Control Center from starting. Privileged operations use the existing typed
`system-settings` boundary.

## Internationalization and themes

Translations are provided by the shared [`argvus-i18n`](../argvus-i18n)
project, currently with English and Brazilian Portuguese coverage. The active
ARGVUS theme is resolved by `argvus-theme` and consumed through semantic colors
by the Home, settings pages and domain views.

## Terminal icons

Interface icons use the Material Design Icons set from Nerd Fonts (`nf-md-*`)
and are intended for a single-cell Symbols Nerd Font Mono terminal font. The
catalog and shared icon/label spacing live in `argvus-tui::icons`.

## License

ARGVUS Control Center is distributed under the
[GNU General Public License v3.0](LICENSE).
