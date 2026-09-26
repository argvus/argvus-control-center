//! Portable user-owned theme profiles.
//!
//! Official themes remain resolved by `argvus-theme` and applied by the
//! existing appearance scripts. A custom theme is only a validated snapshot
//! plus a registry entry; it is not a second theme engine.

use crate::model::{CustomTheme, THEMES, WidgetTelemetryBlock, canonical_theme_id};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use tar::{Archive, Builder, EntryType, Header};
use tempfile::{TempDir, tempdir};
use time::{OffsetDateTime, format_description};

const ROOT: &str = "argvus-theme-profile/";
const MANIFEST_PATH: &str = "argvus-theme-profile/manifest.json";
const SUMS_PATH: &str = "argvus-theme-profile/SHA256SUMS";
const MAX_MEMBERS: usize = 128;
const MAX_MANIFEST: u64 = 1024 * 1024;
const MAX_STATE_FILE: u64 = 8 * 1024 * 1024;
const MAX_WALLPAPER: u64 = 512 * 1024 * 1024;
const MAX_TOTAL: u64 = 520 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum FileId {
  ActiveTheme,
  Accent,
  GtkMode,
  Spaces,
  Borders,
  WallpaperCustom,
  Fonts,
  /// Legacy profile identifier kept for importing format v1 archives.
  Effects,
  Animations,
  Transparency,
  ThemeEffects,
  TaskbarRight2Mode,
  WidgetTelemetryBlocks,
  ControlPanelCards,
  CanonicalConfig,
}

impl FileId {
  const ALL: [Self; 14] = [
    Self::ActiveTheme,
    Self::Accent,
    Self::GtkMode,
    Self::Spaces,
    Self::Borders,
    Self::WallpaperCustom,
    Self::Fonts,
    Self::Animations,
    Self::Transparency,
    Self::ThemeEffects,
    Self::TaskbarRight2Mode,
    Self::WidgetTelemetryBlocks,
    Self::ControlPanelCards,
    Self::CanonicalConfig,
  ];
  fn id(self) -> &'static str {
    match self {
      Self::ActiveTheme => "active-theme",
      Self::Accent => "accent",
      Self::GtkMode => "gtk-mode",
      Self::Spaces => "spaces",
      Self::Borders => "borders",
      Self::WallpaperCustom => "wallpaper-custom",
      Self::Fonts => "fonts",
      Self::Effects => "effects",
      Self::Animations => "animations",
      Self::Transparency => "transparency",
      Self::ThemeEffects => "theme-effects",
      Self::TaskbarRight2Mode => "taskbar-right-2-mode",
      Self::WidgetTelemetryBlocks => "widget-telemetry-blocks",
      Self::ControlPanelCards => "control-panel-cards",
      Self::CanonicalConfig => "canonical-config",
    }
  }
  fn archive_path(self) -> &'static str {
    match self {
      Self::ActiveTheme => "payload/config/argvus/.active-theme",
      Self::Accent => "payload/config/argvus/.accent-color",
      Self::GtkMode => "payload/config/argvus/.gtk-mode",
      Self::Spaces => "payload/config/argvus/.spaces",
      Self::Borders => "payload/config/argvus/.borders",
      Self::WallpaperCustom => "payload/config/argvus/.wallpaper-custom",
      Self::Fonts => "payload/config/argvus/fonts.conf",
      Self::Effects => "payload/config/argvus/state/effects",
      Self::Animations => "payload/config/argvus/state/animations",
      Self::Transparency => "payload/config/argvus/state/transparency",
      Self::ThemeEffects => "payload/config/argvus/state/effects/current-theme.conf",
      Self::TaskbarRight2Mode => "payload/config/argvus/state/taskbar-right-2-mode",
      Self::WidgetTelemetryBlocks => "payload/config/argvus/state/widget-telemetry-blocks",
      Self::ControlPanelCards => "payload/config/argvus/control-panel/cards.json",
      Self::CanonicalConfig => "payload/config/argvus/config.json",
    }
  }
  fn destination(self, root: &Path) -> PathBuf {
    if self == Self::ThemeEffects {
      return theme_effects_destination(
        root,
        &read_first(&root.join(".active-theme"), "argvus-dark"),
      );
    }
    root.join(match self {
      Self::ActiveTheme => ".active-theme",
      Self::Accent => ".accent-color",
      Self::GtkMode => ".gtk-mode",
      Self::Spaces => ".spaces",
      Self::Borders => ".borders",
      Self::WallpaperCustom => ".wallpaper-custom",
      Self::Fonts => "fonts.conf",
      Self::Effects => "state/effects",
      Self::Animations => "state/animations",
      Self::Transparency => "state/transparency",
      Self::ThemeEffects => unreachable!(),
      Self::TaskbarRight2Mode => "state/taskbar-right-2-mode",
      Self::WidgetTelemetryBlocks => "state/widget-telemetry-blocks",
      Self::ControlPanelCards => "control-panel/cards.json",
      Self::CanonicalConfig => "config.json",
    })
  }
  fn parse(value: &str) -> Option<Self> {
    if value == Self::Effects.id() {
      Some(Self::Effects)
    } else {
      Self::ALL.into_iter().find(|id| id.id() == value)
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Manifest {
  format: String,
  #[serde(default)]
  format_version: Option<u32>,
  #[serde(default)]
  schema_version: Option<u32>,
  name: String,
  created_at: String,
  argvus: ManifestArgvus,
  files: Vec<ManifestFile>,
  #[serde(default)]
  wallpaper: Option<WallpaperMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManifestArgvus {
  theme: String,
  mode: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManifestFile {
  id: String,
  source_path: String,
  archive_path: String,
  sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WallpaperMeta {
  original_path: String,
  original_home: String,
  filename: String,
  archive_path: String,
  sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Registry {
  version: u32,
  themes: Vec<RegistryTheme>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegistryTheme {
  id: String,
  name: String,
  base_theme: String,
  profile_path: String,
  #[serde(default)]
  wallpaper_path: Option<String>,
}

impl From<RegistryTheme> for CustomTheme {
  fn from(value: RegistryTheme) -> Self {
    Self {
      id: value.id,
      name: value.name,
      base_theme: value.base_theme,
      profile_path: value.profile_path,
      wallpaper_path: value.wallpaper_path,
    }
  }
}

#[derive(Debug)]
struct StagedEntry {
  path: PathBuf,
  sha256: String,
}
#[derive(Debug)]
struct StagedProfile {
  _temp: TempDir,
  manifest: Manifest,
  entries: HashMap<String, StagedEntry>,
}

pub fn custom_themes() -> Vec<CustomTheme> {
  let Ok(text) = fs::read_to_string(registry_path()) else {
    return Vec::new();
  };
  let Ok(registry) = serde_json::from_str::<Registry>(&text) else {
    return Vec::new();
  };
  if registry.version != 1 {
    return Vec::new();
  }
  registry
    .themes
    .into_iter()
    .filter(|theme| {
      let expected_profile = profile_root()
        .join(&theme.id)
        .join("profile.tar.gz")
        .display()
        .to_string();
      !theme.id.is_empty()
        && !theme.name.trim().is_empty()
        && theme.profile_path == expected_profile
        && Path::new(&theme.profile_path).is_file()
        && THEMES.iter().any(|(name, _)| *name == theme.base_theme)
    })
    .map(Into::into)
    .collect()
}

pub fn active_custom_theme() -> Option<String> {
  fs::read_to_string(custom_current_path())
    .ok()
    .map(|value| value.trim().to_owned())
    .filter(|id| custom_themes().iter().any(|theme| &theme.id == id))
}

pub fn list_import_archives() -> Vec<PathBuf> {
  let mut files = fs::read_dir(argvus_control_center_core::paths::home())
    .ok()
    .into_iter()
    .flatten()
    .filter_map(Result::ok)
    .map(|entry| entry.path())
    .filter(|path| {
      path.is_file()
        && path
          .file_name()
          .and_then(|name| name.to_str())
          .is_some_and(|name| name.ends_with(".tar.gz"))
    })
    .collect::<Vec<_>>();
  files.sort();
  files
}

pub fn export_named(name: &str) -> Result<PathBuf, String> {
  let display_name = clean_display_name(name)?;
  let slug = filename_slug(&display_name);
  let timestamp = local_timestamp()?;
  let home = argvus_control_center_core::paths::home();
  let mut output = home.join(format!("{slug}-{timestamp}.tar.gz"));
  let mut suffix = 2;
  while output.exists() {
    output = home.join(format!("{slug}-{suffix}-{timestamp}.tar.gz"));
    suffix += 1;
  }
  let root = argvus_root();
  let canonical_export = tempdir().map_err(|error| error.to_string())?;
  let canonical_config_path = canonical_export.path().join("config.json");
  let has_canonical_config = export_canonical_appearance(&canonical_config_path)?;
  let canonical_theme = if has_canonical_config {
    fs::read_to_string(&canonical_config_path)
      .ok()
      .and_then(|contents| serde_json::from_str::<serde_json::Value>(&contents).ok())
      .and_then(|document| {
        document
          .pointer("/appearance/theme")
          .and_then(|value| value.as_str())
          .map(str::to_owned)
      })
  } else {
    None
  };
  let active_theme =
    canonical_theme.unwrap_or_else(|| read_first(&root.join(".active-theme"), "argvus-dark"));
  let effects_path = theme_effects_destination(&root, &active_theme);
  if !effects_path.is_file() {
    write_atomic(
      &effects_path,
      "taskbar.transparency=50\ncontrol-panel.transparency=50\nwidget-telemetry.transparency=50\nterminal.transparency=50\ntaskbar.transparency.enabled=enabled\ncontrol-panel.transparency.enabled=enabled\nwidget-telemetry.transparency.enabled=enabled\nterminal.transparency.enabled=enabled\ntaskbar.blur=50\ncontrol-panel.blur=50\nwidget-telemetry.blur=50\nblur.global=50\ntaskbar.blur.enabled=enabled\ncontrol-panel.blur.enabled=enabled\nwidget-telemetry.blur.enabled=enabled\nterminal.blur.enabled=enabled\n",
    )?;
  }
  let mut files = Vec::new();
  let mut contents = Vec::new();
  let mut profile_sources = source_paths(&root)
    .into_iter()
    .filter(|(id, _)| *id != FileId::CanonicalConfig)
    .collect::<Vec<_>>();
  if has_canonical_config {
    profile_sources.push((FileId::CanonicalConfig, canonical_config_path));
  }
  for (id, source) in profile_sources {
    if !source.is_file() {
      continue;
    }
    let bytes = fs::read(&source).map_err(|error| format!("{}: {error}", id.id()))?;
    if bytes.len() as u64 > MAX_STATE_FILE {
      return Err(format!("{} is too large", id.id()));
    }
    files.push(ManifestFile {
      id: id.id().into(),
      source_path: source.display().to_string(),
      archive_path: format!("{ROOT}{}", id.archive_path()),
      sha256: sha256(&bytes),
    });
    contents.push((id.archive_path().to_owned(), bytes));
  }
  let wallpaper = current_wallpaper().map(|path| wallpaper_meta(&path));
  let manifest = Manifest {
    format: "argvus-theme-profile".into(),
    format_version: Some(if has_canonical_config { 3 } else { 2 }),
    schema_version: None,
    name: display_name,
    created_at: OffsetDateTime::now_utc().to_string(),
    argvus: ManifestArgvus {
      theme: canonical_theme_id(&active_theme),
      mode: "sticky".into(),
    },
    files: files.clone(),
    wallpaper: wallpaper.clone(),
  };
  let bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
  let sums = files
    .iter()
    .map(|file| format!("{}  {}\n", file.sha256, file.archive_path))
    .collect::<String>();
  write_archive_atomic(&output, &bytes, &sums, &contents, wallpaper.as_ref())?;
  Ok(output)
}

pub fn preview_export_path(name: &str) -> Option<PathBuf> {
  let display_name = clean_display_name(name).ok()?;
  let timestamp = local_timestamp().ok()?;
  Some(argvus_control_center_core::paths::home().join(format!(
    "{}-{timestamp}.tar.gz",
    filename_slug(&display_name)
  )))
}

pub fn import_archive(path: &Path) -> Result<CustomTheme, String> {
  let staged = stage_archive(path)?;
  install_staged_profile(&staged, path)
}

pub fn inspect_archive(path: &Path) -> Result<(String, bool), String> {
  let staged = stage_archive(path)?;
  let name = clean_display_name(&staged.manifest.name)?;
  let slug = filename_slug(&name);
  let duplicate = custom_themes()
    .iter()
    .any(|theme| theme.id == slug || theme.name.eq_ignore_ascii_case(&name));
  Ok((name, duplicate))
}

pub fn apply_custom_theme(theme: &CustomTheme) -> Result<ApplyReport, String> {
  let staged = stage_archive(Path::new(&theme.profile_path))?;
  apply_staged(&staged, theme.wallpaper_path.as_deref(), &theme.id)
}

pub fn delete_custom_theme(theme: &CustomTheme) -> Result<(), String> {
  if active_custom_theme().as_deref() == Some(theme.id.as_str()) {
    crate::backend::set_theme("argvus-dark")?;
  }
  let mut themes = read_registry();
  themes.retain(|candidate| candidate.id != theme.id);
  save_registry(&themes)?;
  if let Some(parent) = Path::new(&theme.profile_path).parent() {
    fs::remove_dir_all(parent)
      .map_err(|error| format!("could not remove theme profile: {error}"))?;
  }
  Ok(())
}

fn install_staged_profile(staged: &StagedProfile, source: &Path) -> Result<CustomTheme, String> {
  let display_name = clean_display_name(&staged.manifest.name)?;
  let themes = read_registry();
  let slug = filename_slug(&display_name);
  let id = themes
    .iter()
    .find(|theme| theme.id == slug || theme.name.eq_ignore_ascii_case(&display_name))
    .map(|theme| theme.id.clone())
    .unwrap_or_else(|| unique_id(&slug, &themes));
  let root = profile_root();
  fs::create_dir_all(&root).map_err(|e| e.to_string())?;
  let storage = root.join(&id);
  let temporary = root.join(format!(".{id}.tmp-{}", process_id()));
  let _ = fs::remove_dir_all(&temporary);
  fs::create_dir_all(&temporary).map_err(|e| e.to_string())?;
  let installed = temporary.join("profile.tar.gz");
  copy_atomic(source, &installed)?;
  let restored_wallpaper = staged
    .manifest
    .wallpaper
    .as_ref()
    .map(|meta| {
      let entry = staged
        .entries
        .get(&meta.archive_path)
        .ok_or("missing wallpaper payload")?;
      restore_wallpaper(meta, &entry.path)
    })
    .transpose()?;
  let wallpaper_path = restored_wallpaper
    .as_ref()
    .map(|(path, _created)| path.clone());
  let wallpaper_created = restored_wallpaper
    .as_ref()
    .is_some_and(|(_, created)| *created);
  let record = RegistryTheme {
    id: id.clone(),
    name: display_name,
    base_theme: staged.manifest.argvus.theme.clone(),
    profile_path: root.join(&id).join("profile.tar.gz").display().to_string(),
    wallpaper_path: wallpaper_path.clone(),
  };
  let mut next = themes
    .into_iter()
    .filter(|theme| theme.id != id && !theme.name.eq_ignore_ascii_case(&record.name))
    .collect::<Vec<_>>();
  next.push(record.clone());
  if storage.exists() {
    let backup = root.join(format!(".{id}.backup-{}", process_id()));
    let _ = fs::remove_dir_all(&backup);
    fs::rename(&storage, &backup).map_err(|e| e.to_string())?;
    if let Err(error) = fs::rename(&temporary, &storage) {
      let _ = fs::rename(&backup, &storage);
      return Err(error.to_string());
    }
    if let Err(error) = save_registry(&next) {
      let _ = fs::remove_dir_all(&storage);
      let _ = fs::rename(&backup, &storage);
      if wallpaper_created && let Some(path) = &wallpaper_path {
        let _ = fs::remove_file(path);
      }
      return Err(error);
    }
    let _ = fs::remove_dir_all(backup);
  } else {
    fs::rename(&temporary, &storage).map_err(|e| e.to_string())?;
    if let Err(error) = save_registry(&next) {
      let _ = fs::remove_dir_all(&storage);
      if wallpaper_created && let Some(path) = &wallpaper_path {
        let _ = fs::remove_file(path);
      }
      return Err(error);
    }
  }
  Ok(record.into())
}

fn apply_staged(
  staged: &StagedProfile,
  wallpaper: Option<&str>,
  custom_theme_id: &str,
) -> Result<ApplyReport, String> {
  let root = argvus_root();
  let backup = tempdir().map_err(|e| e.to_string())?;
  let mut backups = Vec::new();
  let mut payload = Vec::new();
  let previous_theme = read_first(&root.join(".active-theme"), "argvus-dark");
  let previous_wallpaper = fs::read_to_string(root.join(".wallpaper-custom"))
    .ok()
    .map(|value| value.trim().to_owned())
    .filter(|value| !value.is_empty());
  let previous_custom_marker = fs::read(custom_current_path()).ok();
  for id in FileId::ALL {
    let destination = id.destination(&root);
    let old = if destination.is_file() {
      let old = backup.path().join(id.id());
      fs::copy(&destination, &old).map_err(|e| e.to_string())?;
      Some(old)
    } else {
      None
    };
    backups.push((destination, old));
  }
  let imported_effects = theme_effects_destination(&root, &staged.manifest.argvus.theme);
  if !backups.iter().any(|(path, _)| *path == imported_effects) {
    let old = if imported_effects.is_file() {
      let old_path = backup.path().join("imported-theme-effects");
      fs::copy(&imported_effects, &old_path).map_err(|e| e.to_string())?;
      Some(old_path)
    } else {
      None
    };
    backups.push((imported_effects.clone(), old));
  }
  for file in &staged.manifest.files {
    let id = FileId::parse(&file.id).ok_or("unknown theme profile file id")?;
    let entry = staged
      .entries
      .get(&file.archive_path)
      .ok_or("missing theme profile payload")?;
    if id == FileId::Effects {
      // Format v1 stored one combined setting. Preserve that profile's
      // meaning by initializing both independent settings.
      payload.push((entry.path.clone(), FileId::Animations.destination(&root)));
      payload.push((entry.path.clone(), FileId::Transparency.destination(&root)));
    } else if id == FileId::ThemeEffects {
      payload.push((entry.path.clone(), imported_effects.clone()));
    } else if id == FileId::CanonicalConfig {
      // The canonical payload is imported with argvus-config below. It must
      // not replace the complete desktop document with an appearance scope.
    } else {
      payload.push((entry.path.clone(), id.destination(&root)));
    }
  }
  let marker_path = custom_current_path();
  let result: Result<bool, String> = (|| {
    if let Some(entry) = staged.entries.get(
      &staged
        .manifest
        .files
        .iter()
        .find(|file| file.id == FileId::CanonicalConfig.id())
        .map(|file| file.archive_path.clone())
        .unwrap_or_default(),
    ) {
      import_canonical_appearance(&entry.path)?;
      project_canonical_config()?;
    }
    for (source, destination) in &payload {
      if *destination == FileId::Fonts.destination(&root) {
        copy_atomic(source, destination)?;
      }
    }
    crate::backend::set_theme_static(&staged.manifest.argvus.theme)?;
    for (source, destination) in &payload {
      copy_atomic(source, destination)?;
    }
    let wallpaper_missing = apply_overrides_static(wallpaper)?;
    write_atomic(&marker_path, &format!("{custom_theme_id}\n"))?;
    Ok(wallpaper_missing)
  })();
  let wallpaper_missing = match result {
    Ok(value) => value,
    Err(error) => {
      let recovery = rollback_staged(
        &backups,
        &marker_path,
        previous_custom_marker.as_deref(),
        &previous_theme,
        previous_wallpaper.as_deref(),
      );
      if let Err(recovery) = recovery {
        let retained = backup.keep();
        return Err(format!(
          "{error}; recovery failed: {recovery}; backup: {}",
          retained.display()
        ));
      }
      return Err(format!("profile apply rolled back: {error}"));
    }
  };
  if let Err(error) = crate::backend::reload_session_for_profile() {
    let recovery = rollback_staged(
      &backups,
      &marker_path,
      previous_custom_marker.as_deref(),
      &previous_theme,
      previous_wallpaper.as_deref(),
    );
    if let Err(recovery) = recovery.and_then(|()| crate::backend::reload_session_for_profile()) {
      let retained = backup.keep();
      return Err(format!(
        "{error}; recovery failed: {recovery}; backup: {}",
        retained.display()
      ));
    }
    return Err(format!("profile reload rolled back: {error}"));
  }
  if wallpaper_missing {
    return Ok(ApplyReport {
      wallpaper_missing: true,
    });
  }
  Ok(ApplyReport {
    wallpaper_missing: false,
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplyReport {
  pub wallpaper_missing: bool,
}

fn rollback_staged(
  backups: &[(PathBuf, Option<PathBuf>)],
  marker_path: &Path,
  previous_marker: Option<&[u8]>,
  previous_theme: &str,
  previous_wallpaper: Option<&str>,
) -> Result<(), String> {
  // Fonts must precede theme generation; reset-prone overrides follow it.
  if let Some((destination, old)) = backups
    .iter()
    .find(|(path, _)| *path == FileId::Fonts.destination(&argvus_root()))
  {
    restore_backup(destination, old.as_deref())?;
  }
  crate::backend::set_theme_static(previous_theme)?;
  for (destination, old) in backups {
    restore_backup(destination, old.as_deref())?;
  }
  apply_overrides_static(previous_wallpaper)?;
  match previous_marker {
    Some(value) => {
      write_atomic(
        marker_path,
        std::str::from_utf8(value).map_err(|e| e.to_string())?,
      )?;
    }
    None => {
      restore_backup(marker_path, None)?;
    }
  }
  Ok(())
}

fn restore_backup(destination: &Path, old: Option<&Path>) -> Result<(), String> {
  if let Some(old) = old {
    copy_atomic(old, destination)
  } else {
    match fs::remove_file(destination) {
      Ok(()) => Ok(()),
      Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
      Err(error) => Err(error.to_string()),
    }
  }
}

fn apply_overrides_static(wallpaper: Option<&str>) -> Result<bool, String> {
  const STATIC_ENV: [(&str, &str); 1] = [("ARGVUS_NO_RUNTIME", "1")];
  crate::backend::run_profile_script_with_env(
    "accent-switch.sh",
    &["--apply-static"],
    &STATIC_ENV,
  )?;
  crate::backend::run_profile_script_with_env("effects-toggle.sh", &["apply"], &STATIC_ENV)?;
  crate::backend::run_profile_script_with_env("spaces-switch.sh", &["--apply"], &STATIC_ENV)?;
  crate::backend::run_profile_script_with_env("borders-switch.sh", &["--apply"], &STATIC_ENV)?;
  crate::backend::run_profile_script_with_env("taskbar-right-2-mode.sh", &["apply"], &STATIC_ENV)?;
  crate::backend::run_profile_script_with_env(
    "argvus-widget-telemetry-toggle",
    &["blocks", "apply"],
    &STATIC_ENV,
  )?;
  if let Some(path) = wallpaper {
    if Path::new(path).is_file() {
      crate::backend::run_profile_script_with_env(
        "hypr-wallpaper-pick.sh",
        &["--apply-static", path],
        &STATIC_ENV,
      )?;
      return Ok(false);
    }
    let _ = fs::remove_file(argvus_root().join(".wallpaper-custom"));
    return Ok(true);
  } else {
    let _ = fs::remove_file(argvus_root().join(".wallpaper-custom"));
  }
  Ok(false)
}

fn stage_archive(path: &Path) -> Result<StagedProfile, String> {
  if !path.is_file() || !path.to_string_lossy().ends_with(".tar.gz") {
    return Err("invalid theme profile archive".into());
  }
  let temporary = tempdir().map_err(|e| e.to_string())?;
  let entries_dir = temporary.path().join("entries");
  fs::create_dir(&entries_dir).map_err(|e| e.to_string())?;
  let decoder = GzDecoder::new(File::open(path).map_err(|e| e.to_string())?);
  let mut archive = Archive::new(decoder);
  let mut entries = HashMap::new();
  let mut total = 0u64;
  for (index, item) in archive.entries().map_err(|e| e.to_string())?.enumerate() {
    if index >= MAX_MEMBERS {
      return Err("theme profile has too many entries".into());
    }
    let mut entry = item.map_err(|e| e.to_string())?;
    let name = entry
      .path()
      .map_err(|e| e.to_string())?
      .to_string_lossy()
      .to_string();
    if name.starts_with('/')
      || name.split('/').any(|part| part == ".." || part.is_empty())
      || !name.starts_with(ROOT)
      || entries.contains_key(&name)
    {
      return Err("unsafe or duplicate theme profile entry".into());
    }
    let kind = entry.header().entry_type();
    if kind != EntryType::Regular && kind != EntryType::Continuous {
      return Err("theme profile contains a non-regular entry".into());
    }
    let size = entry.size();
    let limit = if name == MANIFEST_PATH {
      MAX_MANIFEST
    } else if name.starts_with("argvus-theme-profile/wallpaper/") {
      MAX_WALLPAPER
    } else {
      MAX_STATE_FILE
    };
    total = total
      .checked_add(size)
      .ok_or("theme profile size overflow")?;
    if size > limit || total > MAX_TOTAL {
      return Err("theme profile exceeds safety limits".into());
    }
    let target = entries_dir.join(format!("entry-{index}"));
    let (_, checksum) = copy_reader_hash(&mut entry, &target)?;
    entries.insert(
      name,
      StagedEntry {
        path: target,
        sha256: checksum,
      },
    );
  }
  let manifest_entry = entries
    .get(MANIFEST_PATH)
    .ok_or("theme profile manifest is missing")?;
  let mut manifest: Manifest =
    serde_json::from_slice(&fs::read(&manifest_entry.path).map_err(|e| e.to_string())?)
      .map_err(|e| format!("invalid theme profile manifest: {e}"))?;
  manifest.argvus.theme = canonical_theme_id(&manifest.argvus.theme);
  if manifest.format != "argvus-theme-profile"
    || !is_supported_profile_version(manifest.format_version.or(manifest.schema_version))
  {
    return Err("unsupported theme profile version".into());
  }
  validate_manifest(&manifest, &entries)?;
  Ok(StagedProfile {
    _temp: temporary,
    manifest,
    entries,
  })
}

fn is_supported_profile_version(version: Option<u32>) -> bool {
  matches!(version, Some(1..=3))
}

fn validate_manifest(
  manifest: &Manifest,
  entries: &HashMap<String, StagedEntry>,
) -> Result<(), String> {
  if manifest.name.trim().is_empty()
    || manifest.name.chars().count() > 120
    || !THEMES
      .iter()
      .any(|(name, _)| *name == manifest.argvus.theme)
  {
    return Err("invalid theme profile metadata".into());
  }
  let mut expected = HashSet::from([MANIFEST_PATH.to_owned(), SUMS_PATH.to_owned()]);
  let mut seen = HashSet::new();
  for file in &manifest.files {
    let id = FileId::parse(&file.id).ok_or("unknown theme profile file id")?;
    if !seen.insert(id)
      || file.archive_path != format!("{ROOT}{}", id.archive_path())
      || !Path::new(&file.source_path).is_absolute()
    {
      return Err("invalid theme profile file mapping".into());
    }
    let entry = entries
      .get(&file.archive_path)
      .ok_or("theme profile payload is missing")?;
    if entry.sha256 != file.sha256 {
      return Err(format!("theme profile checksum mismatch: {}", file.id));
    }
    validate_content(id, &fs::read(&entry.path).map_err(|e| e.to_string())?)?;
    expected.insert(file.archive_path.clone());
  }
  if !seen.contains(&FileId::ActiveTheme) {
    return Err("theme profile active theme is missing".into());
  }
  let active_theme = manifest
    .files
    .iter()
    .find(|file| file.id == FileId::ActiveTheme.id())
    .and_then(|file| entries.get(&file.archive_path))
    .ok_or("theme profile active theme is missing")?;
  if canonical_theme_id(&read_first(&active_theme.path, "")) != manifest.argvus.theme {
    return Err("theme profile base theme does not match active theme".into());
  }
  if let Some(wallpaper) = &manifest.wallpaper {
    if wallpaper.original_path.is_empty()
      || wallpaper.original_home.is_empty()
      || !Path::new(&wallpaper.original_path).is_absolute()
      || !Path::new(&wallpaper.original_home).is_absolute()
      || wallpaper.filename.is_empty()
      || !is_safe_filename(&wallpaper.filename)
      || wallpaper.archive_path != format!("{ROOT}wallpaper/{}", wallpaper.filename)
      || !entries.contains_key(&wallpaper.archive_path)
      || entries[&wallpaper.archive_path].sha256 != wallpaper.sha256
    {
      return Err("theme profile wallpaper is invalid or missing".into());
    }
    expected.insert(wallpaper.archive_path.clone());
  }
  let sums = entries
    .get(SUMS_PATH)
    .ok_or("theme profile checksums are missing")?;
  let rendered = manifest
    .files
    .iter()
    .map(|file| format!("{}  {}\n", file.sha256, file.archive_path))
    .collect::<String>();
  if fs::read(&sums.path).map_err(|e| e.to_string())? != rendered.as_bytes() {
    return Err("theme profile checksum index mismatch".into());
  }
  if entries.keys().collect::<HashSet<_>>() != expected.iter().collect::<HashSet<_>>() {
    return Err("theme profile contains unexpected entries".into());
  }
  Ok(())
}

fn validate_content(id: FileId, data: &[u8]) -> Result<(), String> {
  let text = std::str::from_utf8(data).map_err(|_| format!("invalid UTF-8: {}", id.id()))?;
  let value = text.trim();
  match id {
    FileId::ActiveTheme if !THEMES.iter().any(|(name, _)| *name == value) => {
      return Err("unknown active theme".into());
    }
    FileId::Accent
      if value.len() != 7
        || !value.starts_with('#')
        || !value[1..].bytes().all(|b| b.is_ascii_hexdigit()) =>
    {
      return Err("invalid accent".into());
    }
    FileId::GtkMode if !matches!(value, "light" | "dark" | "auto" | "sticky") => {
      return Err("invalid GTK mode".into());
    }
    FileId::Effects | FileId::Animations | FileId::Transparency
      if !matches!(value, "enabled" | "disabled") =>
    {
      return Err("invalid effects state".into());
    }
    FileId::ThemeEffects => {
      let mut seen = HashSet::new();
      for line in text.lines() {
        let (key, value) = line.split_once('=').ok_or("invalid theme effects")?;
        let valid_value = if key.ends_with(".enabled") {
          matches!(value, "enabled" | "disabled")
        } else {
          value.parse::<u8>().ok().is_some_and(|number| number <= 100)
        };
        if !matches!(
          key,
          "taskbar.transparency"
            | "control-panel.transparency"
            | "widget-telemetry.transparency"
            | "terminal.transparency"
            | "taskbar.transparency.enabled"
            | "control-panel.transparency.enabled"
            | "widget-telemetry.transparency.enabled"
            | "terminal.transparency.enabled"
            | "taskbar.blur"
            | "control-panel.blur"
            | "widget-telemetry.blur"
            | "blur.global"
            // Accept the legacy key while importing older profiles; runtime
            // consumers now use blur.global and the canonical global value.
            | "terminal.blur"
            | "taskbar.blur.enabled"
            | "control-panel.blur.enabled"
            | "widget-telemetry.blur.enabled"
            | "terminal.blur.enabled"
        ) || !seen.insert(key)
          || !valid_value
        {
          return Err("invalid theme effects".into());
        }
      }
      // Older profiles contain the original three surfaces (6 or 12 keys);
      // keep accepting them while exporting the four-surface format.
      if !matches!(seen.len(), 6 | 8 | 12 | 16) {
        return Err("invalid theme effects".into());
      }
    }
    FileId::TaskbarRight2Mode if !matches!(value, "auto" | "always-expanded") => {
      return Err("invalid taskbar mode".into());
    }
    FileId::CanonicalConfig => {
      let document: serde_json::Value =
        serde_json::from_slice(data).map_err(|_| "invalid canonical configuration JSON")?;
      if document
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        != Some(1)
        || !document.is_object()
      {
        return Err("invalid canonical configuration schema".into());
      }
      for section in ["appearance", "layout", "effects", "fonts", "control_panel"] {
        if let Some(value) = document.get(section)
          && !value.is_object()
        {
          return Err(format!(
            "invalid canonical configuration section: {section}"
          ));
        }
      }
    }
    FileId::WallpaperCustom
      if text.lines().count() > 1 || (!value.is_empty() && !Path::new(value).is_absolute()) =>
    {
      return Err("invalid wallpaper path".into());
    }
    FileId::Spaces | FileId::Borders
      if text.lines().count() > 16
        || text.split_whitespace().any(|part| {
          part
            .parse::<i32>()
            .ok()
            .is_none_or(|number| !(0..=100).contains(&number))
        }) =>
    {
      return Err(format!("invalid geometry: {}", id.id()));
    }
    FileId::WidgetTelemetryBlocks => {
      for line in text.lines() {
        let (key, state) = line.split_once('=').ok_or("invalid telemetry blocks")?;
        if !WidgetTelemetryBlock::ALL
          .iter()
          .any(|block| block.key() == key)
          || !matches!(state, "enabled" | "disabled")
        {
          return Err("invalid telemetry blocks".into());
        }
      }
    }
    FileId::ControlPanelCards => validate_control_panel_cards(value)?,
    FileId::Fonts => {}
    _ => {}
  }
  Ok(())
}

fn validate_control_panel_cards(value: &str) -> Result<(), String> {
  const IDS: [&str; 14] = [
    "user",
    "notifications",
    "calendar",
    "weather",
    "volume",
    "brightness",
    "network",
    "bluetooth",
    "system",
    "appearance",
    "session",
    "display",
    "spaces-borders-position",
    "power",
  ];
  let json: serde_json::Value =
    serde_json::from_str(value).map_err(|_| "invalid Control Panel cards JSON")?;
  let cards = json
    .get("cards")
    .and_then(serde_json::Value::as_array)
    .ok_or("invalid Control Panel cards list")?;
  if cards.len() != IDS.len() {
    return Err("invalid Control Panel cards count".into());
  }
  let mut seen = HashSet::new();
  for card in cards {
    let id = card
      .get("id")
      .and_then(serde_json::Value::as_str)
      .ok_or("invalid Control Panel card id")?;
    if !IDS.contains(&id)
      || !seen.insert(id)
      || card
        .get("enabled")
        .and_then(serde_json::Value::as_bool)
        .is_none()
    {
      return Err("invalid Control Panel card entry".into());
    }
  }
  Ok(())
}

fn write_archive_atomic(
  path: &Path,
  manifest: &[u8],
  sums: &str,
  contents: &[(String, Vec<u8>)],
  wallpaper: Option<&WallpaperMeta>,
) -> Result<(), String> {
  let temporary = path.with_extension(format!("tar.gz.tmp-{}", std::process::id()));
  let output = File::create(&temporary).map_err(|e| e.to_string())?;
  let mut tar = Builder::new(GzEncoder::new(output, Compression::default()));
  append_bytes(&mut tar, MANIFEST_PATH, manifest)?;
  append_bytes(&mut tar, SUMS_PATH, sums.as_bytes())?;
  for (name, bytes) in contents {
    append_bytes(&mut tar, &format!("{ROOT}{name}"), bytes)?;
  }
  if let Some(meta) = wallpaper {
    append_path(&mut tar, &meta.archive_path, Path::new(&meta.original_path))?;
  }
  tar
    .into_inner()
    .map_err(|e| e.to_string())?
    .finish()
    .map_err(|e| e.to_string())?;
  fs::rename(temporary, path).map_err(|e| e.to_string())
}

fn export_canonical_appearance(destination: &Path) -> Result<bool, String> {
  let config_path = argvus_root();
  let config_home = config_path
    .parent()
    .ok_or_else(|| "invalid ARGVUS configuration path".to_string())?;
  let output = Command::new("argvus-config")
    .args(["export", "--scope", "appearance", "--output"])
    .arg(destination)
    .env("ARGVUS_CONFIG_HOME", config_home)
    .output();
  match output {
    Ok(output) if output.status.success() => Ok(true),
    Ok(output) if output.status.code() == Some(127) => Ok(false),
    Ok(output) => {
      let error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
      Err(if error.is_empty() {
        "argvus-config export failed".into()
      } else {
        error
      })
    }
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
    Err(error) => Err(format!("could not run argvus-config export: {error}")),
  }
}

fn import_canonical_appearance(source: &Path) -> Result<(), String> {
  let source_path = source.to_string_lossy().into_owned();
  run_canonical_config_command(&["import", "--scope", "appearance", &source_path])
}

fn project_canonical_config() -> Result<(), String> {
  run_canonical_config_command(&["project"])
}

fn run_canonical_config_command(arguments: &[&str]) -> Result<(), String> {
  let config_path = argvus_root();
  let config_home = config_path
    .parent()
    .ok_or_else(|| "invalid ARGVUS configuration path".to_string())?;
  let output = Command::new("argvus-config")
    .args(arguments)
    .env("ARGVUS_CONFIG_HOME", config_home)
    .output()
    .map_err(|error| format!("could not run argvus-config: {error}"))?;
  if output.status.success() {
    Ok(())
  } else {
    let error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(if error.is_empty() {
      format!("argvus-config failed with {}", output.status)
    } else {
      error
    })
  }
}
fn append_bytes<W: Write>(tar: &mut Builder<W>, name: &str, bytes: &[u8]) -> Result<(), String> {
  let mut header = Header::new_gnu();
  header.set_size(bytes.len() as u64);
  header.set_mode(0o600);
  header.set_cksum();
  tar
    .append_data(&mut header, name, bytes)
    .map_err(|e| e.to_string())
}
fn append_path<W: Write>(tar: &mut Builder<W>, name: &str, path: &Path) -> Result<(), String> {
  let mut input = File::open(path).map_err(|e| e.to_string())?;
  let size = input.metadata().map_err(|e| e.to_string())?.len();
  if size > MAX_WALLPAPER {
    return Err("wallpaper exceeds profile limit".into());
  }
  let mut header = Header::new_gnu();
  header.set_size(size);
  header.set_mode(0o600);
  header.set_cksum();
  tar
    .append_data(&mut header, name, &mut input)
    .map_err(|e| e.to_string())
}
fn copy_reader_hash(reader: &mut impl Read, target: &Path) -> Result<(u64, String), String> {
  let mut output = OpenOptions::new()
    .write(true)
    .create_new(true)
    .open(target)
    .map_err(|e| e.to_string())?;
  let mut hasher = Sha256::new();
  let mut buffer = [0u8; 64 * 1024];
  let mut size = 0;
  loop {
    let count = reader.read(&mut buffer).map_err(|e| e.to_string())?;
    if count == 0 {
      break;
    }
    output
      .write_all(&buffer[..count])
      .map_err(|e| e.to_string())?;
    hasher.update(&buffer[..count]);
    size += count as u64;
  }
  output.flush().map_err(|e| e.to_string())?;
  Ok((size, hex_digest(hasher.finalize())))
}
fn copy_atomic(source: &Path, destination: &Path) -> Result<(), String> {
  if let Some(parent) = destination.parent() {
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
  }
  let temporary = destination.with_extension(format!("tmp-{}", std::process::id()));
  let mut input = File::open(source).map_err(|e| e.to_string())?;
  let mut output = File::create(&temporary).map_err(|e| e.to_string())?;
  io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
  output.sync_all().map_err(|e| e.to_string())?;
  fs::rename(temporary, destination).map_err(|e| e.to_string())
}
fn restore_wallpaper(meta: &WallpaperMeta, source: &Path) -> Result<(String, bool), String> {
  let current_home = argvus_control_center_core::paths::home();
  let original = Path::new(&meta.original_path);
  let original_home = Path::new(&meta.original_home);
  let target = original
    .strip_prefix(original_home)
    .ok()
    .filter(|relative| {
      !relative.as_os_str().is_empty()
        && relative
          .components()
          .all(|component| !matches!(component, std::path::Component::ParentDir))
    })
    .map(|relative| current_home.join(relative))
    .unwrap_or_else(|| pictures_dir().join("ARGVUS").join(&meta.filename));
  let final_path =
    if target.is_file() && sha256_file(&target).ok().as_deref() != Some(meta.sha256.as_str()) {
      collision_path(&target)
    } else {
      target
    };
  let created = !final_path.is_file();
  if created {
    copy_atomic(source, &final_path)?;
  }
  Ok((final_path.display().to_string(), created))
}
fn collision_path(path: &Path) -> PathBuf {
  let stem = path
    .file_stem()
    .and_then(|v| v.to_str())
    .unwrap_or("wallpaper");
  let extension = path.extension().and_then(|v| v.to_str()).unwrap_or("");
  for index in 1..10000 {
    let name = if extension.is_empty() {
      format!("{stem}-imported-{index}")
    } else {
      format!("{stem}-imported-{index}.{extension}")
    };
    let candidate = path.with_file_name(name);
    if !candidate.exists() {
      return candidate;
    }
  }
  path.with_file_name(format!(
    "{stem}-imported-{}.{extension}",
    std::process::id()
  ))
}
fn current_wallpaper() -> Option<PathBuf> {
  let custom = argvus_root().join(".wallpaper-custom");
  if let Some(path) = fs::read_to_string(custom)
    .ok()
    .map(|value| PathBuf::from(value.trim()))
    .filter(|path| path.is_file())
  {
    return Some(path);
  }
  crate::backend::active_wallpaper()
    .map(|name| PathBuf::from("/usr/share/backgrounds/argvus").join(name))
    .filter(|path| path.is_file())
}
fn wallpaper_meta(path: &Path) -> WallpaperMeta {
  let filename = path
    .file_name()
    .and_then(|v| v.to_str())
    .unwrap_or("wallpaper")
    .to_owned();
  WallpaperMeta {
    original_path: path.display().to_string(),
    original_home: argvus_control_center_core::paths::home()
      .display()
      .to_string(),
    filename: filename.clone(),
    archive_path: format!("{ROOT}wallpaper/{filename}"),
    sha256: sha256_file(path).unwrap_or_default(),
  }
}
fn source_paths(root: &Path) -> Vec<(FileId, PathBuf)> {
  FileId::ALL
    .into_iter()
    .map(|id| {
      let destination = id.destination(root);
      let source =
        if !destination.is_file() && matches!(id, FileId::Animations | FileId::Transparency) {
          let legacy = FileId::Effects.destination(root);
          if legacy.is_file() {
            legacy
          } else {
            destination
          }
        } else {
          destination
        };
      (id, source)
    })
    .collect()
}

fn theme_effects_destination(root: &Path, theme: &str) -> PathBuf {
  root
    .join("state")
    .join("effects")
    .join(format!("{theme}.conf"))
}
fn argvus_root() -> PathBuf {
  argvus_control_center_core::paths::argvus_config_home()
}
fn registry_path() -> PathBuf {
  argvus_root().join("custom-themes.json")
}
fn profile_root() -> PathBuf {
  argvus_control_center_core::paths::data_home()
    .join("argvus")
    .join("themes")
}
fn custom_current_path() -> PathBuf {
  argvus_root().join("state").join("custom-theme")
}
fn pictures_dir() -> PathBuf {
  std::env::var_os("XDG_PICTURES_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(|| argvus_control_center_core::paths::home().join("Pictures"))
}
fn read_first(path: &Path, fallback: &str) -> String {
  fs::read_to_string(path)
    .ok()
    .and_then(|value| {
      value
        .lines()
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
    })
    .unwrap_or_else(|| fallback.to_owned())
}
fn local_timestamp() -> Result<String, String> {
  OffsetDateTime::now_local()
    .unwrap_or_else(|_| OffsetDateTime::now_utc())
    .format(
      &format_description::parse_borrowed::<2>("[year][month][day][hour][minute][second]")
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
fn sha256(bytes: &[u8]) -> String {
  hex_digest(Sha256::digest(bytes))
}
fn sha256_file(path: &Path) -> Result<String, String> {
  let mut file = File::open(path).map_err(|e| e.to_string())?;
  let mut hasher = Sha256::new();
  let mut buffer = [0u8; 64 * 1024];
  loop {
    let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
    if count == 0 {
      break;
    }
    hasher.update(&buffer[..count]);
  }
  Ok(hex_digest(hasher.finalize()))
}
fn hex_digest(digest: impl AsRef<[u8]>) -> String {
  digest
    .as_ref()
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect()
}
fn process_id() -> u32 {
  std::process::id()
}
fn clean_display_name(value: &str) -> Result<String, String> {
  let name = value.split_whitespace().collect::<Vec<_>>().join(" ");
  if name.is_empty() || name.chars().count() > 120 {
    Err("theme name is invalid".into())
  } else {
    Ok(name)
  }
}
fn filename_slug(value: &str) -> String {
  let mut output = String::new();
  let mut underscore = false;
  for character in value.chars().flat_map(char::to_lowercase) {
    if character.is_ascii_alphanumeric() {
      output.push(character);
      underscore = false;
    } else if !underscore {
      output.push('_');
      underscore = true;
    }
  }
  let output = output.trim_matches('_').replace("..", "_");
  if output.is_empty() {
    "theme".into()
  } else {
    output
  }
}
fn is_safe_filename(value: &str) -> bool {
  let path = Path::new(value);
  path.components().count() == 1
    && !matches!(path.file_name(), Some(name) if name == "." || name == "..")
    && !value.chars().any(|character| character.is_control())
}
fn unique_id(slug: &str, themes: &[RegistryTheme]) -> String {
  if !themes.iter().any(|theme| theme.id == slug) {
    return slug.to_owned();
  }
  for index in 2..10000 {
    let candidate = format!("{slug}-{index}");
    if !themes.iter().any(|theme| theme.id == candidate) {
      return candidate;
    }
  }
  format!("{slug}-{}", process_id())
}
fn read_registry() -> Vec<RegistryTheme> {
  fs::read_to_string(registry_path())
    .ok()
    .and_then(|text| serde_json::from_str::<Registry>(&text).ok())
    .filter(|registry| registry.version == 1)
    .map(|registry| registry.themes)
    .unwrap_or_default()
}
fn save_registry(themes: &[RegistryTheme]) -> Result<(), String> {
  let path = registry_path();
  let text = serde_json::to_vec_pretty(&Registry {
    version: 1,
    themes: themes.to_vec(),
  })
  .map_err(|e| e.to_string())?;
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
  }
  let temporary = path.with_extension(format!("json.tmp-{}", process_id()));
  fs::write(&temporary, text).map_err(|e| e.to_string())?;
  fs::rename(temporary, path).map_err(|e| e.to_string())
}
fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
  }
  let temporary = path.with_extension(format!("tmp-{}", process_id()));
  fs::write(&temporary, content).map_err(|e| e.to_string())?;
  fs::rename(temporary, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::Mutex;

  static ENV_LOCK: Mutex<()> = Mutex::new(());
  #[test]
  fn filenames_are_safe() {
    assert_eq!(filename_slug("  Meu   Tema  "), "meu_tema");
    assert!(!filename_slug("../escape").contains(".."));
  }
  #[test]
  fn rejects_bad_values() {
    assert!(validate_content(FileId::Accent, b"#GGGGGG\n").is_err());
    assert!(validate_content(FileId::Effects, b"maybe\n").is_err());
    assert!(validate_content(
      FileId::ThemeEffects,
      b"taskbar.transparency=50\ncontrol-panel.transparency=50\nwidget-telemetry.transparency=50\nterminal.transparency=50\ntaskbar.transparency.enabled=enabled\ncontrol-panel.transparency.enabled=enabled\nwidget-telemetry.transparency.enabled=enabled\nterminal.transparency.enabled=enabled\ntaskbar.blur=50\ncontrol-panel.blur=50\nwidget-telemetry.blur=50\nblur.global=50\ntaskbar.blur.enabled=enabled\ncontrol-panel.blur.enabled=enabled\nwidget-telemetry.blur.enabled=enabled\nterminal.blur.enabled=enabled\n"
    )
    .is_ok());
    assert!(validate_content(FileId::ThemeEffects, b"taskbar.transparency=101\n").is_err());
  }

  #[test]
  fn accepts_canonical_appearance_profile_payload() {
    let payload = br#"{
      "schema_version": 1,
      "appearance": {"theme": "argvus-dark"},
      "layout": {},
      "effects": {},
      "fonts": {},
      "control_panel": {}
    }"#;
    assert!(validate_content(FileId::CanonicalConfig, payload).is_ok());
  }

  #[test]
  fn restoring_profile_backup_reinstates_or_removes_state() {
    let directory = tempfile::tempdir().expect("backup directory");
    let destination = directory.path().join("state");
    let backup = directory.path().join("state.backup");
    fs::write(&destination, b"new state").expect("new state");
    fs::write(&backup, b"previous state").expect("previous state");

    restore_backup(&destination, Some(&backup)).expect("restore previous state");
    assert_eq!(fs::read(&destination).unwrap(), b"previous state");

    restore_backup(&destination, None).expect("remove state without backup");
    assert!(!destination.exists());
  }

  #[test]
  fn keeps_legacy_profile_versions_compatible() {
    assert!(is_supported_profile_version(Some(1)));
    assert!(is_supported_profile_version(Some(2)));
    assert!(is_supported_profile_version(Some(3)));
    assert!(!is_supported_profile_version(Some(4)));
    assert!(!is_supported_profile_version(None));
  }

  #[test]
  fn registry_validation_is_strict() {
    assert!(validate_control_panel_cards(r#"{"cards":[]}"#).is_err());
  }

  #[test]
  fn export_and_import_round_trip_across_xdg_homes() {
    let _guard = ENV_LOCK.lock().expect("profile test lock");
    let old_home = std::env::var_os("HOME");
    let old_config = std::env::var_os("XDG_CONFIG_HOME");
    let old_data = std::env::var_os("XDG_DATA_HOME");
    let old_argvus_config = std::env::var_os("ARGVUS_CONFIG_HOME");
    let source_home = tempfile::tempdir().expect("source home");
    let target_home = tempfile::tempdir().expect("target home");
    let source_config = source_home.path().join("config");
    let target_config = target_home.path().join("config");
    let source_data = source_home.path().join("data");
    let target_data = target_home.path().join("data");
    fs::create_dir_all(source_config.join("argvus")).expect("source config");
    fs::create_dir_all(target_config.join("argvus")).expect("target config");
    fs::write(source_config.join("argvus/.active-theme"), "universe\n").expect("active theme");
    let source_wallpaper = source_home.path().join("Pictures/Wallpapers/foo.png");
    fs::create_dir_all(source_wallpaper.parent().unwrap()).expect("wallpaper directory");
    fs::write(&source_wallpaper, b"wallpaper-bytes").expect("wallpaper");
    fs::write(
      source_config.join("argvus/.wallpaper-custom"),
      format!("{}\n", source_wallpaper.display()),
    )
    .expect("wallpaper state");
    unsafe {
      std::env::set_var("HOME", source_home.path());
      std::env::set_var("XDG_CONFIG_HOME", &source_config);
      std::env::set_var("XDG_DATA_HOME", &source_data);
      std::env::remove_var("ARGVUS_CONFIG_HOME");
    }
    let archive = export_named("Meu Tema").expect("export profile");
    assert!(
      archive
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("meu_tema-")
    );
    assert!(
      archive
        .extension()
        .is_some_and(|extension| extension == "gz")
    );
    let mut archive_reader = Archive::new(GzDecoder::new(File::open(&archive).unwrap()));
    let members = archive_reader
      .entries()
      .unwrap()
      .map(|entry| entry.unwrap().path().unwrap().to_string_lossy().to_string())
      .collect::<HashSet<_>>();
    assert!(members.contains(MANIFEST_PATH));
    assert!(members.contains(SUMS_PATH));
    assert!(
      members
        .iter()
        .any(|member| member.starts_with("argvus-theme-profile/wallpaper/"))
    );
    if Command::new("argvus-config")
      .arg("path")
      .output()
      .is_ok_and(|output| output.status.success())
    {
      assert!(members.contains("argvus-theme-profile/payload/config/argvus/config.json"));
    }
    unsafe {
      std::env::set_var("HOME", target_home.path());
      std::env::set_var("XDG_CONFIG_HOME", &target_config);
      std::env::set_var("XDG_DATA_HOME", &target_data);
      std::env::remove_var("ARGVUS_CONFIG_HOME");
    }
    let imported = import_archive(&archive).expect("import profile");
    assert_eq!(imported.name, "Meu Tema");
    assert!(Path::new(&imported.profile_path).is_file());
    assert_eq!(
      fs::read(target_home.path().join("Pictures/Wallpapers/foo.png")).unwrap(),
      b"wallpaper-bytes"
    );
    assert_eq!(custom_themes().len(), 1);
    unsafe {
      if let Some(value) = old_home {
        std::env::set_var("HOME", value);
      } else {
        std::env::remove_var("HOME");
      }
      if let Some(value) = old_config {
        std::env::set_var("XDG_CONFIG_HOME", value);
      } else {
        std::env::remove_var("XDG_CONFIG_HOME");
      }
      if let Some(value) = old_data {
        std::env::set_var("XDG_DATA_HOME", value);
      } else {
        std::env::remove_var("XDG_DATA_HOME");
      }
      if let Some(value) = old_argvus_config {
        std::env::set_var("ARGVUS_CONFIG_HOME", value);
      } else {
        std::env::remove_var("ARGVUS_CONFIG_HOME");
      }
    }
  }

  #[test]
  fn deleting_inactive_custom_theme_keeps_wallpaper() {
    let _guard = ENV_LOCK.lock().expect("profile test lock");
    let old_home = std::env::var_os("HOME");
    let old_config = std::env::var_os("XDG_CONFIG_HOME");
    let old_data = std::env::var_os("XDG_DATA_HOME");
    let old_argvus_config = std::env::var_os("ARGVUS_CONFIG_HOME");
    let home = tempfile::tempdir().expect("home");
    let config = home.path().join("config");
    let data = home.path().join("data");
    let wallpaper = home.path().join("Pictures/kept.png");
    fs::create_dir_all(&config).expect("config");
    fs::create_dir_all(&data).expect("data");
    fs::create_dir_all(wallpaper.parent().unwrap()).expect("pictures");
    fs::write(&wallpaper, b"keep-me").expect("wallpaper");
    unsafe {
      std::env::set_var("HOME", home.path());
      std::env::set_var("XDG_CONFIG_HOME", &config);
      std::env::set_var("XDG_DATA_HOME", &data);
      std::env::remove_var("ARGVUS_CONFIG_HOME");
    }
    let storage = profile_root().join("kept-theme");
    fs::create_dir_all(&storage).expect("profile storage");
    let profile_path = storage.join("profile.tar.gz");
    fs::write(&profile_path, b"profile").expect("profile");
    save_registry(&[RegistryTheme {
      id: "kept-theme".into(),
      name: "Kept Theme".into(),
      base_theme: "argvus-dark".into(),
      profile_path: profile_path.display().to_string(),
      wallpaper_path: Some(wallpaper.display().to_string()),
    }])
    .expect("registry");
    let theme = custom_themes().pop().expect("custom theme");
    delete_custom_theme(&theme).expect("delete");
    assert!(wallpaper.is_file());
    assert!(custom_themes().is_empty());
    unsafe {
      if let Some(value) = old_home {
        std::env::set_var("HOME", value);
      } else {
        std::env::remove_var("HOME");
      }
      if let Some(value) = old_config {
        std::env::set_var("XDG_CONFIG_HOME", value);
      } else {
        std::env::remove_var("XDG_CONFIG_HOME");
      }
      if let Some(value) = old_data {
        std::env::set_var("XDG_DATA_HOME", value);
      } else {
        std::env::remove_var("XDG_DATA_HOME");
      }
      if let Some(value) = old_argvus_config {
        std::env::set_var("ARGVUS_CONFIG_HOME", value);
      } else {
        std::env::remove_var("ARGVUS_CONFIG_HOME");
      }
    }
  }

  #[test]
  fn rejects_unexpected_archive_member_before_install() {
    let directory = tempfile::tempdir().expect("archive directory");
    let path = directory.path().join("unsafe.tar.gz");
    let output = File::create(&path).expect("archive");
    let mut archive = Builder::new(GzEncoder::new(output, Compression::default()));
    let mut header = Header::new_gnu();
    header.set_size(4);
    header.set_mode(0o600);
    header.set_cksum();
    archive
      .append_data(
        &mut header,
        "argvus-theme-profile/escape",
        b"nope".as_slice(),
      )
      .expect("write unsafe archive");
    archive
      .into_inner()
      .expect("finish tar")
      .finish()
      .expect("finish gzip");
    assert!(stage_archive(&path).is_err());
  }
}
