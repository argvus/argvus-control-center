# argvus-control-center

`argvus-control-center` is the keyboard-first terminal control center for ARGVUS.
It consolidates the working features of `argvus-settings-term` and
`argvus-about-term` into one full-screen TUI without introducing GTK or a second
configuration system.

The home screen opens Default Apps, Fonts, or About. Settings retain the existing
list/search workflows and ARGVUS backends. About retains its distinct five-tab
layout, responsive logo, system information, links, credits, donation page, and
full copyright text.

## Requirements

- Rust 1.95 or newer
- a terminal supported by Crossterm, including Foot on Wayland
- `fontconfig` (`fc-list`) for installed-font discovery
- the ARGVUS settings resources under `/etc/argvus-settings`
- `xdg-open` for About links
- the existing ARGVUS settings backend crates in the sibling `argvus-settings`
  checkout when building from this source tree

The settings backends may invoke the same user-level tools as the GTK settings
application, including `gsettings`, `xdg-mime`, and ARGVUS refresh helpers. No
operation uses `sh -c` or requests root privileges.

## Build and test

```bash
cargo build --release --workspace
cargo test --workspace
```

The full validation used by the project is:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --release --workspace
```

Install into a staging root or the configured prefix with:

```bash
make install DESTDIR=/tmp/argvus-control-center-package
```

## Run

```bash
./target/release/argvus-control-center
./target/release/argvus-control-center apps
./target/release/argvus-control-center fonts
./target/release/argvus-control-center about
./target/release/argvus-control-center about credits
```

`--help` and `--version` run without entering the alternate screen. Direct routes
are suitable for ARGVUS keyboard shortcuts such as `SUPER+ALT+S`.

## Controls

- `Up` / `Down` or `j` / `k`: navigate lists and About content
- `Right` or `Enter`: open the selected home/settings item
- `Left` / `Right` or `h` / `l`: change About tabs
- `Tab` / `Shift+Tab`: change About tabs
- `PageUp` / `PageDown`, `Home`, `End`: scroll About and long settings lists
- `/`: search application and font lists
- `+` / `-`: change a pending font size
- `Enter`: apply a setting or open the selected About link
- `Esc`: cancel search, close a popup, or return to the previous level
- `?`: contextual keyboard help
- `q`: quit from every page, including search mode

The terminal uses raw mode, the alternate screen, and a hidden cursor. A shared
RAII guard plus panic hook restores all three on normal exit and unwind.

## Architecture

This repository is a Cargo workspace with six deliberate boundaries:

- `argvus-control-center`: binary, Home, CLI, global route and event composition
- `argvus-tui`: reusable terminal lifecycle and shared chrome/widgets
- `argvus-theme`: global ARGVUS theme consumer and semantic TUI theme
- `argvus-i18n`: shared language detection and translations
- `argvus-control-center-settings`: default apps and fonts domain
- `argvus-control-center-about`: tab documents, system data, logo and links

Rendering consumes cached state. Application discovery, fontconfig, system probes,
theme files, and logo decoding occur at startup or explicit update points, never
inside per-frame drawing code. See [docs/MIGRATION.md](docs/MIGRATION.md) for the
source-to-workspace map.

## ARGVUS themes

ARGVUS remains the source of truth. `argvus-theme` reads the active name from
`$XDG_CONFIG_HOME/argvus/.active-theme`, follows the shared CSS resources and
imports in `/etc/argvus-settings`, and consumes the generated ARGVUS theme cache
used by other integrated applications. `ARGVUS_SETTINGS_RESOURCE_DIR` and the
normal XDG/ARGVUS config overrides are respected for development and user setups.

The resolver maps the global `argvus_*` palette to one semantic `Theme` used by
Home, Settings, About, tabs, links, selections, borders, and status messages. A
central Aether fallback keeps the application readable when a file or color is
missing. Official themes therefore require no palettes compiled into this binary;
reopening the application after a global theme change loads the new theme, in the
same consumer-oriented model used for Superfile and other ARGVUS terminal tools.

## Languages

All application areas share `argvus-i18n`. It respects `ARGVUS_LANG`, then the
standard locale variables, and currently preserves the Portuguese and English
coverage of the source TUIs. Menu labels, tabs, breadcrumbs, controls, help,
feedback, errors, and About content all pass through that layer.

## Logo and terminal graphics

The About system tab preserves the existing renderer. It loads and rasterizes the
SVG once, uses Kitty/Sixel/iTerm2 when detected, then falls back to colored Unicode
half blocks or ASCII. Resizing only rebuilds the cached target when dimensions
change. Set `ARGVUS_ABOUT_IMAGE=graphics`, `halfblocks`, or `ascii` to force a
backend for diagnostics.

## Screenshots

Place release screenshots in `docs/screenshots/`:

- `control-center-home.png`
- `default-apps.png`
- `fonts-search.png`
- `about-system.png`
- `about-credits.png`
