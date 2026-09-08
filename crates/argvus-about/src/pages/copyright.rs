use crate::app::App;
use crate::i18n::tr;

use super::{Doc, Row, simple_doc};

const LICENSE_TEXT: &str = include_str!("../../../../LICENSE");

pub fn doc(app: &App, width: usize, selected: usize) -> Doc<'static> {
  let lang = app.lang;
  let rows = vec![
        Row::Section(
            tr(lang, "Direitos autorais", "Copyright").to_string(),
        ),
        Row::Para(
            tr(
                lang,
                "Copyright © 2025 William C. Canin. O ARGVUS e seus módulos são distribuídos sob a Licença Pública Geral GNU, versão 3 (GPLv3), conforme descrito abaixo.",
                "Copyright © 2025 William C. Canin. ARGVUS and its modules are distributed under the GNU General Public License, version 3 (GPLv3), as described below.",
            )
            .to_string(),
        ),
        Row::Spacer,
        Row::Para(
            tr(
                lang,
                "Este programa é software livre: você pode redistribuí-lo e/ou modificá-lo sob os termos da GPL, conforme publicado pela Free Software Foundation, seja a versão 3 da licença, ou (a seu critério) qualquer versão posterior.",
                "This program is free software: you can redistribute it and/or modify it under the terms of the GPL as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.",
            )
            .to_string(),
        ),
        Row::Para(
            tr(
                lang,
                "Este programa é distribuído na esperança de que seja útil, mas SEM NENHUMA GARANTIA; sem sequer a garantia implícita de COMERCIABILIDADE ou ADEQUAÇÃO A UM DETERMINADO FIM. Veja a GNU General Public License para mais detalhes.",
                "This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.",
            )
            .to_string(),
        ),
        Row::Para(
            tr(
                lang,
                "Você deve ter recebido uma cópia da GNU General Public License junto com este programa. Se não, veja <https://www.gnu.org/licenses/>.",
                "You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.",
            )
            .to_string(),
        ),
        Row::Spacer,
        Row::Section(tr(lang, "Texto completo da licença", "Full license text").to_string()),
        Row::Para(LICENSE_TEXT.to_string()),
    ];

  simple_doc(&rows, &app.theme, width, selected)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::app::App;

  #[test]
  fn copyright_embeds_license() {
    let app = App::test();
    let doc = doc(&app, 100, 0);
    let text = doc.lines.iter().map(|l| l.to_string()).collect::<String>();
    assert!(text.contains("GNU GENERAL PUBLIC LICENSE"));
  }
}
