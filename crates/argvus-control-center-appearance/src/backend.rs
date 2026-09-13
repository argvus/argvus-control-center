use crate::model::AppearanceState;
use argvus_control_center_core::{
  paths::{argvus_config_home, cache_home, system_config_root},
  process::{ProcessRequest, ProcessRunner, SystemProcessRunner},
  sanitize::terminal_text,
};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

const WALLPAPERS_DIR: &str = "/usr/share/backgrounds/argvus";
const DEFAULT_THEME: &str = "argvus-dark-aether";
const DEFAULT_ACCENT: &str = "#3590bd";

fn scripts_dir() -> PathBuf {
  system_config_root().join("scripts").join("argvus")
}

fn script(name: &str) -> PathBuf {
  scripts_dir().join(name)
}

fn run_script(script_path: &Path, args: &[&str]) -> Result<(), String> {
  if !script_path.is_file() {
    return Err(format!("script não encontrado: {}", script_path.display()));
  }
  let mut request = ProcessRequest::new("sh").arg(script_path.to_string_lossy().to_string());
  for argument in args {
    if argument.is_empty()
      || argument
        .chars()
        .any(|character| character.is_control() || character == '\n')
    {
      return Err("invalid script argument".into());
    }
    request = request.arg(*argument);
  }
  let output = SystemProcessRunner
    .run(&request)
    .map_err(|error| error.to_string())?;
  if output.status.is_none_or(|status| status != 0) {
    let stderr = terminal_text(&String::from_utf8_lossy(&output.stderr))
      .trim()
      .to_string();
    return Err(if stderr.is_empty() {
      format!("{} falhou", script_path.display())
    } else {
      stderr
    });
  }
  Ok(())
}

fn run_script_output(script_path: &Path, args: &[&str]) -> Option<String> {
  if !script_path.is_file() {
    return None;
  }
  let mut request = ProcessRequest::new("sh").arg(script_path.to_string_lossy().to_string());
  for argument in args {
    if argument.is_empty()
      || argument
        .chars()
        .any(|character| character.is_control() || character == '\n')
    {
      return None;
    }
    request = request.arg(*argument);
  }
  let output = SystemProcessRunner.run(&request).ok()?;
  if output.status.is_none_or(|status| status != 0) {
    return None;
  }
  Some(terminal_text(&String::from_utf8_lossy(&output.stdout)))
}

fn command_words(command: &str) -> Vec<String> {
  command
    .split_whitespace()
    .map(str::to_string)
    .filter(|word| !word.is_empty())
    .collect()
}

fn is_tui_file_manager(command: &[String]) -> bool {
  matches!(
    command.first().map(String::as_str),
    Some(
      "argvus"
        | "spf"
        | "superfile"
        | "yazi"
        | "ranger"
        | "lf"
        | "joshuto"
        | "broot"
        | "mc"
        | "nnn"
    )
  )
}

fn default_app(category: &str) -> Option<Vec<String>> {
  let output = run_script_output(
    &system_config_root().join("scripts/argvus/get-default.sh"),
    &[category],
  )?;
  let words = command_words(output.trim());
  (!words.is_empty()).then_some(words)
}

/// Runs a Hyprland Lua `dispatch` and reports whether it was accepted. Places
/// that dropped a dispatcher (e.g. `window not found`) still exit 0, logging a
/// `warning:`/`error:` on stderr, so only stderr-clean runs count as success.
fn hyprctl_lua_dispatch(expression: &str) -> bool {
  let Ok(output) =
    SystemProcessRunner.run(&ProcessRequest::new("hyprctl").arg("dispatch").arg(expression))
  else {
    return false;
  };
  if output.timed_out || output.status != Some(0) {
    return false;
  }
  let stderr = String::from_utf8_lossy(&output.stderr);
  !stderr.contains("warning:") && !stderr.contains("error:")
}

/// Focuses a window (by address or class) and raises it to the front, above the
/// Control Center window that launched it.
fn focus_and_raise(target: &str) {
  // Only raise once the target is actually focused; otherwise the dispatcher
  // below would raise whatever window currently has focus (the Control Center).
  if hyprctl_lua_dispatch(&format!("hl.dsp.focus({{ window = '{target}' }})")) {
    let _ = hyprctl_lua_dispatch("hl.dsp.window.bring_to_top({})");
  }
}

/// Terminal-backed wallpaper chooser: opens as a floating, centered window in
/// the standard 1024x768 File Manager proportion, on top of the Control Center.
fn focus_wallpaper_picker(child_id: u32) {
  // `argvus-tui-terminal` `exec`s the terminal, so `child_id` is also the PID
  // of the Wayland surface. Cold terminal startup can take a few seconds, so
  // poll patiently; only float/center/raise a window that actually exists, to
  // avoid operating on whatever has focus meanwhile (the Control Center).
  for _ in 0..80 {
    thread::sleep(Duration::from_millis(100));
    let Some(address) = client_address_for_pid(child_id) else {
      continue;
    };
    if hyprctl_lua_dispatch(&format!("hl.dsp.focus({{ window = 'address:{address}' }})")) {
      let _ = hyprctl_lua_dispatch("hl.dsp.window.float({ action = 'enable' })");
      let _ = hyprctl_lua_dispatch("hl.dsp.window.resize({ x = 1024, y = 768 })");
      let _ = hyprctl_lua_dispatch("hl.dsp.window.center({})");
      let _ = hyprctl_lua_dispatch("hl.dsp.window.bring_to_top({})");
    }
    return;
  }
  // Fallback for wrappers that fork instead of exec: match the known class.
  for _ in 0..40 {
    thread::sleep(Duration::from_millis(100));
    if hyprctl_lua_dispatch("hl.dsp.focus({ window = 'class:argvus-wallpaper-picker' })") {
      let _ = hyprctl_lua_dispatch("hl.dsp.window.float({ action = 'enable' })");
      let _ = hyprctl_lua_dispatch("hl.dsp.window.resize({ x = 1024, y = 768 })");
      let _ = hyprctl_lua_dispatch("hl.dsp.window.center({})");
      let _ = hyprctl_lua_dispatch("hl.dsp.window.bring_to_top({})");
      break;
    }
  }
}

/// Best-effort Wayland class (`class:`) used by the GUI File Managers Argvus
/// knows about. TUI managers are matched by their own `argvus-wallpaper-picker`
/// terminal window instead.
fn gui_file_manager_class(command: &str) -> Option<&'static str> {
  let name = Path::new(command)
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or(command);
  match name {
    "nautilus" => Some("org.gnome.Nautilus"),
    "nemo" => Some("org.nemo.Nemo"),
    "thunar" => Some("Thunar"),
    "dolphin" => Some("org.kde.dolphin"),
    "pcmanfm" | "pcmanfm-qt" => Some("pcmanfm"),
    "krusader" => Some("org.kde.krusader"),
    "caja" => Some("org.mate.Caja"),
    "doublecmd" => Some("doublecmd"),
    _ => None,
  }
}

/// Resolves the Hyprland client address (`0x…`) mapped to `pid`, if the
/// window already exists.
fn client_address_for_pid(pid: u32) -> Option<String> {
  let output = SystemProcessRunner
    .run(&ProcessRequest::new("hyprctl").arg("-j").arg("clients"))
    .ok()?;
  if output.status.is_none_or(|status| status != 0) {
    return None;
  }
  let value: Value = serde_json::from_slice(&output.stdout).ok()?;
  value
    .as_array()?
    .iter()
    .find(|client| client.get("pid").and_then(Value::as_u64) == Some(pid as u64))
    .and_then(|client| client.get("address").and_then(Value::as_str))
    .map(str::to_string)
}

/// Brings a freshly launched GUI File Manager window to the front, focused, on
/// top of the `argvus-control-center` window. The Wayland surface appears
/// asynchronously, so the exact process is polled first; GTK/KDE apps may be
/// D-Bus activated, in which case the known class is used as a fallback.
fn raise_file_manager_window(pid: u32, class: Option<&str>) {
  for _ in 0..80 {
    thread::sleep(Duration::from_millis(100));
    if let Some(address) = client_address_for_pid(pid) {
      focus_and_raise(&format!("address:{address}"));
      return;
    }
  }
  if let Some(class) = class {
    let target = format!("class:{class}");
    for _ in 0..40 {
      thread::sleep(Duration::from_millis(100));
      focus_and_raise(&target);
    }
  }
}

fn read_first(path: &Path, fallback: &str) -> String {
  fs::read_to_string(path)
    .ok()
    .and_then(|content| content.lines().next().map(str::to_string))
    .filter(|line| !line.trim().is_empty())
    .unwrap_or_else(|| fallback.to_string())
}

fn read_first_or_default(path: &Path) -> Option<String> {
  fs::read_to_string(path)
    .ok()
    .and_then(|content| content.lines().next().map(str::to_string))
    .map(|line| line.trim().to_string())
}

fn active_theme_file() -> PathBuf {
  argvus_config_home().join(".active-theme")
}

fn accent_file() -> PathBuf {
  argvus_config_home().join(".accent-color")
}

fn spaces_file() -> PathBuf {
  argvus_config_home().join(".spaces")
}

fn theme_default_accent(theme: &str) -> Option<&'static str> {
  match theme {
    "argvus-dark-aether" | "argvus-dark-aether-float" => Some("#3590bd"),
    "argvus-dark-silver" | "argvus-dark-silver-float" => Some("#595959"),
    "argvus-light-veil" | "argvus-light-veil-float" => Some("#181818"),
    "argvus-dark-slate" | "argvus-dark-slate-float" => Some("#7391a5"),
    "argvus-dark-universe" | "argvus-dark-universe-float" => Some("#eeeeee"),
    _ => None,
  }
}

fn load_spaces(state: &mut AppearanceState) {
  let mut gaps_in = None;
  let mut gaps_out = None;
  let mut waybar = None;
  let mut waybar_pos = None;
  if let Ok(content) = fs::read_to_string(spaces_file()) {
    for line in content.lines() {
      let Some((key, value)) = line.split_once('=') else {
        continue;
      };
      match key.trim() {
        "gaps_in" => gaps_in = value.trim().parse::<i32>().ok(),
        "gaps_out" => gaps_out = value.trim().parse::<i32>().ok(),
        "waybar" => waybar = value.trim().parse::<i32>().ok(),
        "waybar_pos" => waybar_pos = Some(value.trim().to_string()),
        _ => {}
      }
    }
  }
  if let Some(value) = gaps_in {
    state.gaps_in = value;
  }
  if let Some(value) = gaps_out {
    state.gaps_out = value;
  }
  if let Some(value) = waybar {
    state.waybar = value;
  }
  match waybar_pos.as_deref() {
    Some("bottom") => state.waybar_pos = "bottom".into(),
    Some(_) => state.waybar_pos = "top".into(),
    None if state.is_float_theme() => {
      state.waybar_pos = "top".into();
    }
    None => {}
  }
}

fn effects_state() -> bool {
  let path = argvus_config_home().join("state").join("effects");
  match read_first_or_default(&path).as_deref() {
    Some("enabled") => true,
    Some("disabled") => false,
    _ => matches!(
      run_script_output(&script("effects-toggle.sh"), &["status"])
        .as_deref()
        .map(str::trim),
      Some("enabled")
    ),
  }
}

fn telemetry_state() -> bool {
  if let Ok(output) =
    SystemProcessRunner.run(&ProcessRequest::new("argvus-widget-telemetry-toggle").arg("status"))
    && output.status.is_none_or(|status| status == 0)
  {
    match terminal_text(&String::from_utf8_lossy(&output.stdout)).trim() {
      "enabled" => return true,
      "disabled" => return false,
      _ => {}
    }
  }
  let cache = cache_home().join("argvus").join("waybar");
  let candidates = [
    cache.join("widget-telemetry-state"),
    cache.join("desktop-telemetry-state"),
    cache.join("sysinfo-state"),
  ];
  for candidate in candidates {
    match read_first_or_default(&candidate).as_deref() {
      Some("enabled") => return true,
      Some("disabled") => return false,
      _ => {}
    }
  }
  false
}

pub fn list_wallpapers() -> Vec<String> {
  let mut names = fs::read_dir(WALLPAPERS_DIR)
    .map(|entries| {
      entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<_>>()
    })
    .unwrap_or_default();
  names.sort();
  names
}

fn hyprpaper_config_path() -> PathBuf {
  argvus_config_home().join("hypr").join("hyprpaper.conf")
}

pub fn active_wallpaper() -> Option<String> {
  let content = fs::read_to_string(hyprpaper_config_path()).ok()?;
  let path = content.lines().find_map(|line| {
    let (key, value) = line.split_once('=')?;
    if key.trim() != "path" {
      return None;
    }
    let value = value.trim();
    let expanded = value.strip_prefix('~').map(|rest| {
      let home = argvus_control_center_core::paths::home();
      home.join(rest).to_string_lossy().to_string()
    });
    Some(expanded.unwrap_or_else(|| value.to_string()))
  })?;
  path
    .strip_prefix(WALLPAPERS_DIR)
    .map(|name| name.trim_start_matches('/').to_string())
}

fn first_monitor() -> Option<String> {
  let output = SystemProcessRunner
    .run(&ProcessRequest::new("hyprctl").arg("monitors"))
    .ok()?;
  if output.status.is_none_or(|status| status != 0) {
    return None;
  }
  String::from_utf8_lossy(&output.stdout)
    .lines()
    .find_map(|line| line.strip_prefix("Monitor "))?
    .split_whitespace()
    .next()
    .map(str::to_string)
}

/// Writes `monitor =` and `path =` lines into the user hyprpaper.conf,
/// keeping the existing block form used by `hypr-wallpaper-pick.sh`.
fn write_hyprpaper_config(wallpaper: &Path) -> Result<(), String> {
  let config_path = hyprpaper_config_path();
  let existing = fs::read_to_string(&config_path).unwrap_or_default();
  let relative = wallpaper.to_string_lossy().replace(
    &argvus_control_center_core::paths::home()
      .to_string_lossy()
      .to_string(),
    "~",
  );
  let monitor = first_monitor().unwrap_or_default();
  if existing
    .lines()
    .any(|line| line.trim_start().starts_with("path ="))
  {
    let mut text = String::new();
    for line in existing.lines() {
      let trimmed = line.trim_start();
      if trimmed.starts_with("path =") {
        text.push_str(&format!("  path = {relative}\n"));
      } else {
        text.push_str(line);
        text.push('\n');
      }
    }
    if !text.contains("monitor =") {
      text = format!("  monitor = {monitor}\n{text}");
    } else {
      let mut patched = String::new();
      for line in text.lines() {
        if line.trim_start().starts_with("monitor =") {
          patched.push_str(&format!("  monitor = {monitor}\n"));
        } else {
          patched.push_str(line);
          patched.push('\n');
        }
      }
      text = patched;
    }
    write_atomic(&config_path, &text)?;
    return Ok(());
  }
  let text = format!(
    "wallpaper {{\n  monitor = {monitor}\n  path = {relative}\n  fit_mode = cover\n}}\n\npreload = {relative}\n"
  );
  write_atomic(&config_path, &text)
}

fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
  if let Some(parent) = path.parent()
    && !parent.exists()
  {
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
  }
  let tmp = path.with_extension("tmp");
  fs::write(&tmp, content).map_err(|error| error.to_string())?;
  fs::rename(&tmp, path).map_err(|error| error.to_string())
}

pub fn load_state() -> AppearanceState {
  let mut state = AppearanceState::default();
  let theme = read_first(&active_theme_file(), DEFAULT_THEME);
  state.theme = theme.clone();
  let default_accent = theme_default_accent(&theme).unwrap_or(DEFAULT_ACCENT);
  state.accent = read_first(&accent_file(), default_accent);
  state.wallpapers = list_wallpapers();
  state.wallpaper_active = active_wallpaper();
  state.effects = effects_state();
  state.widget_telemetry = telemetry_state();
  load_spaces(&mut state);
  state
}

pub fn set_theme(name: &str) -> Result<(), String> {
  run_script(&script("theme-switch.sh"), &[name])
}

pub fn set_accent(color: &str) -> Result<(), String> {
  run_script(&script("accent-switch.sh"), &[color])
}

pub fn set_effects(enabled: bool) -> Result<(), String> {
  run_script(
    &script("effects-toggle.sh"),
    &[if enabled { "enable" } else { "disable" }],
  )
}

pub fn set_telemetry(enabled: bool) -> Result<(), String> {
  let output = SystemProcessRunner.run(
    &ProcessRequest::new("argvus-widget-telemetry-toggle").arg(if enabled { "on" } else { "off" }),
  );
  match output {
    Ok(output) if output.status.is_none_or(|status| status != 0) => Err(format!(
      "argvus-widget-telemetry-toggle falhou: {}",
      terminal_text(&String::from_utf8_lossy(&output.stderr)).trim()
    )),
    Ok(_) => Ok(()),
    Err(_) => {
      let path = cache_home()
        .join("argvus")
        .join("waybar")
        .join("widget-telemetry-state");
      write_atomic(&path, if enabled { "enabled\n" } else { "disabled\n" })
    }
  }
}

pub fn set_wallpaper(name: &str) -> Result<(), String> {
  if name.is_empty()
    || name
      .chars()
      .any(|character| character.is_control() || character == '/' || character == '\n')
  {
    return Err("invalid wallpaper name".into());
  }
  let path = PathBuf::from(WALLPAPERS_DIR).join(name);
  if !path.is_file() {
    return Err(format!("papel de parede não encontrado: {name}"));
  }
  write_hyprpaper_config(&path)?;
  let _ = SystemProcessRunner.run(
    &ProcessRequest::new("systemctl")
      .arg("--user")
      .arg("restart")
      .arg("argvus-wallpaper.service"),
  );
  if let Ok(output) = SystemProcessRunner.run(
    &ProcessRequest::new("sh")
      .arg(script("hyprlock-theme.sh").to_string_lossy().to_string())
      .arg("--invalidate"),
  ) {
    let _ = output;
  }
  Ok(())
}

/// Opens the configured File Manager in a window focused on top of the Control
/// Center. TUI managers are launched inside a dedicated terminal (the
/// `argvus-wallpaper-picker` class); GUI managers are raised via their WM class
/// or matched by PID to guarantee they appear in front.
pub fn choose_wallpaper() -> Result<(), String> {
  let file_manager =
    default_app("file_manager").ok_or_else(|| "file manager padrão não encontrado".to_string())?;
  let home = argvus_control_center_core::paths::home();

  if !is_tui_file_manager(&file_manager) {
    let binary = &file_manager[0];
    let class = gui_file_manager_class(binary);
    let child = Command::new(binary)
      .args(file_manager.iter().skip(1))
      .arg(home)
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .spawn()
      .map_err(|error| format!("não foi possível abrir o File Manager padrão: {error}"))?;
    raise_file_manager_window(child.id(), class);
    return Ok(());
  }

  let selection = cache_home()
    .join("argvus")
    .join(format!("wallpaper-selection-{}", std::process::id()));
  if let Some(parent) = selection.parent() {
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
  }
  let _ = fs::remove_file(&selection);

  let mut terminal_command = Command::new("argvus-tui-terminal");
  let mut terminal_args = vec![
    "--class".to_string(),
    "argvus-wallpaper-picker".to_string(),
    "--term".to_string(),
    "foot".to_string(),
    "--".to_string(),
  ];
  for argument in &file_manager {
    terminal_args.push(argument.clone());
  }
  terminal_args.push(format!("--chooser-file={}", selection.display()));
  terminal_args.push(home.to_string_lossy().to_string());

  let mut child = terminal_command
    .args(terminal_args)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .map_err(|error| format!("não foi possível abrir o File Manager padrão: {error}"))?;
  focus_wallpaper_picker(child.id());
  if child
    .wait()
    .map_err(|error| format!("File Manager padrão falhou: {error}"))?
    .code()
    .is_some_and(|status| status != 0)
  {
    return Err("File Manager padrão falhou".into());
  }

  let selected =
    fs::read_to_string(&selection).map_err(|_| "nenhuma imagem foi selecionada".to_string())?;
  let selected = selected.trim();
  if selected.is_empty() {
    return Err("nenhuma imagem foi selecionada".into());
  }
  let selected = Path::new(selected);
  if !selected.is_file() {
    return Err("o arquivo selecionado não existe".into());
  }
  write_hyprpaper_config(selected)?;
  let _ = SystemProcessRunner.run(
    &ProcessRequest::new("systemctl")
      .arg("--user")
      .arg("restart")
      .arg("argvus-wallpaper.service"),
  );
  let _ = fs::remove_file(selection);
  Ok(())
}

pub fn set_spaces(key: &str, value: &str) -> Result<(), String> {
  if key.is_empty() || value.is_empty() {
    return Err("invalid spaces key/value".into());
  }
  run_script(&script("spaces-switch.sh"), &["--set", key, value])?;
  run_script(&script("spaces-switch.sh"), &["--apply"])
}

pub fn set_waybar_position(position: &str) -> Result<(), String> {
  if position != "top" && position != "bottom" {
    return Err("invalid waybar position".into());
  }
  run_script(
    &script("spaces-switch.sh"),
    &["--set", "waybar_pos", position],
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::Mutex;

  static ENV_LOCK: Mutex<()> = Mutex::new(());

  #[test]
  fn theme_default_accents_exist() {
    assert_eq!(
      theme_default_accent("argvus-dark-slate-float"),
      Some("#7391a5")
    );
    assert_eq!(theme_default_accent("unknown"), None);
  }

  #[test]
  fn spaces_file_round_trips() {
    let dir = std::env::temp_dir().join(format!("argvus-appearance-spaces-{}", std::process::id()));
    let _guard = ENV_LOCK.lock().unwrap();
    let original = argvus_config_home();
    unsafe { std::env::set_var("ARGVUS_CONFIG_HOME", &dir) };
    let _ = fs::remove_dir_all(spaces_file().parent().unwrap());
    fs::create_dir_all(spaces_file().parent().unwrap()).unwrap();
    fs::write(
      spaces_file(),
      "gaps_in=4\ngaps_out=2\nwaybar=16\nwaybar_pos=bottom\n",
    )
    .unwrap();
    let mut state = AppearanceState::default();
    load_spaces(&mut state);
    assert_eq!(state.gaps_in, 4);
    assert_eq!(state.gaps_out, 2);
    assert_eq!(state.waybar, 16);
    assert_eq!(state.waybar_pos, "bottom");
    unsafe { std::env::set_var("ARGVUS_CONFIG_HOME", original.to_str().unwrap_or("/tmp/n")) };
    let _ = fs::remove_dir_all(&dir);
  }

  #[test]
  fn gui_file_manager_class_known_apps() {
    assert_eq!(
      gui_file_manager_class("nautilus"),
      Some("org.gnome.Nautilus")
    );
    assert_eq!(gui_file_manager_class("thunar"), Some("Thunar"));
    assert_eq!(gui_file_manager_class("nemo"), Some("org.nemo.Nemo"));
    assert_eq!(gui_file_manager_class("dolphin"), Some("org.kde.dolphin"));
    assert_eq!(gui_file_manager_class("pcmanfm"), Some("pcmanfm"));
    assert_eq!(
      gui_file_manager_class("/usr/bin/nautilus"),
      Some("org.gnome.Nautilus")
    );
    assert_eq!(gui_file_manager_class("unknown-fm"), None);
    assert_eq!(gui_file_manager_class("spf"), None);
  }
}
