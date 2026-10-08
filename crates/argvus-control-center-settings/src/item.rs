//! Stable identifiers of the Settings menu rows.
//!
//! Every selectable row carries an [`Item`] instead of its position, so the
//! action of a row never depends on how many rows precede it. Items of data
//! lists keep the index into their *unfiltered* source, which keeps them
//! valid while a search narrows the list.
use argvus_control_center_apps::catalog::Category;

use crate::config::fonts::{FontTarget, SettingKind};

/// Rows of the hardware-mouse (libratbag) block on Mouse & Touchpad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RatbagRow {
  Device,
  Profile,
  Dpi,
  ReportRate,
}

/// Identity of one Settings menu row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
  // Settings home.
  DefaultApps,
  Fonts,
  LocaleRegion,
  System,
  /// "Restore defaults" of the app, font and font-setting pages (D6).
  ResetDefaults,

  // Default applications.
  Category(Category),
  /// Index into the installed applications of the open category.
  App(usize),

  // Fonts.
  FontTarget(FontTarget),
  FontSetting(SettingKind),
  FontSize,
  /// Index into the system font list.
  Font(usize),
  /// Index into the options of the open rendering setting.
  SettingOption(usize),

  // Locale and region.
  TimeZone,
  DateTime,
  RegionalLocale,
  SystemLocales,
  Keyboard,
  /// Index into the time zone list.
  Zone(usize),
  LocalDateTime,
  Ntp,
  /// Index into the generated locales.
  Locale(usize),
  /// Index into the `/etc/locale.gen` entries.
  LocaleGen(usize),
  ApplyLocales,
  KeyboardLayout,
  KeyboardVariant,
  ConsoleKeymap,
  /// Index into the XKB layouts.
  Layout(usize),
  /// Index into the variants of the current layout.
  Variant(usize),
  /// Index into the console keymaps.
  Keymap(usize),
  /// 0 = English (US), 1 = Portuguese (Brazil).
  Language(usize),

  // System.
  Hostname,
  Users,
  Groups,
  DoNotDisturb,

  // Mouse and touchpad. The index is the one understood by
  // `InputSettings::toggle` / `InputSettings::cycle_by`.
  Input(usize),
  Ratbag(RatbagRow),

  // Keyboard shortcuts.
  /// Index into the loaded window rules. Every per-rule row edits a draft
  /// until `ApplyWindowRule` saves it.
  WindowRuleName(usize),
  WindowRuleWorkspace(usize),
  WindowRuleClasses(usize),
  ApplyWindowRule(usize),
  RemoveWindowRule(usize),
  /// The name typed for the next rule, used by `AddWindowRule`.
  NewWindowRuleName,
  AddWindowRule,
  /// Opens the editor for a new project path, or a new root when true.
  AddProjectPath,
  AddProjectRoot,
  /// Index into the loaded project entries.
  RemoveProject(usize),
  /// Index into the loaded keybindings.
  Keybinding(usize),
  RestoreAllShortcuts,
  ChangeShortcut,
  DisableShortcut,
  RestoreShortcut,
  /// Apply the detected shortcut, or capture again when none was detected.
  ApplyCapture,
  CancelCapture,

  // Accounts.
  CreateUser,
  UserList,
  SystemUsers,
  /// Index into the listed users.
  UserEntry(usize),
  Username,
  FullName,
  Shell,
  PrimaryGroup,
  SupplementaryGroups,
  /// 0 = current, 1 = new, 2 = confirmation.
  Password(usize),
  CreateAccount,
  /// Create form: give the new account administration rights (`sudo` group).
  CreateAdmin,
  /// Username field of its own page on the create form (Enter opens the editor).
  UsernameField,
  /// Cancel of the create form: discards it and goes back to the list.
  CancelCreate,
  /// User page: administration rights of the account (`sudo` membership), applied by Save.
  UserAdmin,
  SaveUser,
  ChangePassword,
  LockPassword,
  UnlockPassword,
  ExpirePassword,
  /// User page: starts this account automatically at the next boot, skipping
  /// the login screen (greetd's `initial_session`). Applied immediately.
  ToggleAutoLogin,
  AvatarImage,
  EditAvatar,
  RemoveAvatar,
  /// Full name field of its own page (Enter opens the editor).
  FullNameField,
  /// Ok of the password page: returns to the user page, the change waits for Save.
  OkPassword,
  /// Cancel of the user page: discards the draft and goes back.
  CancelUser,
  /// Delete of the user page: opens the Keep home / Delete home choice.
  DeleteUserMenu,
  DeleteUser,
  DeleteUserAndHome,
  /// Index into the available shells.
  ShellOption(usize),
  /// Index into the system groups (primary group picker).
  PrimaryGroupOption(usize),
  /// Index into the system groups (supplementary groups picker).
  GroupOption(usize),

  // Groups.
  CreateGroup,
  GroupList,
  /// Index into the listed groups.
  GroupEntry(usize),
  GroupName,
  Members,
  /// Index into all users (members picker).
  Member(usize),
  SubmitGroup,
  SaveGroup,
  DeleteGroup,

  // Firewall.
  LoadFirewall,
  FirewallService,
  FirewallBoot,
  /// Index into the firewall configuration fields.
  FirewallField(usize),
  SaveFirewall,
  DiscardFirewall,
  AddRules,
  ApplyRules,
}
