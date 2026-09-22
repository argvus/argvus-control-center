//! Implements keyboard-binding defaults and overrides in crate `argvus control center settings`. This separation keeps external effects from contaminating models, routes, or rendering.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::PathBuf, process::Command};

use argvus_i18n::Lang;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Represents `Binding`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub struct Binding {
  pub id: String,
  pub category: String,
  pub description_key: String,
  pub keys: String,
  pub action: String,
  #[serde(default)]
  pub context: String,
  #[serde(default)]
  pub input: String,
  #[serde(default = "default_true")]
  pub configurable: bool,
  #[serde(default)]
  pub flags: Vec<String>,
  #[serde(default)]
  pub enabled: bool,
}
/// Executes the `default_true` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn default_true() -> bool {
  true
}
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Represents `Manifest`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
struct Manifest {
  version: u32,
  bindings: Vec<Binding>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
/// Represents `Overrides`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub struct Overrides {
  pub version: u32,
  #[serde(default)]
  pub keybindings: BTreeMap<String, Override>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
/// Represents `Override`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub struct Override {
  pub keys: Option<String>,
  pub enabled: Option<bool>,
}

/// Executes the `manifest_path` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn manifest_path() -> PathBuf {
  std::env::var_os("ARGVUS_SYSTEM_CONFIG")
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from("/usr/share/argvus"))
    .join("hyprland/keybindings.json")
}
/// Executes the `config_path` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn config_path() -> PathBuf {
  crate::config::paths::argvus_config_home().join("keybindings.toml")
}
/// Executes the `generated_path` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn generated_path() -> PathBuf {
  crate::config::paths::argvus_config_home().join("generated/hypr/keybindings.lua")
}
/// Executes the `generated_cheatsheet_path` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn generated_cheatsheet_path() -> PathBuf {
  crate::config::paths::argvus_config_home().join("generated/hypr/keybindings.txt")
}

/// Executes the `cheatsheet_description` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn cheatsheet_description(lang: Lang, binding: &Binding) -> Option<String> {
  let portuguese = lang.locale().starts_with("pt");
  if let Some(number) = binding.id.strip_prefix("workspace.switch.") {
    return Some(if portuguese {
      format!("Área de trabalho {number}")
    } else {
      format!("Workspace {number}")
    });
  }
  if let Some(number) = binding.id.strip_prefix("workspace.move.") {
    return Some(if portuguese {
      format!("Mover janela para a área de trabalho {number}")
    } else {
      format!("Move window to desktop {number}")
    });
  }
  let description = match binding.id.as_str() {
    "window.toggle_floating" => (
      "Enable/Disable Floating Window",
      "Ativa/Desativa Janela flutuante",
    ),
    "focus.left" | "focus.right" | "focus.up" | "focus.down" => (
      "Focus tiled window by direction",
      "Focar janela tileada por direção",
    ),
    "focus.cycle_left" | "focus.cycle_right" => (
      "Cycle focus between windows (tiled and floating)",
      "Ciclar foco entre janelas (tileadas e flutuantes)",
    ),
    "workspace.next"
    | "workspace.previous"
    | "workspace.next_mouse"
    | "workspace.previous_mouse" => (
      "Moving between work areas",
      "Circular entre áreas de trabalho",
    ),
    "navigation.alt_tab_next" | "navigation.alt_tab_previous" => {
      ("Cycle through all windows", "Circular entre todas janelas")
    }
    "resize.enter" => (
      "Enter window resize mode (floating window only)",
      "Entrar modo redimensionar janela (apenas flutuante)",
    ),
    "resize.right" | "resize.left" | "resize.up" | "resize.down" => ("Resize", "Redimensionar"),
    "resize.move_right" | "resize.move_left" | "resize.move_up" | "resize.move_down" => {
      ("Move window", "Mover janela")
    }
    "resize.cancel_escape" | "resize.cancel_return" => {
      ("Exit resize mode", "Sair modo redimensionar")
    }
    "window.drag_mouse" => (
      "Drag window (floating window only)",
      "Arrastar janela (apenas flutuante)",
    ),
    "window.resize_mouse" => (
      "Window resize mode (floating window only)",
      "Modo redimensionar janela (apenas flutuante)",
    ),
    "window.fullscreen" => ("Full screen window", "Tela cheia"),
    "window.maximize" => ("Maximize window (toggle)", "Maximizar janela (toggle)"),
    "window.toggle_split" => (
      "Split vertical/horizontal toggle",
      "Alternar split vertical/horizontal",
    ),
    "window.group_tabs" => ("Group/Ungroup into tabs", "Agrupar/Desagrupar em abas"),
    "window.next_tab" => ("Navigate between tabs", "Navegar entre abas"),
    id if id.starts_with("window.move_") => ("Move window", "Mover janela"),
    "window.close" => ("Close window", "Fechar janela"),
    id if id.starts_with("workspace.move.") => (
      "Move window to desktop",
      "Mover janela para a área de trabalho",
    ),
    "screenshot.region" => ("Capture selected area", "Capturar área selecionada"),
    "screenshot.window" => ("Capture focused window", "Capturar janela em foco"),
    "screenshot.fullscreen" => ("Capture entire screen", "Capturar tela inteira"),
    "record.toggle" => (
      "Start/Pause/Resume screen recording",
      "Iniciar/Pausar/Retomar gravação de tela",
    ),
    "record.stop" => (
      "Stop and save screen recording",
      "Parar e salvar gravação de tela",
    ),
    "widget.sidebar" | "widget.sidebar_mouse" => (
      "Open/Close notification sidebar",
      "Abre/Fecha sidebar de notificações",
    ),
    "widget.taskbar_toggle" => ("Toggle Waybar top", "Oculta/Mostra Waybar top"),
    "appearance.wallpaper" => (
      "Open wallpaper selector",
      "Abre seletor para trocar wallpaper",
    ),
    "appearance.theme" => ("Open theme selector", "Abre o seletor de temas"),
    "session.idle_timeout" => (
      "Choose inactivity lock timeout",
      "Escolhe o tempo de bloqueio por inatividade",
    ),
    "session.keep_awake" => ("Toggle Keep Awake", "Ativa/Desativa manter acordado"),
    "widget.weather" => (
      "Configure weather location",
      "Configura a localização do clima",
    ),
    "appearance.brightness" => ("Open brightness selector", "Abre o seletor de brilho"),
    "appearance.mode" => (
      "Toggle GTK Dark and Light themes",
      "Altera entre tema Dark e Light do GTK",
    ),
    "appearance.animations" => ("Toggle animations", "Ativa/Desativa animações"),
    "system.about" => ("Open About ARGVUS", "Abrir Sobre o ARGVUS"),
    "session.lock" => ("Lock system", "Bloquear sistema"),
    "session.dpms" => (
      "Turn monitor off/on (not suspend system)",
      "Desligar/Ligar monitor (não é suspender)",
    ),
    "session.exit" => ("Exit system", "Sair do sistema"),
    "session.reload" => ("Reload Hyprland", "Recarregar Hyprland"),
    "widget.waybar_top" | "widget.waybar_bottom" => (
      "Move taskbar to top/bottom",
      "Mover barra de tarefas para topo/inferior",
    ),
    "session.volume_up" => ("Volume up", "Aumentar volume"),
    "session.volume_down" => ("Volume down", "Diminuir volume"),
    "session.mute" => ("Mute", "Silenciar"),
    "session.brightness_up" => ("Brightness up", "Aumentar brilho"),
    "session.brightness_down" => ("Brightness down", "Diminuir brilho"),
    "session.play_pause" => ("Play/Pause", "Reproduzir/Pausar"),
    "session.next_track" => ("Next track", "Próxima faixa"),
    "session.previous_track" => ("Previous track", "Faixa anterior"),
    "session.stop_track" => ("Stop track", "Parar faixa"),
    "app.terminal" => ("Terminal", "Terminal"),
    "app.file_manager" => ("File Manager", "Gerenciador de arquivos"),
    "app.removable_devices" => ("Removable devices menu", "Menu de dispositivos removíveis"),
    "app.browser" => ("Default Browser", "Navegador padrão"),
    "app.launcher" => ("Program Launcher", "Lançador de programas"),
    "app.calculator" => ("Calculator", "Calculadora"),
    "system.kitty_cheatsheet" => ("Kitty Cheatsheets", "Cheatsheets do Kitty"),
    "system.hyprland_cheatsheet" => ("Hyprland Cheatsheets", "Cheatsheets do Hyprland"),
    "system.clipboard" => ("Clipboard History", "Histórico no Clipboard"),
    "system.clipboard_clear" => ("Clear Clipboard History", "Apagar histórico do Clipboard"),
    "system.color_picker" => ("Color Picker", "Seletor de cores (Color Picker)"),
    "system.emoji_picker" => ("Emoji Picker", "Emoji Picker"),
    "system.control_center" => (
      "Default apps selector (argvus-default-apps)",
      "Seletor de aplicativos padrão (argvus-default-apps)",
    ),
    _ => return None,
  };
  Some(if portuguese {
    description.1.to_string()
  } else {
    description.0.to_string()
  })
}
/// Retrieves data for `load` without mixing collection with TUI rendering. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn load() -> Vec<Binding> {
  let Ok(raw) = fs::read_to_string(manifest_path()) else {
    return Vec::new();
  };
  let Ok(manifest) = serde_json::from_str::<Manifest>(&raw) else {
    return Vec::new();
  };
  let overrides = fs::read_to_string(config_path())
    .ok()
    .and_then(|v| toml::from_str::<Overrides>(&v).ok())
    .unwrap_or_default();
  manifest
    .bindings
    .into_iter()
    .map(|mut b| {
      b.keys = normalize_keys(&b.keys).unwrap_or(b.keys);
      if let Some(o) = overrides.keybindings.get(&b.id) {
        if let Some(k) = &o.keys {
          b.keys = normalize_keys(k).unwrap_or(b.keys)
        }
        if let Some(e) = o.enabled {
          b.enabled = e
        }
      } else {
        b.enabled = true
      };
      b
    })
    .collect()
}
/// Executes the `normalize_keys` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn normalize_keys(value: &str) -> Result<String, String> {
  let mut parts: Vec<String> = value
    .split('+')
    .map(str::trim)
    .filter(|v| !v.is_empty())
    .map(|v| match v.to_ascii_lowercase().as_str() {
      "super" | "win" | "meta" => "SUPER".into(),
      "ctrl" | "control" => "CTRL".into(),
      "alt" => "ALT".into(),
      "shift" => "SHIFT".into(),
      "enter" | "return" => "Return".into(),
      "esc" | "escape" => "Escape".into(),
      "space" => "Space".into(),
      "tab" => "Tab".into(),
      "left" | "leftarrow" => "left".into(),
      "right" | "rightarrow" => "right".into(),
      "up" | "uparrow" => "up".into(),
      "down" | "downarrow" => "down".into(),
      "pageup" => "Page_Up".into(),
      "pagedown" => "Page_Down".into(),
      "backspace" => "BackSpace".into(),
      "delete" => "Delete".into(),
      "insert" => "Insert".into(),
      "home" => "Home".into(),
      "end" => "End".into(),
      "/" | "slash" => "slash".into(),
      "-" | "minus" => "minus".into(),
      "." | "period" => "period".into(),
      "," | "comma" => "comma".into(),
      ";" | "semicolon" => "semicolon".into(),
      "'" | "apostrophe" => "apostrophe".into(),
      "[" | "bracketleft" => "bracketleft".into(),
      "]" | "bracketright" => "bracketright".into(),
      "\\" | "backslash" => "backslash".into(),
      "`" | "grave" => "grave".into(),
      "=" | "equal" => "equal".into(),
      "xf86audiolowervolume" => "XF86AudioLowerVolume".into(),
      "xf86audiomute" => "XF86AudioMute".into(),
      "xf86audionext" => "XF86AudioNext".into(),
      "xf86audioplay" => "XF86AudioPlay".into(),
      "xf86audioprev" => "XF86AudioPrev".into(),
      "xf86audiostop" => "XF86AudioStop".into(),
      "xf86monbrightnessdown" => "XF86MonBrightnessDown".into(),
      "xf86monbrightnessup" => "XF86MonBrightnessUp".into(),
      v => {
        if v.starts_with("mouse:") || v.starts_with("XF86") || v == "Print" {
          v.to_string()
        } else if v.len() == 1
          || (v.len() >= 2
            && v.starts_with('f')
            && v[1..].parse::<u8>().is_ok_and(|n| (1..=24).contains(&n)))
        {
          v.to_ascii_uppercase()
        } else {
          v.to_string()
        }
      }
    })
    .collect();
  let key = parts.pop().ok_or("shortcut needs a key")?;
  if ["CTRL", "ALT", "SHIFT", "SUPER"].contains(&key.as_str()) {
    return Err("shortcut needs a non-modifier key".into());
  }
  let mut mods = parts;
  mods.sort_by_key(|v| {
    ["SUPER", "CTRL", "ALT", "SHIFT"]
      .iter()
      .position(|m| m == v)
      .unwrap_or(99)
  });
  mods.dedup();
  Ok(if mods.is_empty() {
    key
  } else {
    format!("{} + {key}", mods.join(" + "))
  })
}

/// Executes the `display_key` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn display_key(key: &str) -> String {
  match key {
    "slash" => "/".into(),
    "minus" => "-".into(),
    "period" => ".".into(),
    "comma" => ",".into(),
    "semicolon" => ";".into(),
    "apostrophe" => "'".into(),
    "bracketleft" => "[".into(),
    "bracketright" => "]".into(),
    "backslash" => "\\".into(),
    "grave" => "`".into(),
    "equal" => "=".into(),
    "left" => "Left Arrow".into(),
    "right" => "Right Arrow".into(),
    "up" => "Up Arrow".into(),
    "down" => "Down Arrow".into(),
    "Return" => "Enter".into(),
    "Escape" => "Escape".into(),
    "Page_Up" => "Page Up".into(),
    "Page_Down" => "Page Down".into(),
    "BackSpace" => "Backspace".into(),
    "XF86AudioLowerVolume" => "Volume Down".into(),
    "XF86AudioMute" => "Mute".into(),
    "XF86AudioNext" => "Next Track".into(),
    "XF86AudioPlay" => "Play/Pause".into(),
    "XF86AudioPrev" => "Previous Track".into(),
    "XF86AudioStop" => "Stop".into(),
    "XF86MonBrightnessDown" => "Brightness Down".into(),
    "XF86MonBrightnessUp" => "Brightness Up".into(),
    other => other.into(),
  }
}
/// Executes the `conflicts` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn conflicts(bindings: &[Binding], id: &str, keys: &str) -> Vec<String> {
  conflicts_in_context(bindings, id, keys, "", "")
}
/// Executes the `conflicts_in_context` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn conflicts_in_context(
  bindings: &[Binding],
  id: &str,
  keys: &str,
  context: &str,
  input: &str,
) -> Vec<String> {
  bindings
    .iter()
    .filter(|b| {
      let current = normalize_keys(&b.keys).ok();
      let requested = normalize_keys(keys).ok();
      b.id != id
        && b.enabled
        && b.configurable
        && current == requested
        && b.context == context
        && b.input == input
    })
    .map(|b| b.id.clone())
    .collect()
}
/// Applies the `save_override` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn save_override(
  id: &str,
  keys: Option<String>,
  enabled: Option<bool>,
  bindings: &[Binding],
) -> Result<(), String> {
  let mut file = fs::read_to_string(config_path())
    .ok()
    .and_then(|v| toml::from_str::<Overrides>(&v).ok())
    .unwrap_or(Overrides {
      version: 1,
      keybindings: BTreeMap::new(),
    });
  let default = defaults()
    .into_iter()
    .find(|b| b.id == id)
    .ok_or("unknown keybinding")?;
  let normalized_keys = keys.as_deref().map(normalize_keys).transpose()?;
  if normalized_keys.as_deref() == Some(normalize_keys(&default.keys)?.as_str())
    && enabled.unwrap_or(true)
  {
    file.keybindings.remove(id);
  } else {
    file.keybindings.insert(
      id.into(),
      Override {
        keys: normalized_keys,
        enabled,
      },
    );
  }
  let target = config_path();
  fs::create_dir_all(target.parent().ok_or("invalid config path")?).map_err(|e| e.to_string())?;
  let tmp = target.with_extension("toml.tmp");
  fs::write(
    &tmp,
    toml::to_string_pretty(&file).map_err(|e| e.to_string())?,
  )
  .map_err(|e| e.to_string())?;
  fs::rename(tmp, target).map_err(|e| e.to_string())?;
  generate(bindings)
}
/// Executes the `restore_all` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn restore_all(bindings: &[Binding]) -> Result<(), String> {
  let p = config_path();
  if p.exists() {
    fs::remove_file(p).map_err(|e| e.to_string())?;
  }
  generate(bindings)
}
/// Executes the `restore` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn restore(id: &str, bindings: &[Binding]) -> Result<(), String> {
  let mut file = fs::read_to_string(config_path())
    .ok()
    .and_then(|v| toml::from_str::<Overrides>(&v).ok())
    .unwrap_or_default();
  file.keybindings.remove(id);
  let target = config_path();
  fs::create_dir_all(target.parent().ok_or("invalid config path")?).map_err(|e| e.to_string())?;
  fs::write(
    &target,
    toml::to_string_pretty(&file).map_err(|e| e.to_string())?,
  )
  .map_err(|e| e.to_string())?;
  generate(bindings)
}
/// Executes the `generate` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn generate(bindings: &[Binding]) -> Result<(), String> {
  let overrides = fs::read_to_string(config_path())
    .ok()
    .and_then(|v| toml::from_str::<Overrides>(&v).ok())
    .unwrap_or_default();
  let mut out = String::from(
    "-- Generated by argvus-control-center; defaults remain in argvus-hyprland.\nreturn {\n",
  );
  for (id, value) in overrides.keybindings {
    out.push_str(&format!("  [{}] = {{", lua_key(&id)));
    if let Some(keys) = value.keys {
      let keys = normalize_keys(&keys)?;
      out.push_str(&format!(" keys = {:?},", keys));
    }
    if let Some(enabled) = value.enabled {
      out.push_str(&format!(" enabled = {},", enabled));
    }
    out.push_str(" },\n");
  }
  out.push_str("}\n");
  let p = generated_path();
  fs::create_dir_all(p.parent().ok_or("invalid generated path")?).map_err(|e| e.to_string())?;
  let tmp = p.with_extension("lua.tmp");
  fs::write(&tmp, out).map_err(|e| e.to_string())?;
  if Command::new("luac")
    .args(["-p", tmp.to_str().unwrap_or("")])
    .status()
    .is_ok_and(|status| !status.success())
  {
    return Err("generated keybindings Lua is invalid".into());
  }
  fs::rename(tmp, p).map_err(|e| e.to_string())?;
  let mut cheatsheet =
    String::from("ARGVUS effective Hyprland shortcuts\n===================================\n\n");
  let language = argvus_i18n::Lang::detect();
  for binding in bindings.iter().filter(|binding| binding.configurable) {
    let state = if binding.enabled {
      &display_keys(&binding.keys)
    } else {
      "Disabled"
    };
    let description = cheatsheet_description(language, binding)
      .unwrap_or_else(|| language.tr(&binding.description_key));
    let description = if description == binding.description_key {
      binding
        .id
        .rsplit('.')
        .next()
        .unwrap_or(&binding.id)
        .replace('_', " ")
    } else {
      description
    };
    cheatsheet.push_str(&format!("{:<34} {}\n", state, description));
  }
  let cheat_path = generated_cheatsheet_path();
  let cheat_tmp = cheat_path.with_extension("txt.tmp");
  fs::write(&cheat_tmp, cheatsheet).map_err(|e| e.to_string())?;
  fs::rename(cheat_tmp, cheat_path).map_err(|e| e.to_string())
}
/// Executes the `display_keys` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
pub fn display_keys(keys: &str) -> String {
  let mut modifiers = Vec::new();
  let mut key = "";
  for part in keys.split(" + ") {
    match part.to_ascii_uppercase().as_str() {
      "SUPER" | "CTRL" | "ALT" | "SHIFT" => modifiers.push(part.to_ascii_uppercase()),
      _ => key = part,
    }
  }
  modifiers.sort_by_key(|part| {
    ["SUPER", "CTRL", "ALT", "SHIFT"]
      .iter()
      .position(|m| m == part)
      .unwrap_or(99)
  });
  let mut output = modifiers
    .into_iter()
    .map(|part| {
      if part == "SHIFT" {
        "Shift".into()
      } else {
        part
      }
    })
    .collect::<Vec<String>>();
  if !key.is_empty() {
    output.push(display_key(key));
  }
  output.join(" + ")
}
/// Executes the `defaults` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn defaults() -> Vec<Binding> {
  let Ok(raw) = fs::read_to_string(manifest_path()) else {
    return Vec::new();
  };
  serde_json::from_str::<Manifest>(&raw)
    .map(|m| m.bindings)
    .unwrap_or_default()
}
/// Executes the `lua_key` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn lua_key(s: &str) -> String {
  format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  /// Executes the `canonicalizes_modifiers` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn canonicalizes_modifiers() {
    assert_eq!(
      normalize_keys("shift + super + q").unwrap(),
      "SUPER + SHIFT + Q"
    );
  }
  #[test]
  /// Executes the `rejects_empty` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn rejects_empty() {
    assert!(normalize_keys("CTRL +").is_err());
  }
  #[test]
  /// Executes the `deduplicates_modifiers` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn deduplicates_modifiers() {
    assert_eq!(normalize_keys("SUPER + super + q").unwrap(), "SUPER + Q");
  }
  #[test]
  /// Executes the `canonicalizes_symbols_and_special_keys` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn canonicalizes_symbols_and_special_keys() {
    for (raw, expected) in [
      ("/", "slash"),
      ("-", "minus"),
      (".", "period"),
      (",", "comma"),
      (";", "semicolon"),
      ("'", "apostrophe"),
      ("[", "bracketleft"),
      ("]", "bracketright"),
      ("\\", "backslash"),
      ("`", "grave"),
      ("=", "equal"),
      ("F1", "F1"),
      ("F12", "F12"),
      ("XF86AudioMute", "XF86AudioMute"),
      ("left", "left"),
      ("Return", "Return"),
    ] {
      assert_eq!(normalize_keys(raw).unwrap(), expected, "{raw}");
    }
  }
  #[test]
  /// Executes the `display_keys_hides_internal_keysyms` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn display_keys_hides_internal_keysyms() {
    assert_eq!(display_keys("SUPER + SHIFT + slash"), "SUPER + Shift + /");
    assert_eq!(
      display_keys("SHIFT + CTRL + ALT + SUPER + slash"),
      "SUPER + CTRL + ALT + Shift + /"
    );
    assert_eq!(display_keys("SUPER + F1"), "SUPER + F1");
    assert_eq!(display_keys("XF86AudioMute"), "Mute");
  }
}
