use crate::app::App;
use crate::i18n::tr;

use super::{ARGVUS_URL, DONATE_URL, Doc, Row, simple_doc};

pub struct Module {
  pub name: &'static str,
  pub pt: &'static str,
  pub en: &'static str,
}

pub struct Group {
  pub pt: &'static str,
  pub en: &'static str,
  pub modules: &'static [Module],
}

pub fn doc(app: &App, width: usize, selected: usize) -> Doc<'static> {
  let lang = app.lang;
  let mut rows = vec![
        Row::Lead(
            tr(
                lang,
                "O ARGVUS é uma coleção modular de pacotes que juntos entregam um ambiente de desktop completo para Wayland e Hyprland.",
                "ARGVUS is a modular collection of packages that together provide a complete desktop environment for Wayland and Hyprland.",
            )
            .to_string(),
        ),
        Row::Spacer,
        kv_row(lang, "Versão", "Version", app.argvus_version.clone()),
        kv_row(lang, "Licença", "License", "GPL-3.0".to_string()),
        Row::Spacer,
    ];

  for group in groups() {
    rows.push(Row::Divider {
      label: Some(tr(lang, group.pt, group.en).to_string()),
    });
    for module in group.modules {
      rows.push(Row::Module {
        name: module.name.to_string(),
        description: tr(lang, module.pt, module.en).to_string(),
      });
    }
  }

  rows.push(Row::Spacer);
  rows.push(Row::Divider {
    label: Some(tr(lang, "Links", "Links").to_string()),
  });
  rows.push(Row::Link {
    label: ARGVUS_URL.to_string(),
    url: ARGVUS_URL.to_string(),
  });
  rows.push(Row::Link {
    label: tr(lang, "Apoiar o projeto", "Support the project").to_string(),
    url: DONATE_URL.to_string(),
  });

  simple_doc(&rows, &app.theme, width, selected)
}

fn kv_row<'a>(lang: crate::i18n::Lang, pt: &'a str, en: &'a str, value: String) -> Row {
  Row::KeyValue {
    key: format!("{:<8}", tr(lang, pt, en)),
    value,
  }
}

pub fn groups() -> [Group; 4] {
  [
    Group {
      pt: "Núcleo",
      en: "Core",
      modules: &[
        Module {
          name: "argvus-session",
          pt: "Ciclo de vida da sessão, targets e integração Hyprland.",
          en: "Session lifecycle, targets and Hyprland integration.",
        },
        Module {
          name: "argvus-hyprland",
          pt: "Configuracao Hyprland e scripts do shell ARGVUS.",
          en: "Hyprland configuration and ARGVUS shell scripts.",
        },
        Module {
          name: "argvus-portal",
          pt: "Portais Wayland, DBus e preferências de integração.",
          en: "Wayland portals, DBus and integration preferences.",
        },
      ],
    },
    Group {
      pt: "Interface e Desktop",
      en: "Shell and desktop",
      modules: &[
        Module {
          name: "argvus-launcher",
          pt: "Launcher Rofi, menus e temas.",
          en: "Rofi launcher, menus and themes.",
        },
        Module {
          name: "argvus-control-panel",
          pt: "Painel lateral Quickshell e seus temas.",
          en: "Quickshell sidebar control panel and its themes.",
        },
        Module {
          name: "argvus-appearance",
          pt: "Temas, fontes, wallpapers e integração visual.",
          en: "Themes, fonts, wallpapers and visual integration.",
        },
        Module {
          name: "argvus-taskbar-calendar",
          pt: "Calendário e popup integrado à taskbar.",
          en: "Calendar and taskbar popup integration.",
        },
        Module {
          name: "argvus-taskbar-storage",
          pt: "Módulo de dispositivos removíveis e armazenamento.",
          en: "Removable device and storage module.",
        },
        Module {
          name: "argvus-greeter",
          pt: "Tela gráfica de login do ARGVUS.",
          en: "ARGVUS graphical login screen.",
        },
        Module {
          name: "argvus-lock",
          pt: "Bloqueio de tela e temas do lock screen.",
          en: "Screen locking and lock screen themes.",
        },
      ],
    },
    Group {
      pt: "Configuração",
      en: "Configuration",
      modules: &[
        Module {
          name: "argvus-control-center",
          pt: "Configurações do ARGVUS, incluindo fontes e aplicativos padrão.",
          en: "ARGVUS control center, including fonts and default applications.",
        },
        Module {
          name: "argvus-about",
          pt: "Informações do sistema, créditos e licença do ARGVUS.",
          en: "System information, credits and ARGVUS license.",
        },
        Module {
          name: "argvus-accounts",
          pt: "Configurações de conta e usuário.",
          en: "Account and user settings.",
        },
        Module {
          name: "argvus-display",
          pt: "Gerenciamento de monitores e layouts.",
          en: "Monitor and layout management.",
        },
      ],
    },
    Group {
      pt: "Serviços e Energia",
      en: "Services and power",
      modules: &[
        Module {
          name: "argvus-network",
          pt: "NetworkManager, Wi-Fi e Bluetooth.",
          en: "NetworkManager, Wi-Fi and Bluetooth.",
        },
        Module {
          name: "argvus-power",
          pt: "Menu de energia, idle e ações de sessão.",
          en: "Power menu, idle and session actions.",
        },
      ],
    },
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

  #[test]
  fn every_group_is_represented_in_both_languages() {
    use crate::i18n::Lang;
    for lang in [Lang::Pt, Lang::En] {
      let mut app = App::test();
      app.lang = lang;
      let doc = doc(&app, 80, 0);
      let text = doc.lines.iter().map(|l| l.to_string()).collect::<String>();
      for group in groups() {
        let label = tr(lang, group.pt, group.en);
        assert!(
          text.contains(label),
          "missing group '{}' for {:?}",
          label,
          lang
        );
      }
    }
  }

  #[test]
  fn about_offers_site_and_support_links() {
    let doc = doc(&App::test(), 80, 0);
    assert!(doc.actions.iter().any(|a| a.url == ARGVUS_URL));
    assert!(doc.actions.iter().any(|a| a.url == DONATE_URL));
  }

  #[test]
  fn groups_cover_every_module_name() {
    let names = groups()
      .iter()
      .flat_map(|group| group.modules.iter())
      .map(|module| module.name)
      .collect::<Vec<_>>();
    assert_eq!(names.len(), 16);
    assert!(names.contains(&"argvus-about"));
    assert!(names.contains(&"argvus-taskbar-storage"));
  }
}
