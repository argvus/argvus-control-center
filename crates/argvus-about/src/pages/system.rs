use crate::app::App;
use crate::i18n::tr;

use super::{Doc, Row, simple_doc};

pub fn doc(app: &App, width: usize, selected: usize) -> Doc<'static> {
  let lang = app.lang;
  let rows = vec![
    key_value(tr(lang, "control_center.device"), &app.system.hostname),
    key_value(tr(lang, "control_center.os_name"), &app.system.os_name),
    key_value(tr(lang, "control_center.os_type"), &app.system.os_type),
    key_value(
      tr(lang, "control_center.distributor"),
      &app.system.distributor,
    ),
    key_value("ARGVUS", &app.argvus_version),
    key_value("GTK", &app.gtk_version),
    key_value(tr(lang, "control_center.kernel"), &app.system.kernel),
    key_value(
      tr(lang, "control_center.window_system"),
      &app.system.window_system,
    ),
    key_value("CPU", &app.system.cpu),
    key_value(tr(lang, "control_center.memory"), &app.system.memory),
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
