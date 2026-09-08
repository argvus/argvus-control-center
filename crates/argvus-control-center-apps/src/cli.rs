use std::process::ExitCode;

use argvus_control_center_core::i18n::label;
use argvus_control_center_core::paths;

use crate::apply;
use crate::catalog::{Category, effective_values, find_app};
use crate::detect;
use crate::state::AppState;

const PROG: &str = "argvus-control-center";
const CATEGORY_KEYS: &str = "terminal, file_manager, text_editor, terminal_editor, browser, image_viewer, pdf_viewer, video_player, audio_player, archive, launcher";

pub fn run(args: &[String]) -> ExitCode {
  let Some(cmd) = args.first().map(String::as_str) else {
    usage(false);
    return ExitCode::from(2);
  };

  match cmd {
    "help" | "--help" | "-h" => {
      usage(true);
      ExitCode::SUCCESS
    }
    "version" | "--version" | "-V" => {
      println!("{PROG} {}", env!("CARGO_PKG_VERSION"));
      ExitCode::SUCCESS
    }
    "list" => cmd_list(&args[1..]),
    "get" => cmd_get(&args[1..]),
    "set" => cmd_set(&args[1..]),
    "categories" => {
      for c in Category::ORDER {
        println!("{}", c.key());
      }
      ExitCode::SUCCESS
    }
    unknown => {
      eprintln!(
        "{PROG}: {} '{unknown}'",
        label("comando desconhecido", "unknown command")
      );
      usage(false);
      ExitCode::from(2)
    }
  }
}

fn bad_category(key: &str) -> ExitCode {
  eprintln!(
    "{PROG}: {} '{key}' ({}: {CATEGORY_KEYS})",
    label("categoria desconhecida", "unknown category"),
    label("válidas", "valid")
  );
  ExitCode::from(2)
}

fn cmd_list(args: &[String]) -> ExitCode {
  let desktops = detect::scan_desktop_files();
  let cats: Vec<Category> = match args.first() {
    None => Category::ORDER.to_vec(),
    Some(key) => match Category::from_key(key) {
      Some(c) => vec![c],
      None => return bad_category(key),
    },
  };

  let state = AppState::load();
  for cat in &cats {
    let current = state.effective(*cat);
    let apps = detect::installed_apps(*cat, &desktops);
    let title = if !current.is_empty() {
      format!("{} ({})", cat.key(), label("atual", "current"))
    } else {
      format!("{} ({})", cat.key(), label("sem padrão", "no default"))
    };
    println!("[{title}]");
    if apps.is_empty() {
      println!("  ({})", label("nenhum detectado", "none detected"));
      continue;
    }
    for app in &apps {
      let marker = if app.is_current(Some(&current)) {
        " *"
      } else {
        "  "
      };
      let id = app.desktop_id.as_deref().unwrap_or("");
      let tui = if app.tui { " [tui]" } else { "" };
      let mut line = format!("{marker} {:<20} {}{tui}", app.binary, app.display);
      if !id.is_empty() {
        line.push_str(&format!(" ({id})"));
      }
      println!("{line}");
    }
    println!();
  }
  ExitCode::SUCCESS
}

fn cmd_get(args: &[String]) -> ExitCode {
  let state = AppState::load();
  match args.first() {
    None => {
      for (cat, value) in effective_values(&as_list(&state)) {
        println!("{}={value}", cat.key());
      }
      ExitCode::SUCCESS
    }
    Some(key) => match Category::from_key(key) {
      Some(cat) => {
        let value = state.effective(cat);
        if value.is_empty() {
          eprintln!(
            "{PROG}: {} '{key}' ({})",
            label("nenhum padrão definido para", "no default set for"),
            label("usa o padrão do sistema", "falls back to xdg")
          );
          ExitCode::from(1)
        } else {
          println!("{value}");
          ExitCode::SUCCESS
        }
      }
      None => bad_category(key),
    },
  }
}

fn cmd_set(args: &[String]) -> ExitCode {
  if args.len() < 2 {
    eprintln!(
      "{}: {PROG} set <{}> <{}>",
      label("Uso", "Usage"),
      label("categoria", "category"),
      label("app", "app")
    );
    return ExitCode::from(2);
  }
  let cat = match Category::from_key(&args[0]) {
    Some(c) => c,
    None => return bad_category(&args[0]),
  };
  let app_query = &args[1];

  let desktops = detect::scan_desktop_files();
  let binary = find_app(cat, app_query)
    .map(|a| a.binary.to_string())
    .unwrap_or_else(|| app_query.to_string());

  if binary == "default" {
    let mut state = AppState::load();
    state.set(cat, "");
    return match state.save() {
      Ok(()) => {
        let reloaded = apply::refresh_argvus();
        println!("{}={}", cat.key(), label("padrão", "default"));
        if reloaded {
          println!(
            "  ARGVUS: {}",
            label("hyprctl reload solicitado", "hyprctl reload requested")
          );
        }
        ExitCode::SUCCESS
      }
      Err(e) => {
        eprintln!(
          "{PROG}: {} {}: {e}",
          label("não foi possível escrever", "could not write"),
          paths::defaults_file().display()
        );
        ExitCode::from(1)
      }
    };
  }

  if !detect::binary_available(&binary, &desktops) {
    eprintln!(
      "{PROG}: '{}' {} '{}'",
      binary,
      label(
        "não está instalado na categoria",
        "is not installed in category"
      ),
      cat.key()
    );
    eprintln!("{}:", label("Opções instaladas", "Installed options"));
    for app in detect::installed_apps(cat, &desktops) {
      eprintln!("  {} ({})", app.binary, app.display);
    }
    return ExitCode::from(1);
  }

  let mut state = AppState::load();
  state.set(cat, binary.clone());
  if let Err(e) = state.save() {
    eprintln!(
      "{PROG}: {} {}: {e}",
      label("não foi possível escrever", "could not write"),
      paths::defaults_file().display()
    );
    return ExitCode::from(1);
  }

  match apply::apply(cat, &binary, &desktops) {
    Ok(report) => {
      println!("{}={}", cat.key(), binary);
      if let Some(id) = report.desktop_id {
        println!(
          "  {}: {id}",
          label(".desktop registrado", "registered .desktop")
        );
      }
      if let Some(path) = report.mimeapps_updated {
        println!(
          "  {}: {}",
          label("mimeapps atualizado", "updated mimeapps"),
          path.display()
        );
      }
      if report.xdg_settings {
        println!(
          "  xdg-settings: {}",
          label("navegador padrão definido", "default-web-browser set")
        );
      }
      if report.hyprctl_reloaded {
        println!(
          "  ARGVUS: {}",
          label("hyprctl reload solicitado", "hyprctl reload requested")
        );
      }
      ExitCode::SUCCESS
    }
    Err(err) => {
      eprintln!("{PROG}: {err}");
      ExitCode::from(1)
    }
  }
}

fn as_list(state: &AppState) -> Vec<(Category, Option<String>)> {
  Category::ORDER
    .iter()
    .copied()
    .map(|c| (c, state.get(c)))
    .collect()
}

fn usage(verbose: bool) {
  if argvus_control_center_core::i18n::is_pt() {
    println!(
      "{PROG} - selecione e alterne os aplicativos padrão do ARGVUS\n\
       \n\
       Uso:\n\
         {PROG} list [categoria]\n\
         {PROG} get [categoria]\n\
         {PROG} set <categoria> <app>\n\
         {PROG} categories\n\
         {PROG} help\n\
         {PROG} version"
    );
  } else {
    println!(
      "{PROG} - pick and switch the ARGVUS default applications\n\
       \n\
       Usage:\n\
         {PROG} list [category]\n\
         {PROG} get [category]\n\
         {PROG} set <category> <app>\n\
         {PROG} categories\n\
         {PROG} help\n\
         {PROG} version"
    );
  }

  if verbose {
    println!(
      "\n{}:\n  {CATEGORY_KEYS}\n\n{}: {}",
      label("Categorias", "Categories"),
      label("Estado", "State"),
      paths::defaults_file().display()
    );
  }
}
