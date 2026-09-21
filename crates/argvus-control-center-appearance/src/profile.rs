//! Versioned, allowlisted ARGVUS appearance profile archives.
//!
//! Only logical user state is serialized. Generated consumers remain owned by
//! the normal appearance scripts and are deliberately absent from this format.

use crate::model::{THEMES, WidgetTelemetryBlock};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tar::{Archive, Builder, EntryType, Header};

const ROOT: &str = "argvus-theme-profile/";
const MAX_MEMBERS: usize = 64;
const MAX_UNCOMPRESSED: u64 = 16 * 1024 * 1024;
const MAX_FILE: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileId {
  ActiveTheme,
  Accent,
  GtkMode,
  Spaces,
  Borders,
  WallpaperCustom,
  Fonts,
  Effects,
  TaskbarRight2Mode,
  WidgetTelemetryBlocks,
  ControlPanelCards,
}

impl FileId {
  const ALL: [Self; 11] = [
    Self::ActiveTheme,
    Self::Accent,
    Self::GtkMode,
    Self::Spaces,
    Self::Borders,
    Self::WallpaperCustom,
    Self::Fonts,
    Self::Effects,
    Self::TaskbarRight2Mode,
    Self::WidgetTelemetryBlocks,
    Self::ControlPanelCards,
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
      Self::TaskbarRight2Mode => "taskbar-right-2-mode",
      Self::WidgetTelemetryBlocks => "widget-telemetry-blocks",
      Self::ControlPanelCards => "control-panel-cards",
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
      Self::TaskbarRight2Mode => "payload/config/argvus/state/taskbar-right-2-mode",
      Self::WidgetTelemetryBlocks => "payload/config/argvus/state/widget-telemetry-blocks",
      Self::ControlPanelCards => "payload/config/argvus/control-panel/cards.json",
    }
  }
  fn destination(self, root: &Path) -> PathBuf {
    root.join(match self {
      Self::ActiveTheme => ".active-theme",
      Self::Accent => ".accent-color",
      Self::GtkMode => ".gtk-mode",
      Self::Spaces => ".spaces",
      Self::Borders => ".borders",
      Self::WallpaperCustom => ".wallpaper-custom",
      Self::Fonts => "fonts.conf",
      Self::Effects => "state/effects",
      Self::TaskbarRight2Mode => "state/taskbar-right-2-mode",
      Self::WidgetTelemetryBlocks => "state/widget-telemetry-blocks",
      Self::ControlPanelCards => "control-panel/cards.json",
    })
  }
  fn parse(value: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|id| id.id() == value)
  }
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
  format: String,
  schema_version: u32,
  created_at: String,
  argvus: ProfileArgvus,
  files: Vec<ManifestFile>,
  wallpaper: Wallpaper,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProfileArgvus {
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
#[derive(Debug, Serialize, Deserialize)]
struct Wallpaper {
  kind: String,
  path: Option<String>,
}

// Keep this wrapper private and typo-free in the serialized API.
impl Manifest {
  fn new(theme: String, files: Vec<ManifestFile>, wallpaper: Wallpaper) -> Self {
    Self {
      format: "argvus-theme-profile".into(),
      schema_version: 1,
      created_at: now(),
      argvus: ProfileArgvus {
        theme,
        mode: "sticky".into(),
      },
      files,
      wallpaper,
    }
  }
}

fn now() -> String {
  use std::time::{SystemTime, UNIX_EPOCH};
  let seconds = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();
  format!("unix:{seconds}")
}

fn sha(bytes: &[u8]) -> String {
  Sha256::digest(bytes)
    .iter()
    .map(|b| format!("{b:02x}"))
    .collect()
}

fn source_paths(root: &Path) -> Vec<(FileId, PathBuf)> {
  FileId::ALL
    .into_iter()
    .map(|id| (id, id.destination(root)))
    .collect()
}

pub fn export(path: &Path) -> Result<(), String> {
  if path.extension().and_then(|v| v.to_str()) != Some("gz")
    || !path.to_string_lossy().ends_with(".tar.gz")
  {
    return Err("profile path must end in .tar.gz".into());
  }
  let root = argvus_root();
  let mut files = Vec::new();
  let mut contents = Vec::new();
  for (id, source) in source_paths(&root) {
    if !source.is_file() {
      continue;
    }
    let bytes = fs::read(&source).map_err(|e| format!("{}: {e}", id.id()))?;
    if bytes.len() as u64 > MAX_FILE {
      return Err(format!("{} is too large", id.id()));
    }
    files.push(ManifestFile {
      id: id.id().into(),
      source_path: source.display().to_string(),
      archive_path: format!("{}{}", ROOT, id.archive_path()),
      sha256: sha(&bytes),
    });
    contents.push((id.archive_path(), bytes));
  }
  let theme = fs::read_to_string(root.join(".active-theme"))
    .unwrap_or_else(|_| "argvus-dark-aether".into())
    .trim()
    .to_string();
  let wallpaper_path = fs::read_to_string(root.join(".wallpaper-custom"))
    .ok()
    .map(|v| v.trim().to_string())
    .filter(|v| !v.is_empty());
  let wallpaper = Wallpaper {
    kind: if wallpaper_path.is_some() {
      "custom"
    } else {
      "theme"
    }
    .into(),
    path: wallpaper_path,
  };
  let manifest = serde_json::to_vec_pretty(&Manifest::new(theme, files.clone(), wallpaper))
    .map_err(|e| e.to_string())?;
  let sums = files
    .iter()
    .map(|f| format!("{}  {}\n", f.sha256, f.archive_path))
    .collect::<String>();
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
  }
  let output = File::create(path).map_err(|e| e.to_string())?;
  let encoder = GzEncoder::new(output, Compression::default());
  let mut tar = Builder::new(encoder);
  append_bytes(&mut tar, "argvus-theme-profile/manifest.json", &manifest)?;
  append_bytes(&mut tar, "argvus-theme-profile/SHA256SUMS", sums.as_bytes())?;
  for (name, bytes) in contents {
    append_bytes(&mut tar, &format!("{ROOT}{name}"), &bytes)?;
  }
  tar
    .into_inner()
    .map_err(|e| e.to_string())?
    .finish()
    .map_err(|e| e.to_string())?;
  Ok(())
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

pub fn import(path: &Path) -> Result<String, String> {
  if !path.is_file() || !path.to_string_lossy().ends_with(".tar.gz") {
    return Err("invalid archive path".into());
  }
  let file = File::open(path).map_err(|e| e.to_string())?;
  let decoder = GzDecoder::new(file);
  let mut archive = Archive::new(decoder);
  let mut members = HashSet::new();
  let mut bytes_total = 0;
  let mut raw = HashMap::new();
  for item in archive.entries().map_err(|e| e.to_string())? {
    let mut entry = item.map_err(|e| e.to_string())?;
    let name = entry
      .path()
      .map_err(|e| e.to_string())?
      .to_string_lossy()
      .to_string();
    if name.starts_with('/') || name.split('/').any(|p| p == "..") || !members.insert(name.clone())
    {
      return Err("unsafe or duplicate archive member".into());
    }
    let kind = entry.header().entry_type();
    if !(kind == EntryType::Regular || kind == EntryType::Continuous) {
      return Err("archive contains a non-regular member".into());
    }
    let size = entry.size();
    bytes_total += size;
    if bytes_total > MAX_UNCOMPRESSED || size > MAX_FILE {
      return Err("archive exceeds safety limits".into());
    }
    let mut data = Vec::with_capacity(size as usize);
    entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
    raw.insert(name, data);
  }
  if members.len() > MAX_MEMBERS {
    return Err("archive has too many members".into());
  }
  let manifest: Manifest = serde_json::from_slice(
    raw
      .get("argvus-theme-profile/manifest.json")
      .ok_or("missing manifest")?,
  )
  .map_err(|e| format!("invalid manifest: {e}"))?;
  if manifest.format != "argvus-theme-profile" {
    return Err("invalid archive format".into());
  }
  if manifest.schema_version != 1 {
    return Err("unsupported profile schema".into());
  }
  if !THEMES
    .iter()
    .any(|(name, _)| *name == manifest.argvus.theme)
  {
    return Err("unknown manifest theme".into());
  }
  let sums = raw
    .get("argvus-theme-profile/SHA256SUMS")
    .ok_or("missing checksums")?;
  let expected_sums = manifest
    .files
    .iter()
    .map(|file| format!("{}  {}\n", file.sha256, file.archive_path))
    .collect::<String>();
  if sums != expected_sums.as_bytes() {
    return Err("checksum index mismatch".into());
  }
  let root = argvus_root();
  let mut staged = Vec::new();
  let mut seen = HashSet::new();
  for file in &manifest.files {
    let id = FileId::parse(&file.id).ok_or("unknown profile file id")?;
    if !seen.insert(id) || file.archive_path != format!("{ROOT}{}", id.archive_path()) {
      return Err("invalid profile file mapping".into());
    }
    if !Path::new(&file.source_path).is_absolute() {
      return Err("profile source path is not absolute".into());
    }
    let data = raw
      .get(&file.archive_path)
      .ok_or("missing profile payload")?;
    if sha(data) != file.sha256 {
      return Err(format!("checksum mismatch: {}", file.id));
    }
    validate_content(id, data, &root)?;
    staged.push((id, data.clone()));
  }
  if !seen.contains(&FileId::ActiveTheme) {
    return Err("missing active-theme payload".into());
  }
  let payload_theme = staged
    .iter()
    .find(|(id, _)| *id == FileId::ActiveTheme)
    .map(|(_, data)| String::from_utf8_lossy(data).trim().to_string())
    .ok_or("missing active-theme payload")?;
  if payload_theme != manifest.argvus.theme {
    return Err("manifest theme does not match payload".into());
  }
  let mut expected = HashSet::from([
    "argvus-theme-profile/manifest.json".to_string(),
    "argvus-theme-profile/SHA256SUMS".to_string(),
  ]);
  for file in &manifest.files {
    expected.insert(file.archive_path.clone());
  }
  if members != expected {
    return Err("archive contains unexpected or missing members".into());
  }
  let backup = tempfile::tempdir().map_err(|e| e.to_string())?;
  let mut backups = Vec::new();
  for (id, _) in &staged {
    let destination = id.destination(&root);
    if destination.is_file() {
      let copy = backup.path().join(id.id());
      fs::copy(&destination, &copy).map_err(|e| e.to_string())?;
      backups.push((destination, Some(copy)));
    } else {
      backups.push((destination, None));
    }
  }
  let result = apply_staged(&root, &staged);
  if let Err(error) = result {
    for (destination, old) in backups {
      if let Some(old) = old {
        let _ = fs::copy(old, &destination);
      } else {
        let _ = fs::remove_file(destination);
      }
    }
    return Err(format!("import rolled back: {error}"));
  }
  Ok("theme profile imported".into())
}

fn apply_staged(root: &Path, staged: &[(FileId, Vec<u8>)]) -> Result<(), String> {
  let theme = staged
    .iter()
    .find(|(id, _)| *id == FileId::ActiveTheme)
    .map(|(_, b)| String::from_utf8_lossy(b).trim().to_string())
    .ok_or("missing theme")?;
  crate::backend::set_theme(&theme)?;
  for (id, data) in staged {
    let destination = id.destination(root);
    if let Some(parent) = destination.parent() {
      fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = destination.with_extension("profile.tmp");
    fs::write(&tmp, data).map_err(|e| e.to_string())?;
    let _ = fs::set_permissions(&tmp, std::os::unix::fs::PermissionsExt::from_mode(0o600));
    fs::rename(tmp, destination).map_err(|e| e.to_string())?;
  }
  apply_overrides(staged)?;
  crate::backend::reload_session_for_profile()
}

fn apply_overrides(staged: &[(FileId, Vec<u8>)]) -> Result<(), String> {
  for (id, data) in staged {
    let value = String::from_utf8_lossy(data).trim().to_string();
    match id {
      FileId::Accent => crate::backend::run_profile_script("accent-switch.sh", &["--apply"]),
      FileId::Effects => crate::backend::run_profile_script(
        "effects-toggle.sh",
        &[if value == "enabled" {
          "enable"
        } else {
          "disable"
        }],
      ),
      FileId::Spaces => crate::backend::run_profile_script("spaces-switch.sh", &["--apply"]),
      FileId::Borders => crate::backend::run_profile_script("borders-switch.sh", &["--apply"]),
      FileId::TaskbarRight2Mode => {
        crate::backend::run_profile_script("taskbar-right-2-mode.sh", &["set", &value])
      }
      FileId::WidgetTelemetryBlocks => {
        crate::backend::run_profile_script("argvus-widget-telemetry-toggle", &["blocks", "apply"])
      }
      FileId::WallpaperCustom if !value.is_empty() && Path::new(&value).is_file() => {
        crate::backend::run_profile_script("hypr-wallpaper-pick.sh", &["--apply", &value])
      }
      _ => Ok(()),
    }?
  }
  Ok(())
}

fn validate_content(id: FileId, data: &[u8], root: &Path) -> Result<(), String> {
  let text = std::str::from_utf8(data).map_err(|_| format!("invalid UTF-8: {}", id.id()))?;
  let value = text.trim();
  match id {
    FileId::ActiveTheme if !THEMES.iter().any(|(name, _)| *name == value) => {
      return Err("unknown theme".into());
    }
    FileId::Accent => {
      let valid = value.len() == 7
        && value.starts_with('#')
        && value[1..].bytes().all(|b| b.is_ascii_hexdigit());
      if !valid {
        return Err("invalid accent".into());
      }
    }
    FileId::Effects if !matches!(value, "enabled" | "disabled") => {
      return Err("invalid effects state".into());
    }
    FileId::GtkMode if !matches!(value, "light" | "dark" | "auto" | "sticky") => {
      return Err("invalid GTK mode".into());
    }
    FileId::TaskbarRight2Mode if !matches!(value, "auto" | "always-expanded") => {
      return Err("invalid utility mode".into());
    }
    FileId::WidgetTelemetryBlocks => {
      for line in text.lines() {
        let mut parts = line.split('=');
        let key = parts.next().unwrap_or_default();
        let state = parts.next().unwrap_or_default();
        if !WidgetTelemetryBlock::ALL.iter().any(|b| b.key() == key)
          || !matches!(state, "enabled" | "disabled")
        {
          return Err("invalid telemetry blocks".into());
        }
      }
    }
    FileId::WallpaperCustom => {
      if text.lines().count() > 1 || value.contains('\0') {
        return Err("invalid wallpaper path".into());
      }
      if !value.is_empty() && !Path::new(value).is_absolute() {
        return Err("wallpaper path must be absolute".into());
      }
    }
    FileId::Spaces | FileId::Borders
      if text.lines().count() > 8
        || text.split_whitespace().any(|v| {
          v.parse::<i32>()
            .ok()
            .is_none_or(|n| !(0..=100).contains(&n))
        }) =>
    {
      return Err(format!("invalid geometry: {}", id.id()));
    }
    FileId::ControlPanelCards => validate_control_panel_cards(value)?,
    FileId::Fonts => {
      let _ = root;
    }
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
    serde_json::from_str(value).map_err(|_| "invalid control-panel cards JSON")?;
  let cards = json
    .get("cards")
    .and_then(serde_json::Value::as_array)
    .ok_or("invalid control-panel cards list")?;
  if cards.len() != IDS.len() {
    return Err("invalid control-panel cards count".into());
  }
  let mut seen = std::collections::HashSet::new();
  for card in cards {
    let id = card
      .get("id")
      .and_then(serde_json::Value::as_str)
      .ok_or("invalid control-panel card id")?;
    if !IDS.contains(&id)
      || !seen.insert(id)
      || card
        .get("enabled")
        .and_then(serde_json::Value::as_bool)
        .is_none()
    {
      return Err("invalid control-panel card entry".into());
    }
  }
  Ok(())
}

fn argvus_root() -> PathBuf {
  argvus_control_center_core::paths::argvus_config_home()
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn ids_have_stable_paths() {
    assert_eq!(
      FileId::ActiveTheme.archive_path(),
      "payload/config/argvus/.active-theme"
    );
    assert!(FileId::parse("active-theme").is_some());
  }
  #[test]
  fn rejects_bad_values() {
    assert!(validate_content(FileId::Accent, b"#GGGGGG\n", Path::new("/tmp")).is_err());
    assert!(validate_content(FileId::Effects, b"maybe\n", Path::new("/tmp")).is_err());
  }
}
