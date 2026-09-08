use crate::app::App;
use crate::i18n::tr;

use super::{ARGVUS_URL, Doc, Row, WILLIAM_CANIN_URL, simple_doc};

pub fn doc(app: &App, width: usize, selected: usize) -> Doc<'static> {
  let lang = app.lang;
  let rows = vec![
        Row::Section(
            tr(lang, "Desenvolvedor principal", "Lead developer").to_string(),
        ),
        Row::Para(
            tr(
                lang,
                "O ARGVUS é idealizado e desenvolvido por William C. Canin.",
                "ARGVUS is designed and developed by William C. Canin.",
            )
            .to_string(),
        ),
        Row::Link {
            label: WILLIAM_CANIN_URL.to_string(),
            url: WILLIAM_CANIN_URL.to_string(),
        },
        Row::Spacer,
        Row::Section(tr(lang, "Projeto", "Project").to_string()),
        Row::Link {
            label: ARGVUS_URL.to_string(),
            url: ARGVUS_URL.to_string(),
        },
        Row::Spacer,
        Row::Section(tr(lang, "Contribuições", "Contributions").to_string()),
        Row::Para(
            tr(
                lang,
                "Agradecemos a toda a comunidade ARGVUS e a todos os colaboradores que contribuíram com código, traduções, relatórios de bug, documentação e ideias nos repositórios oficiais de cada módulo.",
                "We thank the entire ARGVUS community and all contributors who helped with code, translations, bug reports, documentation and ideas in the official repositories of each module.",
            )
            .to_string(),
        ),
        Row::Spacer,
        Row::Section(tr(lang, "Tecnologias", "Technologies").to_string()),
        Row::Para(
            tr(
                lang,
                "Construído sobre Hyprland, Wayland, Quickshell, Rust, Vala e as bibliotecas e ferramentas livres que tornam este desktop possível.",
                "Built on Hyprland, Wayland, Quickshell, Rust, Vala and the free libraries and tools that make this desktop possible.",
            )
            .to_string(),
        ),
    ];

  simple_doc(&rows, &app.theme, width, selected)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::App;

  #[test]
  fn credits_have_author_and_project_links() {
    let app = App::test();
    let doc = doc(&app, 100, 0);
    assert!(doc.actions.iter().any(|a| a.url == WILLIAM_CANIN_URL));
    assert!(doc.actions.iter().any(|a| a.url == ARGVUS_URL));
  }
}
