use crate::{backend, model::*};
use argvus_control_center_core::{
  capabilities::Capabilities,
  config::AppConfig,
  jobs::{JobHandle, JobManager, JobState},
};
use argvus_i18n::{Lang, tr};
use argvus_theme::Theme;
use argvus_tui::{
  components::{StatusKind, StatusMessage},
  page::{Selection, list, readonly, shell, status},
};
use crossterm::event::KeyCode;
use ratatui::{Frame, text::Line};

pub struct DiagnosticsApp {
  pub page: DiagnosticPage,
  checks: Vec<DiagnosticCheck>,
  selected: Selection,
  job: Option<JobHandle<Vec<DiagnosticCheck>>>,
  jobs: JobManager,
  cap: Capabilities,
  pub status: Option<StatusMessage>,
  pub lang: Lang,
  pub theme: Theme,
}
impl DiagnosticsApp {
  pub fn new(lang: Lang, theme: Theme, cap: Capabilities) -> Self {
    let mut a = Self {
      page: DiagnosticPage::Home,
      checks: Vec::new(),
      selected: Selection::default(),
      job: None,
      jobs: JobManager::default(),
      cap,
      status: None,
      lang,
      theme,
    };
    a.refresh();
    a
  }
  pub fn reload(&mut self) {
    self.refresh()
  }
  pub fn refresh(&mut self) {
    if self.job.is_some() {
      return;
    }
    let cap = self.cap.clone();
    self.job = Some(
      self
        .jobs
        .spawn(move |_| Ok::<_, String>(backend::evaluate(&backend::collect(&cap), &cap))),
    );
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(self.lang, "Executando verificações...", "Running checks...").into(),
    });
  }
  pub fn poll(&mut self) -> bool {
    let Some(job) = &self.job else {
      return false;
    };
    if let JobState::Finished(result) = job.try_state() {
      self.job = None;
      match result {
        Ok(checks) => {
          self.checks = checks;
          self.selected.normalize(self.len());
          self.status = Some(completion_status(self.lang, &self.checks));
        }
        Err(error) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Error,
            text: error,
          });
        }
      }
      true
    } else {
      false
    }
  }
  pub fn busy(&self) -> bool {
    self.job.is_some()
  }
  fn len(&self) -> usize {
    match self.page {
      DiagnosticPage::Home => self.dashboard_rows().len(),
      DiagnosticPage::Summary | DiagnosticPage::Detail(_) => 1,
      _ => self.category_checks().count(),
    }
  }
  fn category(&self) -> &str {
    match self.page {
      DiagnosticPage::Services => "services",
      DiagnosticPage::Boot => "boot",
      DiagnosticPage::Graphics => "graphics",
      DiagnosticPage::Network => "network",
      DiagnosticPage::Audio => "audio",
      DiagnosticPage::Bluetooth => "bluetooth",
      DiagnosticPage::Storage => "storage",
      DiagnosticPage::Packages => "packages",
      DiagnosticPage::Argvus => "argvus",
      _ => "system",
    }
  }
  fn category_checks(&self) -> impl Iterator<Item = (usize, &DiagnosticCheck)> {
    let category = self.category();
    self
      .checks
      .iter()
      .enumerate()
      .filter(move |(_, check)| check.category == category)
  }
  fn detail_index(&self) -> usize {
    match self.page {
      DiagnosticPage::Detail(index) => index,
      _ => 0,
    }
  }
  fn back_page(&self) -> DiagnosticPage {
    match self.page {
      DiagnosticPage::Detail(index) => self
        .checks
        .get(index)
        .and_then(|check| page_for_category(&check.category))
        .unwrap_or(DiagnosticPage::Home),
      DiagnosticPage::Summary => DiagnosticPage::Home,
      _ => DiagnosticPage::Home,
    }
  }
  pub fn handle(&mut self, key: KeyCode) -> bool {
    if matches!(key, KeyCode::Esc | KeyCode::Left) {
      if self.page == DiagnosticPage::Home {
        return true;
      }
      self.page = self.back_page();
      self.selected.index = 0;
      return false;
    }
    if self.busy() {
      return false;
    }
    if key == KeyCode::Char('r') {
      self.refresh();
      return false;
    }
    self.selected.handle(key, self.len(), 8);
    if matches!(key, KeyCode::Enter | KeyCode::Right) {
      self.open_selected();
    }
    false
  }
  fn open_selected(&mut self) {
    match self.page {
      DiagnosticPage::Home => {
        self.page = match self.selected.index {
          0 => DiagnosticPage::Summary,
          1 => DiagnosticPage::Services,
          2 => DiagnosticPage::Boot,
          3 => DiagnosticPage::Graphics,
          4 => DiagnosticPage::Network,
          5 => DiagnosticPage::Audio,
          6 => DiagnosticPage::Bluetooth,
          7 => DiagnosticPage::Storage,
          8 => DiagnosticPage::Packages,
          _ => DiagnosticPage::Argvus,
        };
        self.selected.index = 0;
      }
      DiagnosticPage::Services
      | DiagnosticPage::Boot
      | DiagnosticPage::Graphics
      | DiagnosticPage::Network
      | DiagnosticPage::Audio
      | DiagnosticPage::Bluetooth
      | DiagnosticPage::Storage
      | DiagnosticPage::Packages
      | DiagnosticPage::Argvus => {
        let index = self
          .category_checks()
          .nth(self.selected.index)
          .map(|(index, _)| index);
        if let Some(index) = index {
          self.page = DiagnosticPage::Detail(index);
          self.selected.index = 0;
        }
      }
      _ => {}
    }
  }
  fn breadcrumb(&self) -> String {
    let root = tr(self.lang, "Diagnóstico", "Diagnostics");
    match self.page {
      DiagnosticPage::Home => root.into(),
      DiagnosticPage::Summary => {
        format!("{root} > {}", tr(self.lang, "Resumo", "Summary"))
      }
      DiagnosticPage::Detail(index) => format!(
        "{root} > {}",
        self
          .checks
          .get(index)
          .map(|check| category_label(self.lang, &check.category))
          .unwrap_or_else(|| tr(self.lang, "Detalhes", "Details"))
      ),
      _ => format!("{root} > {}", category_label(self.lang, self.category())),
    }
  }
  fn footer_hints(&self) -> &'static str {
    match self.page {
      DiagnosticPage::Home => tr(
        self.lang,
        "↑/↓ Navegar   →/Enter Abrir   r Atualizar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   →/Enter Open   r Refresh   ←/Esc Back   ? Help",
      ),
      DiagnosticPage::Summary | DiagnosticPage::Detail(_) => tr(
        self.lang,
        "r Atualizar   ←/Esc Voltar   ? Ajuda",
        "r Refresh   ←/Esc Back   ? Help",
      ),
      _ => tr(
        self.lang,
        "↑/↓ Navegar   →/Enter Detalhes   r Atualizar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   →/Enter Details   r Refresh   ←/Esc Back   ? Help",
      ),
    }
  }
  fn dashboard_rows(&self) -> Vec<String> {
    let all: Vec<&DiagnosticCheck> = self.checks.iter().collect();
    let summary = format!(
      "{} {}  ·  {}",
      AppConfig::icon("💻"),
      tr(self.lang, "Resumo", "Summary"),
      self.entry_status(&all)
    );
    vec![
      summary,
      self.dashboard_entry("services", "⚙️", tr(self.lang, "Serviços", "Services")),
      self.dashboard_entry(
        "boot",
        "🧠",
        tr(self.lang, "Kernel & Boot", "Kernel & Boot"),
      ),
      self.dashboard_entry("graphics", "🎮", tr(self.lang, "Gráficos", "Graphics")),
      self.dashboard_entry("network", "🌐", tr(self.lang, "Rede", "Network")),
      self.dashboard_entry("audio", "🔊", tr(self.lang, "Áudio", "Audio")),
      self.dashboard_entry("bluetooth", "🔗", tr(self.lang, "Bluetooth", "Bluetooth")),
      self.dashboard_entry("storage", "💽", tr(self.lang, "Armazenamento", "Storage")),
      self.dashboard_entry("packages", "📦", tr(self.lang, "Pacotes", "Packages")),
      self.dashboard_entry("argvus", "⭐", tr(self.lang, "ARGVUS", "ARGVUS")),
    ]
  }
  fn dashboard_entry(&self, category: &str, icon: &str, label: &'static str) -> String {
    let checks: Vec<&DiagnosticCheck> = self
      .checks
      .iter()
      .filter(|check| check.category == category)
      .collect();
    format!(
      "{} {}  ·  {}",
      AppConfig::icon(icon),
      label,
      self.entry_status(&checks)
    )
  }
  fn entry_status(&self, checks: &[&DiagnosticCheck]) -> String {
    if checks.is_empty() {
      tr(self.lang, "Sem dados", "No data").into()
    } else {
      format!(
        "{} · {} {}",
        self.preview(checks),
        checks.len(),
        tr(self.lang, "verificações", "checks")
      )
    }
  }
  fn preview(&self, checks: &[&DiagnosticCheck]) -> String {
    let errors = checks
      .iter()
      .filter(|check| check.severity == Severity::Error)
      .count();
    let warnings = checks
      .iter()
      .filter(|check| check.severity == Severity::Warning)
      .count();
    if checks.is_empty() {
      tr(self.lang, "Sem dados", "No data").into()
    } else if errors > 0 {
      format!("{} {}", errors, tr(self.lang, "erro(s)", "error(s)"))
    } else if warnings > 0 {
      format!("{} {}", warnings, tr(self.lang, "aviso(s)", "warning(s)"))
    } else {
      tr(self.lang, "OK", "OK").into()
    }
  }
  fn category_rows(&self) -> Vec<String> {
    self
      .category_checks()
      .map(|(_, check)| {
        format!(
          "{} {} — {}",
          severity_glyph(check.severity),
          check.title,
          check.summary
        )
      })
      .collect()
  }
  fn summary_rows(&self) -> Vec<String> {
    let errors = self
      .checks
      .iter()
      .filter(|check| check.severity == Severity::Error)
      .count();
    let warnings = self
      .checks
      .iter()
      .filter(|check| check.severity == Severity::Warning)
      .count();
    let unknown = self
      .checks
      .iter()
      .filter(|check| check.severity == Severity::Unknown)
      .count();
    let mut rows = vec![
      format!(
        " {} {}",
        AppConfig::icon("💻"),
        tr(self.lang, "DIAGNÓSTICO", "DIAGNOSTICS")
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Verificações:", "Checks:"),
        self.checks.len()
      ),
      format!("   {:<14} {}", tr(self.lang, "Erros:", "Errors:"), errors),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Avisos:", "Warnings:"),
        warnings
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Indefinidos:", "Unknown:"),
        unknown
      ),
    ];
    let system: Vec<&DiagnosticCheck> = self
      .checks
      .iter()
      .filter(|check| check.category == "system")
      .collect();
    if !system.is_empty() {
      rows.push(String::new());
      rows.push(format!(
        " {} {}",
        AppConfig::icon("⚙️"),
        tr(self.lang, "SISTEMA", "SYSTEM")
      ));
      rows.extend(system.into_iter().map(|check| {
        format!(
          "   {} {} — {}",
          severity_glyph(check.severity),
          check.title,
          check.summary
        )
      }));
    }
    rows
  }
  fn detail_rows(&self, index: usize) -> Vec<String> {
    let Some(check) = self.checks.get(index) else {
      return vec![tr(self.lang, "Verificação não encontrada", "Check not found").into()];
    };
    vec![
      format!(
        " {} {}",
        AppConfig::icon(category_icon(&check.category)),
        category_label(self.lang, &check.category).to_uppercase()
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Categoria:", "Category:"),
        category_label(self.lang, &check.category)
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Severidade:", "Severity:"),
        severity_label(self.lang, check.severity)
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Título:", "Title:"),
        check.title
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Resumo:", "Summary:"),
        check.summary
      ),
      String::new(),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Detalhes:", "Details:"),
        check.details
      ),
      String::new(),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Recomendação:", "Remediation:"),
        check.remediation_hint.as_deref().unwrap_or_else(|| tr(
          self.lang,
          "Nenhuma ação automática",
          "No automatic action"
        ))
      ),
    ]
  }
  pub fn draw(&mut self, frame: &mut Frame) {
    let body = shell(
      frame,
      frame.area(),
      &self.theme,
      &self.breadcrumb(),
      self.footer_hints(),
    );
    match self.page {
      DiagnosticPage::Home => {
        let rows = self.dashboard_rows();
        list(
          frame,
          body,
          &self.theme,
          &rows,
          self.selected.index.min(rows.len().saturating_sub(1)),
        );
      }
      DiagnosticPage::Summary => {
        let rows = self
          .summary_rows()
          .into_iter()
          .map(Line::from)
          .collect::<Vec<_>>();
        readonly(frame, body, &self.theme, &rows);
      }
      DiagnosticPage::Detail(_) => {
        let rows = self
          .detail_rows(self.detail_index())
          .into_iter()
          .map(Line::from)
          .collect::<Vec<_>>();
        readonly(frame, body, &self.theme, &rows);
      }
      _ => {
        let rows = self.category_rows();
        list(
          frame,
          body,
          &self.theme,
          &rows,
          self.selected.index.min(rows.len().saturating_sub(1)),
        );
      }
    }
    if let Some(message) = &self.status {
      status(frame, body, &self.theme, message);
    }
  }
}

fn page_for_category(category: &str) -> Option<DiagnosticPage> {
  match category {
    "services" => Some(DiagnosticPage::Services),
    "boot" => Some(DiagnosticPage::Boot),
    "graphics" => Some(DiagnosticPage::Graphics),
    "network" => Some(DiagnosticPage::Network),
    "audio" => Some(DiagnosticPage::Audio),
    "bluetooth" => Some(DiagnosticPage::Bluetooth),
    "storage" => Some(DiagnosticPage::Storage),
    "packages" => Some(DiagnosticPage::Packages),
    "argvus" => Some(DiagnosticPage::Argvus),
    _ => None,
  }
}
fn severity_glyph(severity: Severity) -> &'static str {
  match severity {
    Severity::Error => "✖",
    Severity::Warning => "▲",
    Severity::Unknown => "?",
    Severity::Info => "•",
    Severity::Ok => "✔",
  }
}
fn severity_label(lang: Lang, severity: Severity) -> &'static str {
  match severity {
    Severity::Error => tr(lang, "Erro", "Error"),
    Severity::Warning => tr(lang, "Aviso", "Warning"),
    Severity::Unknown => tr(lang, "Indefinido", "Unknown"),
    Severity::Info => tr(lang, "Info", "Info"),
    Severity::Ok => tr(lang, "OK", "OK"),
  }
}
fn category_label(lang: Lang, category: &str) -> &'static str {
  match category {
    "services" => tr(lang, "Serviços", "Services"),
    "boot" => tr(lang, "Kernel & Boot", "Kernel & Boot"),
    "graphics" => tr(lang, "Gráficos", "Graphics"),
    "network" => tr(lang, "Rede", "Network"),
    "audio" => tr(lang, "Áudio", "Audio"),
    "bluetooth" => tr(lang, "Bluetooth", "Bluetooth"),
    "storage" => tr(lang, "Armazenamento", "Storage"),
    "packages" => tr(lang, "Pacotes", "Packages"),
    "argvus" => tr(lang, "ARGVUS", "ARGVUS"),
    _ => tr(lang, "Sistema", "System"),
  }
}
fn category_icon(category: &str) -> &'static str {
  match category {
    "services" => "⚙️",
    "boot" => "🧠",
    "graphics" => "🎮",
    "network" => "🌐",
    "audio" => "🔊",
    "bluetooth" => "🔗",
    "storage" => "💽",
    "packages" => "📦",
    "argvus" => "⭐",
    _ => "💻",
  }
}
fn completion_status(lang: Lang, checks: &[DiagnosticCheck]) -> StatusMessage {
  let errors = checks
    .iter()
    .filter(|check| check.severity == Severity::Error)
    .count();
  let warnings = checks
    .iter()
    .filter(|check| check.severity == Severity::Warning)
    .count();
  if errors > 0 {
    StatusMessage {
      kind: StatusKind::Error,
      text: format!(
        "{} {}",
        errors,
        tr(lang, "erro(s) encontrado(s)", "error(s) found")
      ),
    }
  } else if warnings > 0 {
    StatusMessage {
      kind: StatusKind::Warning,
      text: format!(
        "{} {}",
        warnings,
        tr(lang, "aviso(s) encontrado(s)", "warning(s) found")
      ),
    }
  } else {
    StatusMessage {
      kind: StatusKind::Success,
      text: tr(
        lang,
        "Diagnóstico concluído: todos os sistemas estão OK",
        "Diagnostics completed: all systems OK",
      )
      .into(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use ratatui::{Terminal, backend::TestBackend};

  fn app() -> DiagnosticsApp {
    let mut app = DiagnosticsApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.job = None;
    app.checks = vec![
      DiagnosticCheck {
        id: "system.failed_units".into(),
        category: "services".into(),
        severity: Severity::Warning,
        title: "Failed system units".into(),
        summary: "2 system unit(s) failed".into(),
        details: "The system manager reports failed units.".into(),
        remediation_hint: Some("Open Services > Failed".into()),
        ..Default::default()
      },
      DiagnosticCheck {
        id: "graphics.opengl".into(),
        category: "graphics".into(),
        severity: Severity::Ok,
        title: "OpenGL renderer".into(),
        summary: "Mesa hardware renderer".into(),
        details: "OpenGL reports a hardware renderer.".into(),
        ..Default::default()
      },
      DiagnosticCheck {
        id: "graphics.environment".into(),
        category: "system".into(),
        severity: Severity::Info,
        title: "Graphics environment".into(),
        summary: "Hardware graphics checks are provided by Hardware".into(),
        ..Default::default()
      },
    ];
    app
  }

  #[test]
  fn diagnostics_home_dashboard_lists_categories_with_previews() {
    let app = app();
    let rows = app.dashboard_rows();
    assert_eq!(rows.len(), 10);
    assert!(
      rows[0].contains("Resumo") || rows[0].contains("Summary"),
      "first row must summarize"
    );
    assert!(rows[1].contains("Serviços") || rows[1].contains("Services"));
    assert!(rows[1].contains("warning"));
    assert!(rows[3].contains("Gráficos") || rows[3].contains("Graphics"));
    assert!(rows[3].contains("OK"));
    assert!(
      rows[9].contains("ARGVUS"),
      "last row must cover the ARGVUS category"
    );
  }

  #[test]
  fn diagnostics_category_rows_render_severity_glyphs() {
    let mut app = app();
    app.page = DiagnosticPage::Services;
    let rows = app.category_rows();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].starts_with("▲"));
    assert!(rows[0].contains("Failed system units"));
    assert!(rows[0].contains("2 system unit(s) failed"));
  }

  #[test]
  fn diagnostics_detail_rows_render_aligned_sections() {
    let app = app();
    let rows = app.detail_rows(0);
    assert!(
      rows[0].contains("SERVICES"),
      "first row is a section header"
    );
    assert!(rows.iter().any(|row| row.contains("Severity:")));
    assert!(
      rows
        .iter()
        .any(|row| row.contains("Open Services > Failed"))
    );
  }

  #[test]
  fn diagnostics_summary_rows_include_counters_and_system_checks() {
    let app = app();
    let rows = app.summary_rows();
    assert!(rows[0].contains("DIAGNOSTICS"));
    assert!(rows.iter().any(|row| row.contains("Checks:")));
    assert!(rows.iter().any(|row| row.contains("SYSTEM")));
    assert!(rows.iter().any(|row| row.contains("Graphics environment")));
  }

  #[test]
  fn diagnostics_home_opens_summary_and_escapes_back() {
    let mut app = app();
    app.handle(KeyCode::Enter);
    assert_eq!(app.page, DiagnosticPage::Summary);
    app.handle(KeyCode::Esc);
    assert_eq!(app.page, DiagnosticPage::Home);
    assert!(
      app.handle(KeyCode::Esc),
      "second back returns to global home"
    );
  }

  #[test]
  fn diagnostics_category_details_back_out_one_level() {
    let mut app = app();
    app.page = DiagnosticPage::Services;
    app.handle(KeyCode::Enter);
    assert_eq!(app.page, DiagnosticPage::Detail(0));
    app.handle(KeyCode::Esc);
    assert_eq!(app.page, DiagnosticPage::Services);
    app.handle(KeyCode::Esc);
    assert_eq!(app.page, DiagnosticPage::Home);
  }

  #[test]
  fn diagnostics_uses_shared_chrome_and_contextual_footer() {
    let mut app = app();
    let mut terminal = Terminal::new(TestBackend::new(90, 25)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    let text = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(text.contains("ARGVUS"));
    assert!(text.contains("Diagnostics"));
    assert!(text.contains(app.theme.name.as_str()));
    assert!(text.contains("Enter") && text.contains("Back"));

    app.page = DiagnosticPage::Services;
    let mut terminal = Terminal::new(TestBackend::new(90, 25)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    let text = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(text.contains("Services"));
    assert!(text.contains("Details"));
  }

  #[test]
  fn diagnostics_detail_footer_and_breadcrumb_follow_the_parent_category() {
    let app = app();
    assert_eq!(app.breadcrumb(), "Diagnostics");
    let mut detail = app;
    detail.page = DiagnosticPage::Detail(2);
    assert!(detail.breadcrumb().ends_with("System"));
    assert!(detail.footer_hints().contains("Back"));
  }

  #[test]
  fn completion_status_reflects_worst_severity() {
    assert_eq!(completion_status(Lang::En, &[]).kind, StatusKind::Success);
    let mut checks = app();
    checks.checks = vec![DiagnosticCheck {
      id: "storage.usage./".into(),
      category: "storage".into(),
      severity: Severity::Error,
      ..Default::default()
    }];
    assert_eq!(
      completion_status(Lang::En, &checks.checks).kind,
      StatusKind::Error
    );
    let mut warnings = app();
    warnings.checks = vec![DiagnosticCheck {
      id: "packages.lock".into(),
      category: "packages".into(),
      severity: Severity::Warning,
      ..Default::default()
    }];
    assert_eq!(
      completion_status(Lang::En, &warnings.checks).kind,
      StatusKind::Warning
    );
  }
}
