use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::app::{App, Tab};
use crate::theme::Theme;

pub mod about;
pub mod copyright;
pub mod credits;
pub mod donate;
pub mod system;

pub const DONATE_URL: &str = "https://argvus.github.io/#support";
pub const ARGVUS_URL: &str = "https://argvus.github.io";
pub const WILLIAM_CANIN_URL: &str = "https://williamcanin.github.io";

pub fn doc_for(app: &App, width: usize, selected: usize) -> Doc<'static> {
  match app.active_tab {
    Tab::System => system::doc(app, width, selected),
    Tab::About => about::doc(app, width, selected),
    Tab::Donate => donate::doc(app, width, selected),
    Tab::Credits => credits::doc(app, width, selected),
    Tab::Copyright => copyright::doc(app, width, selected),
  }
}

#[derive(Debug, Clone)]
pub struct Action {
  pub line: usize,
  pub url: String,
}

#[derive(Debug, Clone, Default)]
pub struct Doc<'a> {
  pub lines: Vec<Line<'a>>,
  pub actions: Vec<Action>,
}

impl Doc<'_> {
  pub fn height(&self) -> usize {
    self.lines.len()
  }
}

pub enum Row {
  Spacer,
  Section(String),
  Sub(String),
  Para(String),
  KeyValue { key: String, value: String },
  Link { label: String, url: String },
}

pub fn simple_doc(rows: &[Row], theme: &Theme, width: usize, selected: usize) -> Doc<'static> {
  let width = width.max(10);
  let mut doc = Doc::default();
  let mut link_index = 0;

  for row in rows {
    match row {
      Row::Spacer => doc.lines.push(Line::from("")),
      Row::Section(title) => doc.lines.push(Line::styled(
        title.clone(),
        Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
      )),
      Row::Sub(title) => doc.lines.push(Line::styled(
        title.clone(),
        Style::new()
          .fg(theme.foreground)
          .add_modifier(Modifier::BOLD),
      )),
      Row::Para(text) => {
        for part in wrap_paragraphs(text, width) {
          doc
            .lines
            .push(Line::styled(part, Style::new().fg(theme.foreground)));
        }
      }
      Row::KeyValue { key, value } => {
        append_key_value(&mut doc, key, value, theme, width);
      }
      Row::Link { label, url } => {
        let is_selected = link_index == selected;
        doc
          .lines
          .push(line_for_link(label, url, theme, is_selected, width));
        doc.actions.push(Action {
          line: doc.lines.len() - 1,
          url: url.clone(),
        });
        link_index += 1;
      }
    }
  }

  doc
}

fn append_key_value(doc: &mut Doc, key: &str, value: &str, theme: &Theme, width: usize) {
  let key_span = Span::styled(key.to_string(), Style::new().fg(theme.muted));
  let separator = Span::raw("  ");
  let indent = key.chars().count().saturating_add(2);

  let mut first = true;
  for part in value.lines() {
    let joined_width = indent.saturating_add(part.chars().count());
    if joined_width > width {
      for wrapped in wrap_paragraphs(part, width.saturating_sub(indent)) {
        let line = if first {
          Line::from(vec![
            key_span.clone(),
            separator.clone(),
            Span::styled(wrapped, Style::new().fg(theme.foreground)),
          ])
        } else {
          continuation_line(&wrapped, indent, theme)
        };
        doc.lines.push(line);
        first = false;
      }
    } else {
      let line = if first {
        Line::from(vec![
          key_span.clone(),
          separator.clone(),
          Span::styled(part.to_string(), Style::new().fg(theme.foreground)),
        ])
      } else {
        continuation_line(part, indent, theme)
      };
      doc.lines.push(line);
      first = false;
    }
  }
  if first {
    let line = Line::from(vec![
      key_span,
      separator,
      Span::styled("", Style::new().fg(theme.foreground)),
    ]);
    doc.lines.push(line);
  }
}

fn continuation_line(text: &str, indent: usize, theme: &Theme) -> Line<'static> {
  Line::from(vec![
    Span::styled(" ".repeat(indent), Style::new().fg(theme.muted)),
    Span::styled(text.to_string(), Style::new().fg(theme.foreground)),
  ])
}

fn line_for_link(
  label: &str,
  _url: &str,
  theme: &Theme,
  selected: bool,
  width: usize,
) -> Line<'static> {
  let mut text = label.to_string();
  if text.chars().count() > width {
    let cut: String = text.chars().take(width.saturating_sub(3)).collect();
    text = format!("{cut}...");
  }
  let style = if selected {
    Style::new()
      .fg(theme.selected_foreground)
      .bg(theme.selected_background)
      .add_modifier(Modifier::BOLD)
  } else {
    Style::new().fg(theme.link)
  };
  Line::styled(text, style)
}

fn wrap_paragraphs(text: &str, width: usize) -> Vec<String> {
  let width = width.max(1);
  text
    .split('\n')
    .flat_map(|paragraph| {
      if paragraph.trim().is_empty() {
        vec![String::new()]
      } else {
        wrap_words(paragraph, width)
      }
    })
    .collect()
}

fn wrap_words(paragraph: &str, width: usize) -> Vec<String> {
  let mut lines = Vec::new();
  let mut current = String::new();
  for word in paragraph.split_whitespace() {
    if current.is_empty() {
      current = word.to_string();
    } else if current.chars().count() + 1 + word.chars().count() <= width {
      current.push(' ');
      current.push_str(word);
    } else {
      lines.push(std::mem::take(&mut current));
      current = word.to_string();
    }
  }
  if !current.is_empty() {
    lines.push(current);
  }
  lines
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn wraps_long_paragraphs() {
    let lines = wrap_paragraphs("aaa bbb ccc ddd", 7);
    assert!(lines.iter().all(|line| line.chars().count() <= 7));
    assert!(lines.join(" ") == "aaa bbb ccc ddd");
  }

  #[test]
  fn preserves_paragraph_breaks() {
    let lines = wrap_paragraphs("one\n\ntwo", 80);
    assert_eq!(
      lines,
      vec!["one".to_string(), String::new(), "two".to_string()]
    );
  }

  #[test]
  fn link_rows_become_actions() {
    let rows = vec![
      Row::Spacer,
      Row::Link {
        label: "site".into(),
        url: "https://argvus.github.io".into(),
      },
      Row::Para("body".into()),
      Row::Link {
        label: "donate".into(),
        url: "https://argvus.github.io/#support".into(),
      },
    ];
    let doc = simple_doc(&rows, &Theme::load(), 80, 1);
    assert_eq!(doc.actions.len(), 2);
    assert_eq!(doc.actions[0].line, 1);
    assert_eq!(doc.actions[1].line, 3);
    assert_eq!(doc.actions[1].url, "https://argvus.github.io/#support");
  }
}
