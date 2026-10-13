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

- **Appearance** — themes, modes, accents and wallpapers.
- **Taskbar** — position, spaces, icons, date and time, reusing the effects pages implemented in Appearance.
- **Control Panel** — card sessions, order and transparency, reusing the effects pages implemented in Appearance.
- **Widget Telemetry** — sessions and transparency for the telemetry widget, reusing the effects pages implemented in Appearance.
- **Default applications** — select installed applications for supported roles.
- **Snippets** — text saved for `argvus snippets`, typed with `SUPER + ALT + N`.
- **SSH keys** — key pairs in `~/.ssh` managed by `argvus keys`: generate, load into the agent, unload and delete.
- **GPG keys** — the keyring managed by `argvus gpg`: list, generate, export and import public keys, and choose the pinentry.
- **Fonts** — choose the font targets and font rendering settings exposed by ARGVUS.
- **Locale & Region** — time zone, date and time, regional locale, system locales and keyboard configuration.
- **System** — host, users, groups and system administration pages available to the current installation.
- **Hyprland** — keyboard shortcuts, window rules and the window-effects pages reused from Appearance (window spaces, animations, blur, borders), plus the installed Hyprland version.
- **Displays, network, session and other domains** — available through the home search and domain routes when the corresponding optional capabilities are installed.

The exact provider pages can depend on installed packages and permissions. Do not assume that a missing optional domain is a broken Control Center.

When the grid has more entries than fit on screen, a scrollbar appears along the right edge to show there is more below.

## Finding a setting

The home screen registers settings by title, category, identifier and keywords. Search is accent-insensitive and accepts practical terms such as `theme`, `wallpaper`, `shortcut`, `mouse`, `monitor`, `network`, `user` and `DPI`. Exact title matches are ranked first, followed by a title prefix, any word of the title or category that contains the typed text, and finally keyword matches — so typing any part of a word (not only its start) can still find the destination.

The visible menu is capability-aware. A route can be absent when its provider is not installed, when the hardware is not available, or when the current session cannot provide the required capability. Search is therefore also a useful way to discover what this installation can configure.

The main settings tree currently includes:

| Area | Current destinations |
| --- | --- |
| Appearance | Themes, theme modes, highlight color, wallpapers, effects, spaces/borders/position |
| Taskbar | Position, spacing, utility group, icons, date and time |
| Control Panel | Card visibility/order, sessions and transparency |
| Widget Telemetry | Sessions and transparency |
| Input and keyboard | Mouse and touchpad, keyboard layout/variant, console keymap |
| Locale & Region | Language, time zone, date and time, regional locale, system locales and keyboard |
| Applications | Default applications and per-category selectors, Projects, Snippets, SSH keys, GPG keys |
| Hyprland | Keyboard shortcuts, Window rules, and the window-effects pages reused from Appearance (Window spaces, Animations, Blur, Borders), plus the installed Hyprland version |
| System | Hostname, firewall, users, groups and system administration |
| Hardware | Summary, CPU, GPU, memory, power and devices |
| Services and diagnostics | Services, Dev Services, boot, packages, storage and diagnostics |
| Connectivity | Network, audio and Bluetooth |
| Session | Components, autostart, diagnostics and logs |
| Displays | Resolution, refresh rate, scale, position, orientation, primary display, VRR and HDR when supported |

Some of these are full pages and some are subpages reached through search or a domain summary. The exact list is determined by the installed ARGVUS providers.

## What each area is for

### Appearance

Appearance is the main customization area. It includes themes and Sticky/Float modes, accent colors and bundled or custom wallpapers. Taskbar, Control Panel and Widget Telemetry are separate top-level areas (see below) that reuse Appearance's shared transparency page for their own surfaces. Blur is one global setting, under Hyprland > Blur, and it applies to every ARGVUS surface that has blur enabled. That page edits its Enable switch and the Hyprland blur values (Size, Passes, Brightness, Noise, Contrast, Vibrancy and Vibrancy darkness) as a draft, and nothing changes until **Apply** is selected. Animations works the same way under Hyprland > Animations: its Enable switch, the Custom curves (Bézier, Spring, Linear and Ease-out) and the five animation categories form one draft that is applied together with **Apply**. These controls coordinate with the appearance and session components instead of changing only the Control Center window.

### Taskbar

Taskbar configures the panel's position, the spacing around it and its windows, the utility icon group, and the date/time blocks, plus its own transparency.

### Control Panel

Control Panel configures the visibility and order of its quick-action cards, plus its own sessions and transparency.

### Widget Telemetry

Widget Telemetry configures the system-information widget's sessions and transparency.

### Fonts

Fonts lets you select a family and size for the taskbar, telemetry/system information, Control Panel, ARGVUS system interface, applications, terminal and browser. It also exposes font rendering settings such as antialiasing, hinting, subpixel mode and DPI. The default primary family is IBM Plex Mono; individual targets can be changed independently and restored from the Fonts page.

### Default applications

Default applications selects installed programs for roles such as terminal, file manager, text editor, terminal editor, browser, image viewer, PDF viewer, video player, audio player, archive tool and launcher. The selection is saved by ARGVUS and may update standard XDG associations.

### Snippets

Snippets is the page for the text that `argvus snippets` types with `SUPER + ALT + N`. It is in **Applications → Snippets** (search `snippets` or run `argvus-control-center snippets`).

- **New snippet** — type a **Name** and a **Content**, then **Save snippet**. A name that already exists replaces that snippet's content.
- **Saved snippets** — listed with their position (`1  email`) and the content on one line. Enter on the content edits it. **Type** types the snippet, and **Remove** asks for confirmation.
- **Open picker** — opens the same picker as `SUPER + ALT + N`.

Typing starts 3 seconds after you press Enter, because the Control Center itself has the focus at that moment. Click the field that should receive the text within that time. The page only edits one-line content; a multi-line snippet can be created in a terminal with `argvus snippets add`.

The page reads the entries from `argvus-config` and writes them through `argvus snippets`, so the launcher keeps the validation. When `argvus snippets` is not installed, the page says so.

### SSH keys

SSH keys is the page for the key pairs that `argvus keys` manages. It is in **Applications → SSH keys** (search `ssh keys` or run `argvus-control-center ssh-keys`).

The page has four parts, from top to bottom:

```
Search        filters the key pairs by name or comment
Generate      name, type and the Generate key pair action
Keys          one block per key pair; a scrollbar appears when they do not fit
Actions       Refresh and Unload all keys from agent
```

#### How to use it

1. Open **Control Center → Applications → SSH keys**. From the Home, you can also search for `ssh keys`.
2. **Find a key:** press `/`, then type to show only the pairs whose name or comment matches. `Enter` ends the search and runs the highlighted row, as in the other lists, so check the row before pressing it; `Esc` clears the search.
3. **Create a pair:** in **Generate**, select **Name**, press `Enter`, type the name and press `Enter` again. Select **Type** and press `Enter` to switch between `ed25519` and `rsa`. Select **Generate key pair** and press `Enter`. The new pair appears under **Keys** at once, without a passphrase.
4. **Copy the public key:** in the pair's block, select **Copy public key** and press `Enter`. The contents of the `.pub` file go to the Wayland clipboard, ready to paste into a server's `authorized_keys` or a Git host. The private key is never copied. This needs `wl-clipboard` (`wl-copy`); without it, the page says the key was not copied.
5. **Load a pair into the agent:** in the pair's block, select **Load into agent** and press `Enter`. A key without a passphrase is loaded at once, with no window. A key with a passphrase opens an ARGVUS terminal, where OpenSSH asks for it; close the terminal with `Enter` when it says the key is loaded. The page checks the agent every few seconds while it is open, so the row changes to **Unload from agent** by itself. To update at once, use **Refresh** in **Actions**.
6. **Remove a key from the agent:** in the pair's block, select **Unload from agent**. To remove every key, use **Unload all keys from agent** in **Actions**. The key files stay on disk.
7. **Delete a pair:** in the pair's block, select **Delete** and press `Enter`. The confirmation focuses **Cancel**, so a stray `Enter` does nothing. Press `y` to confirm, or `n`/`Esc` to cancel. A key that is loaded in the agent cannot be deleted; unload it first.

Moving around: `↑`/`↓` or `j`/`k` move through the list, `Enter` activates a row, `Esc` goes back and `?` shows help. `Tab` moves the focus into **Actions**, where `←`/`→` choose a button and `Enter` runs it. `↑`/`↓` bring the focus back to the list.

Passphrases are never typed into the Control Center and never passed as command arguments. The page reads the list with `argvus keys list` and writes only through `argvus keys`. When `argvus-keys` is not installed, the page says so.

#### Troubleshooting

| Symptom | Cause and fix |
| --- | --- |
| `Could not open a connection to your authentication agent` after **Load into agent** | The terminal did not receive the agent socket. Hyprland keeps the environment it started with, and the agent may have started later. Update `argvus-launcher`, which falls back to `$XDG_RUNTIME_DIR/ssh-agent.socket`, and check `systemctl --user status argvus-ssh-agent.service`. |
| **Agent** shows `Agent unavailable` | The launcher cannot reach the agent. Check the service as above. |
| The page says `argvus-keys is not installed` | Install or update the `argvus-launcher` package. |
| **Delete** shows an error | The key is loaded in the agent, and it is kept. Unload it first, then delete it again. |

### GPG keys

GPG keys is the page for the keyring that `argvus gpg` manages. It is in **Applications → GPG keys** (search `gpg keys` or run `argvus-control-center gpg-keys`).

The page has these sections, from top to bottom:

```
Pinentry        the pinentry in use, and the choices: pinentry-qt, pinentry-gtk-2, pinentry-curses
Generate key    identity, algorithm, expiry, the passphrase toggle and the Generate key action
Import          the Import public key action
Keys            one block per key: identity, fingerprint, type, expiry, private key, Export public key
Refresh         reloads the keyring and the pinentry
```

#### How to use it

1. Open **Control Center → Applications → GPG keys**. From the Home, you can also search for `gpg keys`.
2. **Choose the pinentry:** in **Pinentry**, select a choice and press `Enter`. The current one is marked with `●`. The confirmation focuses **Cancel**; press `y` to change the pinentry, or `n`/`Esc` to keep the current one. The page writes only the pinentry line of `gpg-agent.conf` and restarts `gpg-agent`.
3. **Create a key:** in **Generate key**, select **Identity**, press `Enter`, type `Name <you@example.com>` and press `Enter`. **Algorithm** switches between `ed25519` and `rsa4096`. **Expires** takes `0` for no expiry or a number followed by `d`, `w`, `m` or `y` (for example `2y`). The **Passphrase** row is on by default, so the pinentry window asks for the passphrase; the status line says that the page is waiting for it. Select **Generate key** and press `Enter`. The new key gets a signing key and an encryption subkey, so it can sign commits and encrypt files.
4. **Create a key without a passphrase:** turn the **Passphrase** row off with `Enter`, then run **Generate key**. The confirmation focuses **Cancel**, because the private key is then stored unprotected. Press `y` only if that is what you want.
5. **Export a public key:** in the key's block, select **Export public key…** and press `Enter`. The path proposed is `~/argvus-<key id>.asc`; change it if you want, then press `Enter`. The file is created with mode `600`. An existing file is never replaced, and the page shows the error instead.
6. **Import a public key:** in **Import**, select **Import public key…**, type the path of the file and press `Enter`. Files that contain a private key are refused.
7. **Find a key:** press `/` and type to show only the keys whose identity or fingerprint matches. `Esc` clears the search.

Moving around: `↑`/`↓` or `j`/`k` move through the list, `Enter` activates a row, `Esc` goes back and `?` shows help.

Passphrases are never typed into the Control Center: gpg-agent opens the pinentry window and only that window receives the secret. Private keys are never exported from the page. The page reads the keyring with `argvus gpg list` and writes only through `argvus gpg`. When `argvus gpg` is not installed, the page says so.

#### Troubleshooting

| Symptom | Cause and fix |
| --- | --- |
| The page says `argvus gpg is not available` | The installed `argvus-launcher` predates the GPG command. Run `argvus gpg list` in a terminal: `unknown command` means the package must be rebuilt and installed. Keys also need `gnupg`. |
| **Generate key** waits and no window opens | The pinentry is not installed or not selected. Check **Pinentry**, and install the matching package (`pinentry` provides `pinentry-qt`, `pinentry-gtk-2` and `pinentry-curses`). |
| **Pinentry** shows `Not set` | gpg-agent uses its built-in default. Choose one of the three choices to set it. |
| A status line shows `argvus-gpg: ...` | The launcher refused the action, and the text gives the reason, such as an invalid expiry or a file that already exists. |
| The import is refused | The file contains a private key. Export only the public key on the other machine. |


### Locale & Region

This area contains time zone, date and time, regional locale, system locales and keyboard settings. Keyboard settings include layout, variant and console keymap. The home search also exposes keyboard shortcuts and mouse/touchpad settings as direct settings routes.

### Hardware and displays

When the corresponding capabilities are installed, the Control Center can show hardware information such as CPU, GPU, memory, power and devices. The input page configures mouse and touchpad behavior; the display area handles monitor resolution, refresh rate, scale, position, orientation, primary display, VRR and HDR where supported by the running session. Display changes apply immediately. After a change that can leave the screen unusable (resolution or refresh rate, position, mirroring, disabling a monitor, color depth) or after **Apply**, Control Center asks whether to keep it, with the focus on **Revert**: press `y` to keep it; `n`, `Esc` or `Enter` revert it, and without an answer within 15 seconds the previous configuration comes back automatically. Removing the saved configuration of a disconnected monitor and deleting a profile ask for confirmation; profiles open their own page with **Apply profile**, **Rename** and **Delete profile**.

### Connectivity and audio

The network area exposes the installed network provider's status and connection pages, including Wi-Fi, Ethernet, VPN, DNS, proxy and firewall routes when available. Bluetooth is conditional on Bluetooth support. The audio area is provided when its optional capability is enabled and integrates with the installed audio service.

### Power and session

Power provides the system power controls and policy pages available to the installation, including the Keep Awake toggle. When enabled, Keep Awake prevents the ARGVUS idle policy from starting automatic screen-lock and display-power timers. Session exposes ARGVUS session status, components, autostart, diagnostics and logs. These pages may require system permissions and should be distinguished from the quick actions in the Control Panel.

### System tools

The system section can expose boot information, packages, services, storage, diagnostics, user/group administration and system information. These are administrative or diagnostic tools; a page may be read-only or require authorization depending on the operation.

**Dev Services** reports three development-facing sources in one read-only page: the current user's systemd services, the TCP ports currently listening (`ss`), and the running containers from any installed container runtime (Podman, Docker). It only appears on the Home grid when at least one of these is available on the system — a machine with no systemd user session, no Podman and no Docker does not show the entry. `r` refreshes the three sources independently, so a slow or failing one (for example a container runtime that stopped responding) does not block the others from showing their data.

On the **Packages** lists that filter while typing (Search, AUR, Installed, Orphans and Updates), letters go to the filter; use the arrow keys to move and `/` to type a new search. `Enter` on a package opens its page, with **Install** (or **Reinstall**) and, in its **Danger zone**, **Remove**. On **Orphans**, `Space` marks packages (`[x]`) and **Remove marked** removes them together. **Upgrade all** and **Refresh database** are rows of the Updates page, the cache cleanups are in the Danger zone of the Cache page, and **Mirrors → Configure mirrors** sets the reflector options before **Generate preview**. Every package operation shows its plan and asks for confirmation (`y` confirms, `n` or `Esc` cancels) before it runs, and its output is shown while it runs.

### Users and groups

When the account provider is available, open **Settings → System → Users** or search for **users**. The interface can list normal and system accounts, create a user, edit account metadata, manage supplementary and primary groups, change or lock a password, unlock it, require a password change at the next login, set or remove an avatar, and enable automatic login.

User creation asks for a username, full name, shell, supplementary groups and an optional password confirmation. A blank password creates the account with its password locked; it does not silently create a usable password. A user page groups the account fields (full name, shell, primary and supplementary groups, followed by **Save changes**), the **Automatic login** toggle, the **Password** actions (change, lock, unlock, require a change at the next login) and the **Avatar** actions. Username, UID / GID and home directory are shown for information only. The **Danger zone** at the end holds the separate destructive actions to delete the account while keeping its home directory or to delete the home directory as well. Read the confirmation text carefully before choosing the latter. Account operations are privileged and may open the system authorization prompt. Avatars can be consumed by the greeter through the account's standard face image integration.

**Automatic login** starts the selected account's session at the next boot without showing the login screen, the same convention GNOME, LightDM and SDDM use for this feature: no password is stored anywhere. Enabling it for one account disables it for any previously configured one (only one account can auto-login at a time). It applies once, at the next boot; after a manual logout the normal login screen returns for the rest of that session. The toggle is applied immediately, with a confirmation prompt, and is only available to administrators.

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
- **Widget Telemetry** — enable the telemetry surface, select its available blocks and reorder them (`Shift+Up`/`Shift+Down` on a focused block).
- **Control Panel** — enable, disable and reorder panel cards (`Shift+Up`/`Shift+Down` on a focused card, applied immediately through the same primitive the real panel's drag-and-drop uses).

These actions update the logical ARGVUS state and apply the affected runtime configuration. See [Appearance](/docs/user-guide/appearance/), [Themes](/docs/argvus-themes/) and [Windows and layout](/docs/argvus-hyprland/windows-and-layout/).

## Keyboard use

The settings application supports keyboard navigation. Every page is a single list: use the arrow keys or `j`/`k` to move, `Enter` to open or run the selected row, `Space` to toggle, `Esc` to go back, `/` to search lists and `?` for help; the footer shows the keys that apply to the selected row. Actions are rows of the list, not buttons, and `Tab` does not move to them. Rows that only show information are skipped. While a text field, a search or a list filter is active, `q` and `?` are typed as text instead of quitting or opening the help. **Reset defaults** and **Restore all shortcuts** are rows in a **Danger zone** at the end of their pages; `r` asks for the same reset, and on the shortcuts list `r` restores the selected shortcut after a confirmation. On **Keyboard → Layout**, `/` focuses the **Search** box at the top of the page, and `Esc` clears it; `Enter` sets the default layout (marked with `●`) and `Space` adds or removes a layout (`[x]`); these keys only change a draft, and the **Apply** button in the **Actions** bar under the list writes the layouts and the default to Hyprland in one step. From the list, `Tab` or `←/→` moves the focus to that bar while **Apply** is enabled (the footer shows `←/→ Actions` then); `←/→` steps through its buttons and `↑/↓` return to the list. Leaving the page with unapplied changes asks for confirmation first. On **Locale & Region → Time Zone** the layout is the same: `/` focuses the **Search** box at the top, `Enter` picks a zone in a draft (marked with `●`), and the **Apply** button in the **Actions** bar under the list writes it to the system with `timedatectl`; `Tab`, `←/→` and `↑/↓` work as on Keyboard → Layout. Leaving with a zone that was not applied asks for confirmation first. On **System Locales**, `Enter` or `Space` marks a locale and the **Apply** row at the end writes the selection.

Pages that collect changes before saving them (a user, a group, account creation, password change, firewall configuration and System Locales) end that block with a **Save** or **Apply** row that stays dimmed until something changed. If going back would discard unsaved changes, Control Center asks first. Every confirmation shows **Confirm** and **Cancel** as two rows with the focus on **Cancel**: `Enter` runs the focused row, `y` confirms and `n` or `Esc` cancel.

In **Appearance**, every page is a single list: the footer shows only the keys that apply to the selected row, and `Space` also opens or selects the row. Pages that collect changes before applying them (Taskbar, Widget Telemetry, Control Panel and the effect sliders) end with an **Apply** row, which stays dimmed until something changed. On percentage rows, `←`/`→` (or `+`/`-`) change the value in steps of 5 and `Enter` lets you type it; use `Esc` to go back. If going back would discard changes that were not applied, Control Center asks first. Deleting a custom theme (`d`) or replacing an imported one asks for confirmation; the focus starts on **Cancel**, `y` confirms and `n` or `Esc` cancel.

## Persistence and reset

Settings pages apply changes through their owning provider and save the supported user state. Appearance changes are stored in the canonical configuration, and `argvus-config` is the only component that writes the derived consumer files under `~/.config/argvus/data/generated/`; those generated files are not the place to make a permanent edit. Input and keybinding pages have their own persisted state and reset actions. Other system changes may require permissions or a service reload.

There is no global reset for every ARGVUS setting. Restore a change from the page that owns it, or use the relevant feature's documented recovery procedure.

Reset support is deliberately per domain:

- **Appearance** provides a reset for the highlight color to the active theme default. It does not provide a single global appearance reset; restore other appearance values from their own controls.
- **Fonts** can restore all font settings, a target, or an individual setting.
- **Default applications** can restore all defaults, a category, or an individual selector.
- **Keyboard shortcuts** can restore one binding or all bindings and then reload the generated session bindings.
- **Window rules** is in **Hyprland → Settings → Window rules** (search `window rules` or run `argvus-control-center window-rules`). Each rule has a **Workspace** row (`Enter` or `←/→` steps through 1 to 10) and a **Window classes** row that opens a text field with comma-separated regular expressions. **Add rule** creates `rule-N` with workspace 1 and no classes. Each rule's **Remove rule** sits in the **Danger zone** and asks for confirmation. Every change is written to `hyprland.window_rules` in `argvus config` and reloads the configuration immediately; invalid class patterns are rejected and shown as an error.

- **Projects** is in **Applications → Projects** (search `projects` or run `argvus control-center projects`). **Add project folder** and **Add root folder** sit at the top of the page and open a text field with the path; the launcher checks that it is a directory and shows the error otherwise, including when the folder would push the effective project count past 9 (`SUPER + ALT + 1..9` cannot address more). Each configured folder, marked as a root (its subfolders are projects) or a single project, gets its own block below the form with a **Remove** row that asks for confirmation. The page calls `argvus-projects`, so changes are written by the launcher and apply the next time `SUPER + O` or `SUPER + ALT + 1..9` runs. If `argvus-projects` is not installed, the page says so.
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

Useful direct routes include `apps`, `window-rules`, `projects`, `snippets`, `ssh-keys`, `gpg-keys`, `fonts`, `locale`, `input`, `keybindings`, `language`, `config`, `system`, `hardware`, `services`, `dev-services`, `network`, `audio`, `bluetooth`, `boot`, `packages`, `storage`, `diagnostics`, `power`, `session`, `displays` and `appearance`. Appearance also accepts focused routes such as `themes`, `wallpapers`, `accents`, `effects`, `spaces`, `taskbar` and `widget-telemetry`; display accepts `resolution`, `refresh`, `scale`, `position`, `orientation`, `primary`, `vrr` and `hdr`. Use the installed `--help` output when scripting because availability still depends on the installed build.

## Related

- [First configuration](/docs/getting-started/)
- [Control Panel](/docs/argvus-control-panel/)
- [Appearance](/docs/user-guide/appearance/)
- [Keyboard shortcuts](/docs/argvus-hyprland/keyboard-shortcuts/)
- [Mouse and touchpad](/docs/argvus-hyprland/input/)
