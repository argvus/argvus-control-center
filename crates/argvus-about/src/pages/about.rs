use crate::app::App;
use crate::i18n::tr;

use super::{ARGVUS_URL, Doc, Row, simple_doc};

pub fn doc(app: &App, width: usize, selected: usize) -> Doc<'static> {
  let lang = app.lang;
  let mut rows = vec![
        Row::Para(
            tr(
                lang,
                "O ARGVUS é uma coleção modular de pacotes que juntos entregam um ambiente de desktop completo para Wayland e Hyprland.",
                "ARGVUS is a modular collection of packages that together provide a complete desktop environment for Wayland and Hyprland.",
            )
            .to_string(),
        ),
        Row::Section(tr(lang, "Site oficial", "Official website").to_string()),
        Row::Link {
            label: ARGVUS_URL.to_string(),
            url: ARGVUS_URL.to_string(),
        },
        Row::Spacer,
    ];

  for (name, pt, en) in modules() {
    rows.push(Row::Sub(name.to_string()));
    rows.push(Row::Para(tr(lang, pt, en).to_string()));
  }

  simple_doc(&rows, &app.theme, width, selected)
}

pub fn modules() -> [(&'static str, &'static str, &'static str); 16] {
  [
    (
      "argvus-session",
      "Ciclo de vida da sessão, targets e integração Hyprland.",
      "Session lifecycle, targets and Hyprland integration.",
    ),
    (
      "argvus-hyprland",
      "Configuracao Hyprland e scripts do shell ARGVUS.",
      "Hyprland configuration and ARGVUS shell scripts.",
    ),
    (
      "argvus-launcher",
      "Launcher Rofi, menus e temas.",
      "Rofi launcher, menus and themes.",
    ),
    (
      "argvus-control-panel",
      "Painel lateral Quickshell e seus temas.",
      "Quickshell sidebar control panel and its themes.",
    ),
    (
      "argvus-appearance",
      "Temas, fontes, wallpapers e integração visual.",
      "Themes, fonts, wallpapers and visual integration.",
    ),
    (
      "argvus-control-center",
      "Configurações do ARGVUS, incluindo fontes e aplicativos padrão.",
      "ARGVUS control center, including fonts and default applications.",
    ),
    (
      "argvus-about",
      "Informações do sistema, créditos e licença do ARGVUS.",
      "System information, credits and ARGVUS license.",
    ),
    (
      "argvus-calendar",
      "Calendário e popup integrado à taskbar.",
      "Calendar and taskbar popup integration.",
    ),
    (
      "argvus-storage",
      "Módulo de dispositivos removíveis e armazenamento.",
      "Removable device and storage module.",
    ),
    (
      "argvus-greeter",
      "Tela gráfica de login do ARGVUS.",
      "ARGVUS graphical login screen.",
    ),
    (
      "argvus-accounts",
      "Configurações de conta e usuário.",
      "Account and user settings.",
    ),
    (
      "argvus-display",
      "Gerenciamento de monitores e layouts.",
      "Monitor and layout management.",
    ),
    (
      "argvus-network",
      "NetworkManager, Wi-Fi e Bluetooth.",
      "NetworkManager, Wi-Fi and Bluetooth.",
    ),
    (
      "argvus-power",
      "Menu de energia, idle e ações de sessão.",
      "Power menu, idle and session actions.",
    ),
    (
      "argvus-lock",
      "Bloqueio de tela e temas do lock screen.",
      "Screen locking and lock screen themes.",
    ),
    (
      "argvus-portal",
      "Portais Wayland, DBus e preferências de integração.",
      "Wayland portals, DBus and integration preferences.",
    ),
  ]
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::App;

  #[test]
  fn about_doc_mentions_all_modules() {
    use crate::app::Tab;
    let mut app = App::test();
    app.active_tab = Tab::About;
    let doc = doc(&app, 100, 0);
    assert!(doc.actions.iter().any(|a| a.url == ARGVUS_URL));
    let text = doc.lines.iter().map(|l| l.to_string()).collect::<String>();
    assert!(text.contains("argvus-session"));
    assert!(text.contains("argvus-portal"));
  }
}
