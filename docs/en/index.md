---
title: Control Center
description: Configure the persistent parts of your ARGVUS desktop.
---

The Control Center is ARGVUS's keyboard-first settings application. Use it when you want to configure the desktop rather than perform a one-time action.

## Control Center and Control Panel

| Use… | For… |
| --- | --- |
| **Control Center** | Themes, wallpapers, effects, layout, taskbar behavior, fonts, default applications, keyboard and input settings, language, region and system configuration. |
| **Control Panel** | Current status and frequent actions such as volume, brightness, network, notifications, power and session controls. |

The Control Panel can expose quick controls for a setting that is configured permanently in Control Center. For example, use the panel for a quick appearance or display action, and Control Center when you want to change the desktop configuration behind it. See [Control Panel](/docs/argvus-control-panel/) and [Where to configure things](/docs/user-guide/where-to-configure/).

## Main areas

The current Control Center home provides these areas:

- **Appearance** — themes, modes, accents, wallpapers, effects, taskbar and panel layout, widget telemetry and Control Panel cards.
- **Default applications** — select installed applications for supported roles.
- **Fonts** — choose the font targets and font rendering settings exposed by ARGVUS.
- **Locale & Region** — time zone, date and time, regional locale, system locales and keyboard configuration.
- **System** — host, users, groups and system administration pages available to the current installation.
- **Displays, network, session and other domains** — available through the home search and domain routes when the corresponding optional capabilities are installed.

The exact provider pages can depend on installed packages and permissions. Do not assume that a missing optional domain is a broken Control Center.

## Finding a setting

The home screen registers settings by title, category, identifier and keywords. Search is accent-insensitive and accepts practical terms such as `theme`, `wallpaper`, `shortcut`, `mouse`, `monitor`, `network`, `user` and `DPI`. Exact title matches are ranked first, followed by prefix and keyword matches.

The visible menu is capability-aware. A route can be absent when its provider is not installed, when the hardware is not available, or when the current session cannot provide the required capability. Search is therefore also a useful way to discover what this installation can configure.

The main settings tree currently includes:

| Area | Current destinations |
| --- | --- |
| Appearance | Themes, theme modes, highlight color, wallpapers, effects, spaces/borders/position, taskbar position and spacing, taskbar utility group, widget telemetry and Control Panel cards |
| Input and keyboard | Mouse and touchpad, keyboard layout/variant, console keymap, keyboard shortcuts |
| Locale & Region | Language, time zone, date and time, regional locale, system locales and keyboard |
| Applications | Default applications and per-category selectors |
| System | Hostname, firewall, users, groups and system administration |
| Hardware | Summary, CPU, GPU, memory, power and devices |
| Services and diagnostics | Services, boot, packages, storage and diagnostics |
| Connectivity | Network, audio and Bluetooth |
| Session | Components, autostart, diagnostics and logs |
| Displays | Resolution, refresh rate, scale, position, orientation, primary display, VRR and HDR when supported |

Some of these are full pages and some are subpages reached through search or a domain summary. The exact list is determined by the installed ARGVUS providers.

## What each area is for

### Appearance

Appearance is the main customization area. It includes themes and Sticky/Float modes, accent colors, bundled or custom wallpapers, shared effects, taskbar and window spacing, borders, edge thickness, telemetry blocks and the visibility/order of Control Panel cards. These controls coordinate with the appearance and session components instead of changing only the Control Center window.

### Fonts

Fonts lets you select a family and size for the taskbar, telemetry/system information, Control Panel, ARGVUS system interface, applications, terminal and browser. It also exposes font rendering settings such as antialiasing, hinting, subpixel mode and DPI. The default primary family is IBM Plex Mono; individual targets can be changed independently and restored from the Fonts page.

### Default applications

Default applications selects installed programs for roles such as terminal, file manager, text editor, terminal editor, browser, image viewer, PDF viewer, video player, audio player, archive tool and launcher. The selection is saved by ARGVUS and may update standard XDG associations.

### Locale & Region

This area contains time zone, date and time, regional locale, system locales and keyboard settings. Keyboard settings include layout, variant and console keymap. The home search also exposes keyboard shortcuts and mouse/touchpad settings as direct settings routes.

### Hardware and displays

When the corresponding capabilities are installed, the Control Center can show hardware information such as CPU, GPU, memory, power and devices. The input page configures mouse and touchpad behavior; the display area handles monitor resolution, refresh rate, scale, position, orientation, primary display, VRR and HDR where supported by the running session.

### Connectivity and audio

The network area exposes the installed network provider's status and connection pages, including Wi-Fi, Ethernet, VPN, DNS, proxy and firewall routes when available. Bluetooth is conditional on Bluetooth support. The audio area is provided when its optional capability is enabled and integrates with the installed audio service.

### Power and session

Power provides the system power controls and policy pages available to the installation, including the Keep Awake toggle. When enabled, Keep Awake prevents the ARGVUS idle policy from starting automatic screen-lock and display-power timers. Session exposes ARGVUS session status, components, autostart, diagnostics and logs. These pages may require system permissions and should be distinguished from the quick actions in the Control Panel.

### System tools

The system section can expose boot information, packages, services, storage, diagnostics, user/group administration and system information. These are administrative or diagnostic tools; a page may be read-only or require authorization depending on the operation.

### Users and groups

When the account provider is available, open **Settings → System → Users** or search for **users**. The interface can list normal and system accounts, create a user, edit account metadata, manage supplementary and primary groups, change or lock a password, unlock it, require a password change at the next login, and set or remove an avatar.

User creation asks for a username, full name, shell, supplementary groups and an optional password confirmation. A blank password creates the account with its password locked; it does not silently create a usable password. Existing accounts expose separate destructive actions to delete the account while keeping its home directory or to delete the home directory as well. Read the confirmation text carefully before choosing the latter. Account operations are privileged and may open the system authorization prompt. Avatars can be consumed by the greeter through the account's standard face image integration.

Groups have their own list and edit pages, including member management and group deletion. The Control Center does not replace the system's account policy: protected system accounts and operations requiring authorization remain subject to the operating system's restrictions.

### Control Center configuration

The **Configuration** page currently controls decorative icons in the Control Center layout. It changes the settings application's presentation; it is not a switch that disables ARGVUS services or removes settings providers. Saving this option can require authorization, while the visual change is applied to the current application.

## Search and optional capabilities

The home screen has a searchable registry of routes. Search terms include user-facing concepts such as appearance, themes, wallpapers, fonts, keyboard shortcuts, mouse, touchpad, displays, network and session. Optional providers are included only when their feature is compiled and their runtime capability is detected, so the visible menu is intentionally capability-aware.

While the Home search is active, printable keys are entered into the query, including `j` and `k`; use the `↑` and `↓` arrow keys to move through matching routes. Press `Enter` to open the selected route, `Backspace` to edit the query, and `Esc` to clear the query or leave search mode.

## Appearance workflow

Open **Appearance** to see the integrated visual controls. Its pages are:

- **Themes** — choose a bundled family and its Sticky or Float mode, or manage custom theme profiles.
- **Accents** — choose the accent or enter a valid six-digit RGB color.
- **Wallpapers** — choose a bundled wallpaper or select a custom file.
- **Spaces, Borders & Position** — taskbar position, taskbar/shell spacing, window gaps, borders and edge thickness.
- **Effects** — enable or disable the shared visual-effects state.
- **Widget Telemetry** — enable the telemetry surface and select its available blocks.
- **Control Panel** — enable, disable and reorder panel cards.

These actions update the logical ARGVUS state and apply the affected runtime configuration. See [Appearance](/docs/user-guide/appearance/), [Themes](/docs/argvus-themes/) and [Windows and layout](/docs/argvus-hyprland/windows-and-layout/).

## Keyboard use

The settings application supports keyboard navigation. Use the arrow keys or `j`/`k` to move, `Enter` to open or apply, `Esc` to go back, `/` to search lists, `Tab` to move between fields or actions, and `?` for contextual help. A page can show a **Reset defaults** or **Restore all shortcuts** action when that page supports it.

## Persistence and reset

Settings pages apply changes through their owning provider and save the supported user state. Appearance changes are stored in the canonical configuration, and `argvus-config` is the only component that writes the derived consumer files under `~/.config/argvus/data/generated/`; those generated files are not the place to make a permanent edit. Input and keybinding pages have their own persisted state and reset actions. Other system changes may require permissions or a service reload.

There is no global reset for every ARGVUS setting. Restore a change from the page that owns it, or use the relevant feature's documented recovery procedure.

Reset support is deliberately per domain:

- **Appearance** provides a reset for the highlight color to the active theme default. It does not provide a single global appearance reset; restore other appearance values from their own controls.
- **Fonts** can restore all font settings, a target, or an individual setting.
- **Default applications** can restore all defaults, a category, or an individual selector.
- **Keyboard shortcuts** can restore one binding or all bindings and then reload the generated session bindings.
- **Other system pages** expose reset, apply, delete or restore actions only when the underlying provider supports them.

Removing a user preference can make a provider fall back to its packaged/default state, but deleting files by hand is not a general recovery procedure. Prefer the page's reset action or the documented feature-specific command.

## CLI entry points

The installed binary is `argvus-control-center`. It can open focused areas directly, for example:

```sh
argvus-control-center appearance themes
argvus-control-center appearance wallpapers
argvus-control-center input
argvus-control-center keybindings
```

Run `argvus-control-center --help` for the routes available in the installed version. The command-line route is a shortcut into the same settings application, not a separate configuration system.

Useful direct routes include `apps`, `fonts`, `locale`, `input`, `keybindings`, `language`, `config`, `system`, `hardware`, `services`, `network`, `audio`, `bluetooth`, `boot`, `packages`, `storage`, `diagnostics`, `power`, `session`, `displays` and `appearance`. Appearance also accepts focused routes such as `themes`, `wallpapers`, `accents`, `effects`, `spaces`, `taskbar` and `widget-telemetry`; display accepts `resolution`, `refresh`, `scale`, `position`, `orientation`, `primary`, `vrr` and `hdr`. Use the installed `--help` output when scripting because availability still depends on the installed build.

## Related

- [First configuration](/docs/getting-started/)
- [Control Panel](/docs/argvus-control-panel/)
- [Appearance](/docs/user-guide/appearance/)
- [Keyboard shortcuts](/docs/argvus-hyprland/keyboard-shortcuts/)
- [Mouse and touchpad](/docs/argvus-hyprland/input/)
