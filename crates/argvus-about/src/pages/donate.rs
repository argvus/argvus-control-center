use crate::app::App;
use crate::i18n::tr;

use super::{DONATE_URL, Doc, Row, simple_doc};

pub fn doc(app: &App, width: usize, selected: usize) -> Doc<'static> {
  let lang = app.lang;
  let rows = vec![
        Row::Section(tr(lang, "Apoie o desenvolvimento", "Support the development").to_string()),
        Row::Para(
            tr(
                lang,
                "O ARGVUS é um projeto de software livre e gratuito. Se você gosta do desktop, considere apoiar o desenvolvimento com uma doação. Todo o apoio ajuda a manter o projeto vivo, com novas funcionalidades, correções e melhorias.",
                "ARGVUS is a free and open source project. If you like the desktop, consider supporting development with a donation. Every bit of support helps keep the project alive with new features, fixes and improvements.",
            )
            .to_string(),
        ),
        Row::Spacer,
        Row::Link {
            label: DONATE_URL.to_string(),
            url: DONATE_URL.to_string(),
        },
        Row::Spacer,
        Row::Para(
            tr(
                lang,
                "Obrigado pelo carinho e pelo apoio!",
                "Thank you for your care and support!",
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
  fn donate_has_link_to_support() {
    let app = App::test();
    let doc = doc(&app, 100, 0);
    assert!(doc.actions.iter().any(|a| a.url == DONATE_URL));
  }
}
