# Development Guide

## Layout

```
crates/
├── argvus-control-center/              binary, Home, CLI, route composition
├── argvus-tui/                         terminal lifecycle, shared chrome/widgets
├── argvus-theme/                       ARGVUS theme consumer + semantic TUI theme
├── argvus-i18n/                        language detection and translations
├── argvus-about/                       tab documents, system data, logo, links
├── argvus-control-center-settings/     settings domain (locale, boot, appearance, ...)
├── argvus-control-center-apps/         default apps + app selector
├── argvus-control-center-core/         capability detection, ProcessRunner, config
├── argvus-control-center-hardware/     hardware info backend
├── argvus-control-center-services/     systemd services backend
├── argvus-control-center-network/      DNS, Bluetooth, network backend
├── argvus-control-center-audio/        PulseAudio/PipeWire backend
├── argvus-control-center-bluetooth/    Bluetooth adapter/device backend
├── argvus-control-center-boot/         bootloader management
├── argvus-control-center-packages/     package manager backend
├── argvus-control-center-storage/      lsblk/S.M.A.R.T. backend
└── argvus-control-center-diagnostics/  system health checks
packaging/arch/
├── ci/PKGBUILD                        Arch Linux package (remote source)
├── local/PKGBUILD                     Arch Linux package (local tarball)
└── usr/share/applications/            .desktop file
src/usr/share/argvus/control-center/   shipped config, themes, and docs
tools/sh/pkgbuild_local.sh             local source archive and makepkg driver
```

## Commands

```sh
cargo build --release --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt && cargo fmt --check
make validate          # full validation pipeline
make package           # build the local Arch package
```

## Conventions

- **Library-first**: domain logic belongs in the corresponding crate; the
  binary only parses, dispatches and renders.
- **Config override**: `AppConfig::config_path()` respects `ARGVUS_CONFIG_PATH`
  for tests and development. Never hard-code a config path in library code.
- **Theme loader**: `argvus-theme::Loader` respects
  `ARGVUS_CONTROL_CENTER_RESOURCE_DIR` and falls back to
  `/etc/argvus/control-center`, then the `resources/` directory in the source
  tree.
- **Icons**: use `AppConfig::icon(glyph)` for every decorative prefix. The
  function returns `""` when icons are disabled; wrap with `non_empty()` for
  label/detail fields.
- **Privileged operations**: use the `ProcessRunner` with explicit argument
  vectors and timeouts. The settings backend may run `pkexec` for elevated
  commands.
- **Language layer**: all user-visible strings go through `argvus-i18n` (`tr()`).
  Never embed hard-coded Portuguese or English strings in the UI.

## Testing rules

- The main test target is `cargo test --workspace` (290+ tests). All must pass
  before any commit.
- UI tests cover row generation, navigation bounds, search, and confirmation
  flows; they do not touch the terminal.
- Settings tests mock system files via `std::env::set_var("ARGVUS_CONFIG_PATH")`
  and temporary directories; they never write to `/etc` or the real `$HOME`.

## Arch Linux packaging

The `packaging/arch/` directory contains the CI and local PKGBUILDs plus the
desktop file shipped in the Arch package. The package functions install the
binary, compatibility symlink, desktop file, config, themes, docs, SVG, and
license. The `backup=()` array in each PKGBUILD preserves the user's
`config.toml` on package upgrade.

```sh
make package          # local build through tools/sh/pkgbuild_local.sh
(cd packaging/arch/ci && makepkg -p PKGBUILD --printsrcinfo)
```

## Release flow

1. Bump `workspace.version` in `Cargo.toml` and `pkgver` in both PKGBUILDs.
2. Update the changelog under `docs/changelogs/`.
3. Tag the release (`git tag vX.Y.Z`); CI builds and publishes the Arch
   package to the ARGVUS repository.
