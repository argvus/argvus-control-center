use crate::app::App;
use crate::i18n::tr;

use super::{Doc, Row, simple_doc};

pub fn doc(app: &App, width: usize, selected: usize) -> Doc<'static> {
  let lang = app.lang;
  let rows = vec![
    key_value(tr(lang, "Dispositivo", "Device"), &app.system.hostname),
    key_value(tr(lang, "Nome do S.O.", "OS name"), &app.system.os_name),
    key_value(tr(lang, "Tipo do S.O.", "OS type"), &app.system.os_type),
    key_value(
      tr(lang, "Distribuidor", "Distributor"),
      &app.system.distributor,
    ),
    key_value("ARGVUS", &app.argvus_version),
    key_value("GTK", &app.gtk_version),
    key_value(tr(lang, "Kernel", "Kernel"), &app.system.kernel),
    key_value(
      tr(lang, "Sistema de janelas", "Window system"),
      &app.system.window_system,
    ),
    key_value("CPU", &app.system.cpu),
    key_value(tr(lang, "Memória", "Memory"), &app.system.memory),
    key_value("GPU", &app.system.gpus),
  ];

  simple_doc(&rows, &app.theme, width, selected)
}

fn key_value(key: &str, value: &str) -> Row {
  Row::KeyValue {
    key: key.to_string(),
    value: value.to_string(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::App;

  #[test]
  fn system_doc_contains_expected_keys() {
    let app = App::test();
    let doc = doc(&app, 100, 0);
    let text = doc.lines.iter().map(|l| l.to_string()).collect::<String>();
    for key in ["ARGVUS", "GTK", "CPU", "GPU"] {
      assert!(text.contains(key), "missing {key}");
    }
  }
}
