use crate::{
  backend::{PackageBackend, parse_reflector_countries, reflector_args},
  model::{
    AurPackage, CachePackage, HistoryEntry, Mirror, Package, PackageDashboard, PackageDetails,
    PackagesPage, ReflectorOptions, TransactionPlan, Update,
  },
};
use argvus_control_center_core::{
  capabilities::Capabilities,
  config::AppConfig,
  jobs::{JobHandle, JobManager, JobState},
  privileged::{PrivilegedRequest, SystemSettingsOperation},
  process::{LiveProcess, ProcessRequest, ProcessRunner, SystemProcessRunner},
  sanitize::terminal_text,
};
use argvus_i18n::{Lang, tr};
use argvus_theme::Theme;
use argvus_tui::{
  buttons::{Button, ButtonKind},
  components::{
    ConfirmationDialog, ConfirmationOutcome, ConfirmationState, StatusKind, StatusMessage,
    draw_confirmation,
  },
  page::{Selection, list, readonly, shell, status},
};
use crossterm::event::KeyCode;
use ratatui::{
  Frame,
  layout::{Constraint, Layout, Margin},
  style::Style,
  text::Line,
  widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

enum Loaded {
  Packages(Vec<Package>),
  Updates(Vec<Update>),
  Cache(Vec<CachePackage>),
  History(Vec<HistoryEntry>),
  Aur(Vec<AurPackage>),
  Mirrors(Vec<Mirror>),
  Details(Box<PackageDetails>),
  MirrorPreview(String),
  MirrorCountries(Vec<String>),
  Dashboard(PackageDashboard),
}
#[derive(Clone)]
enum Action {
  Install(Vec<String>),
  Remove(Vec<String>),
  Reinstall(String),
  UpgradePackage(String),
  Upgrade,
  RefreshDatabase,
  CleanCache(&'static str),
  Downgrade(String),
  AurInstall(String),
  ApplyMirrors(String),
}

#[derive(Debug, Clone)]
struct MirrorEditor {
  selected: usize,
  options: ReflectorOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionButton {
  Install,
  Remove,
  Update,
  Upgrade,
  RefreshDatabase,
  ToggleMulti,
  CleanKeepThree,
  CleanKeepOne,
  CleanUninstalled,
  Downgrade,
}

pub struct PackagesApp {
  pub page: PackagesPage,
  selected: Selection,
  on_buttons: bool,
  button_selected: usize,
  button_from: Option<usize>,
  packages: Vec<Package>,
  updates: Vec<Update>,
  cache: Vec<CachePackage>,
  history: Vec<HistoryEntry>,
  aur: Vec<AurPackage>,
  mirrors: Vec<Mirror>,
  dashboard: PackageDashboard,
  details: Option<PackageDetails>,
  details_parent: PackagesPage,
  query: String,
  job: Option<JobHandle<Result<Loaded, String>>>,
  plan: Option<JobHandle<Result<(Action, TransactionPlan), String>>>,
  action: Option<JobHandle<Result<String, String>>>,
  jobs: JobManager,
  lang: Lang,
  theme: Theme,
  capabilities: Capabilities,
  input: Option<String>,
  input_search: bool,
  pending: Option<Action>,
  confirmation: ConfirmationState,
  preview: Option<TransactionPlan>,
  pub status: Option<StatusMessage>,
  multi: Vec<usize>,
  mirror_editor: Option<MirrorEditor>,
  mirror_countries: Option<Vec<String>>,
  transaction_live: Option<LiveProcess>,
  transaction_open: bool,
  transaction_scroll: u16,
  transaction_follow: bool,
}

const TRANSACTION_POPUP_WIDTH: u16 = 100;
const TRANSACTION_POPUP_HEIGHT: u16 = 20;
const TRANSACTION_CONTENT_WIDTH: usize = TRANSACTION_POPUP_WIDTH as usize - 2;
const TRANSACTION_CONTENT_HEIGHT: usize = TRANSACTION_POPUP_HEIGHT as usize - 2;

fn transaction_wrapped_lines(output: &str) -> usize {
  output
    .lines()
    .map(|line| (argvus_tui::text::display_width(line).div_ceil(TRANSACTION_CONTENT_WIDTH)).max(1))
    .sum()
}

fn transaction_bottom_offset(output: &str) -> u16 {
  transaction_wrapped_lines(output).saturating_sub(TRANSACTION_CONTENT_HEIGHT) as u16
}

impl PackagesApp {
  pub fn new(lang: Lang, theme: Theme, capabilities: Capabilities) -> Self {
    Self {
      page: PackagesPage::Home,
      selected: Selection::default(),
      on_buttons: false,
      button_selected: 0,
      button_from: None,
      packages: vec![],
      updates: vec![],
      cache: vec![],
      history: vec![],
      aur: vec![],
      mirrors: vec![],
      dashboard: PackageDashboard::default(),
      details: None,
      details_parent: PackagesPage::Search,
      query: String::new(),
      job: None,
      plan: None,
      action: None,
      jobs: JobManager::default(),
      lang,
      theme,
      capabilities,
      input: None,
      input_search: false,
      pending: None,
      confirmation: ConfirmationState::default(),
      preview: None,
      status: None,
      multi: vec![],
      mirror_editor: None,
      mirror_countries: None,
      transaction_live: None,
      transaction_open: false,
      transaction_scroll: 0,
      transaction_follow: false,
    }
  }
  fn busy(&self) -> bool {
    self.job.is_some() || self.plan.is_some() || self.action.is_some()
  }
  fn destructive(&self) -> bool {
    self.plan.is_some() || self.action.is_some()
  }
  pub fn reload(&mut self) {
    if self.destructive() {
      return;
    }
    if matches!(self.page, PackagesPage::Search | PackagesPage::Aur) && self.query.trim().is_empty()
    {
      self.status = Some(StatusMessage {
        kind: StatusKind::Info,
        text: match self.page {
          PackagesPage::Aur => tr(
            self.lang,
            "Digite / para buscar no AUR.",
            "Press / to search the AUR.",
          ),
          _ => tr(
            self.lang,
            "Digite / para buscar pacotes.",
            "Press / to search packages.",
          ),
        }
        .into(),
      });
      return;
    }
    let caps = self.capabilities.clone();
    let page = self.page;
    let query = self.query.clone();
    let name = self.selected_package_name();
    self.job = Some(self.jobs.spawn(move |_| {
      let b = PackageBackend::new(SystemProcessRunner, caps);
      let data = match page {
        PackagesPage::Installed => Loaded::Packages(b.installed().map_err(|e| e.to_string())?),
        PackagesPage::Search => Loaded::Packages(b.search(&query).map_err(|e| e.to_string())?),
        PackagesPage::Updates => Loaded::Updates(b.updates().map_err(|e| e.to_string())?),
        PackagesPage::Orphans => Loaded::Packages(b.orphans().map_err(|e| e.to_string())?),
        PackagesPage::Cache | PackagesPage::Downgrade => Loaded::Cache(b.cache()),
        PackagesPage::History => Loaded::History(b.history().map_err(|e| e.to_string())?),
        PackagesPage::Mirrors => Loaded::Mirrors(b.mirrors()),
        PackagesPage::Aur => {
          let h = b.aur_helper().ok_or("AUR helper is unavailable")?;
          Loaded::Aur(b.aur_search(h, &query).map_err(|e| e.to_string())?)
        }
        PackagesPage::Details(_) | PackagesPage::HistoryDetails(_) => Loaded::Details(Box::new(
          b.details(&name.ok_or("package not found")?)
            .map_err(|e| e.to_string())?,
        )),
        PackagesPage::Home => Loaded::Dashboard(b.dashboard()),
      };
      Ok(Ok(data))
    }));
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(self.lang, "Carregando pacotes...", "Loading packages...").into(),
    });
  }
  pub fn poll(&mut self) -> bool {
    if let Some(plan) = self.plan.take() {
      match plan.try_state() {
        JobState::Running => {
          self.plan = Some(plan);
          return false;
        }
        JobState::Finished(Ok(Ok((action, preview)))) => {
          self.preview = Some(preview);
          self.pending = Some(action);
          self.status = None;
          return true;
        }
        JobState::Finished(Ok(Err(error))) | JobState::Finished(Err(error)) => {
          self.error(error);
          return true;
        }
      }
    }
    if let Some(job) = self.job.take() {
      match job.try_state() {
        JobState::Running => {
          self.job = Some(job);
          return false;
        }
        JobState::Finished(Ok(Ok(d))) => {
          if !self.apply(d) {
            self.success(tr(self.lang, "Pacotes atualizados", "Packages refreshed"));
          }
          return true;
        }
        JobState::Finished(Ok(Err(e))) | JobState::Finished(Err(e)) => {
          self.error(e);
          return true;
        }
      }
    }
    self.poll_action()
  }
  fn poll_action(&mut self) -> bool {
    let Some(job) = self.action.take() else {
      return false;
    };
    match job.try_state() {
      JobState::Running => {
        self.action = Some(job);
        if self.transaction_open {
          if self.transaction_follow
            && let Some(live) = &self.transaction_live
          {
            self.transaction_scroll = transaction_bottom_offset(&live.output());
          }
          true
        } else {
          false
        }
      }
      JobState::Finished(Ok(Ok(_output))) => {
        self.action = None;
        self.success(tr(
          self.lang,
          "Operação concluída; atualizando pacotes.",
          "Operation completed; refreshing packages.",
        ));
        self.reload();
        true
      }
      JobState::Finished(Ok(Err(e))) | JobState::Finished(Err(e)) => {
        self.action = None;
        self.error(e);
        true
      }
    }
  }
  fn apply(&mut self, d: Loaded) -> bool {
    let mut reported_error = false;
    match d {
      Loaded::Packages(v) => self.packages = v,
      Loaded::Updates(v) => self.updates = v,
      Loaded::Cache(v) => self.cache = v,
      Loaded::History(v) => self.history = v,
      Loaded::Aur(v) => self.aur = v,
      Loaded::Mirrors(v) => self.mirrors = v,
      Loaded::Dashboard(mut v) => {
        if let Some(error) = v.error.take() {
          self.error(error);
          reported_error = true;
        }
        self.dashboard = v;
      }
      Loaded::Details(v) => self.details = Some(*v),
      Loaded::MirrorPreview(content) => {
        self.mirror_editor = None;
        self.pending = Some(Action::ApplyMirrors(content));
        self.confirmation = ConfirmationState::default();
      }
      Loaded::MirrorCountries(countries) => {
        self.mirror_countries = Some(countries.clone());
        if !countries.iter().any(|country| country == "Brazil")
          && let Some(editor) = &mut self.mirror_editor
          && editor
            .options
            .countries
            .first()
            .is_some_and(|country| country == "Brazil")
          && let Some(first) = countries.first()
        {
          editor.options.countries = vec![first.clone()];
        }
      }
    }
    self.selected.normalize(self.item_count());
    reported_error
  }
  fn error(&mut self, e: impl Into<String>) {
    self.status = Some(StatusMessage {
      kind: StatusKind::Error,
      text: terminal_text(&e.into()),
    });
  }
  fn success(&mut self, e: impl Into<String>) {
    self.status = Some(StatusMessage {
      kind: StatusKind::Success,
      text: e.into(),
    });
  }
  fn selected_package_name(&self) -> Option<String> {
    if matches!(self.page, PackagesPage::Details(_)) {
      if let Some(name) = self
        .details
        .as_ref()
        .map(|details| details.package.name.clone())
      {
        return Some(name);
      }
      return match self.details_parent {
        PackagesPage::Search | PackagesPage::Installed => self
          .visible_packages()
          .get(self.selected.index)
          .map(|package| package.name.clone()),
        PackagesPage::Orphans => self
          .visible_packages()
          .get(self.selected.index)
          .map(|package| package.name.clone()),
        PackagesPage::Updates => self
          .visible_updates()
          .get(self.selected.index)
          .map(|package| package.name.clone()),
        PackagesPage::Aur => self
          .aur
          .get(self.selected.index)
          .map(|package| package.name.clone()),
        _ => None,
      };
    }
    self
      .visible_packages()
      .get(self.selected.index)
      .map(|p| p.name.clone())
      .or_else(|| {
        self
          .visible_updates()
          .get(self.selected.index)
          .map(|p| p.name.clone())
      })
      .or_else(|| self.aur.get(self.selected.index).map(|p| p.name.clone()))
  }
  fn home_pages(&self) -> Vec<PackagesPage> {
    let mut pages = vec![
      PackagesPage::Search,
      PackagesPage::Installed,
      PackagesPage::Orphans,
      PackagesPage::Updates,
      PackagesPage::Cache,
      PackagesPage::History,
      PackagesPage::Downgrade,
      PackagesPage::Mirrors,
    ];
    if self.capabilities.has_paru || self.capabilities.has_yay {
      pages.insert(1, PackagesPage::Aur);
    }
    pages
  }
  fn visible_packages(&self) -> Vec<&Package> {
    let query = self.query.trim().to_ascii_lowercase();
    self
      .packages
      .iter()
      .filter(|package| {
        query.is_empty()
          || package.name.to_ascii_lowercase().contains(&query)
          || package.description.to_ascii_lowercase().contains(&query)
      })
      .collect()
  }
  fn matches_query(&self, name: &str, description: &str) -> bool {
    let query = self.query.trim().to_ascii_lowercase();
    query.is_empty()
      || name.to_ascii_lowercase().contains(&query)
      || description.to_ascii_lowercase().contains(&query)
  }
  fn visible_updates(&self) -> Vec<&Update> {
    self
      .updates
      .iter()
      .filter(|update| self.matches_query(&update.name, ""))
      .collect()
  }
  fn item_count(&self) -> usize {
    match self.page {
      PackagesPage::Home => self.home_pages().len(),
      PackagesPage::Search | PackagesPage::Installed => self.visible_packages().len(),
      PackagesPage::Aur => {
        if self.page == PackagesPage::Aur {
          self.aur.len()
        } else {
          self.visible_packages().len()
        }
      }
      PackagesPage::Orphans => self
        .packages
        .iter()
        .filter(|package| self.matches_query(&package.name, &package.description))
        .count(),
      PackagesPage::Updates => self.visible_updates().len(),
      PackagesPage::Cache | PackagesPage::Downgrade => self.cache.len(),
      PackagesPage::History | PackagesPage::HistoryDetails(_) => self.history.len(),
      PackagesPage::Mirrors => 1,
      PackagesPage::Details(_) => self
        .details
        .as_ref()
        .map_or(0, |details| detail_rows(self.lang, details).len()),
    }
  }
  fn selected_names(&self) -> Vec<String> {
    if self.page == PackagesPage::Orphans && !self.multi.is_empty() {
      self
        .multi
        .iter()
        .filter_map(|i| self.packages.get(*i))
        .map(|p| p.name.clone())
        .collect()
    } else {
      self.selected_package_name().into_iter().collect()
    }
  }
  pub fn handle(&mut self, key: KeyCode) -> bool {
    if self.mirror_editor.is_some() {
      return self.handle_mirror_editor(key);
    }
    if self.input.is_some() {
      return self.handle_input(key);
    }
    if self.pending.is_some() {
      match self.confirmation.handle(key) {
        ConfirmationOutcome::Confirmed => {
          self.confirmation = ConfirmationState::default();
          self.start_pending();
        }
        ConfirmationOutcome::Cancelled => {
          self.pending = None;
          self.preview = None;
          self.confirmation = ConfirmationState::default();
        }
        ConfirmationOutcome::Pending => {}
      }
      return false;
    }
    if self.transaction_open {
      match key {
        KeyCode::Esc => {
          self.transaction_open = false;
          self.transaction_live = None;
          self.transaction_scroll = 0;
          self.transaction_follow = false;
        }
        KeyCode::Up | KeyCode::Char('k') => {
          self.transaction_follow = false;
          self.transaction_scroll = self.transaction_scroll.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
          self.transaction_follow = false;
          self.transaction_scroll = self.transaction_scroll.saturating_add(1);
        }
        KeyCode::PageUp => {
          self.transaction_follow = false;
          self.transaction_scroll = self.transaction_scroll.saturating_sub(10);
        }
        KeyCode::PageDown => {
          self.transaction_follow = false;
          self.transaction_scroll = self.transaction_scroll.saturating_add(10);
        }
        KeyCode::Home => {
          self.transaction_follow = false;
          self.transaction_scroll = 0;
        }
        KeyCode::End => {
          if let Some(live) = &self.transaction_live {
            self.transaction_scroll = transaction_bottom_offset(&live.output());
          }
        }
        _ => {}
      }
      return false;
    }
    if self.on_buttons && !self.buttons().is_empty() {
      match key {
        KeyCode::Tab => {
          self.toggle_buttons(false);
          return false;
        }
        KeyCode::BackTab => {
          self.toggle_buttons(true);
          return false;
        }
        KeyCode::Left | KeyCode::Char('h') => {
          self.move_button(-1);
          return false;
        }
        KeyCode::Right | KeyCode::Char('l') => {
          self.move_button(1);
          return false;
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
          self.activate_button();
          return false;
        }
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Down | KeyCode::Char('j') => return false,
        _ => {}
      }
    }
    if matches!(key, KeyCode::Esc | KeyCode::Left) {
      if self.page == PackagesPage::Home {
        return true;
      }
      self.page = match self.page {
        PackagesPage::Details(_) => self.details_parent,
        PackagesPage::HistoryDetails(_) => PackagesPage::History,
        _ => PackagesPage::Home,
      };
      self.selected.index = 0;
      self.on_buttons = false;
      self.button_from = None;
      return false;
    }
    if self.destructive() {
      return false;
    }
    match key {
      KeyCode::Tab | KeyCode::BackTab => self.toggle_buttons(key == KeyCode::BackTab),
      KeyCode::Char(c)
        if matches!(
          self.page,
          PackagesPage::Search
            | PackagesPage::Installed
            | PackagesPage::Orphans
            | PackagesPage::Updates
            | PackagesPage::Aur
        ) && self.query.len() < 128
          && c != '/'
          && !c.is_control() =>
      {
        self.query.push(c);
        self.selected.index = 0;
      }
      KeyCode::Backspace
        if matches!(
          self.page,
          PackagesPage::Search
            | PackagesPage::Installed
            | PackagesPage::Orphans
            | PackagesPage::Updates
            | PackagesPage::Aur
        ) =>
      {
        self.query.pop();
        self.selected.index = 0;
      }
      KeyCode::Enter
        if matches!(self.page, PackagesPage::Search | PackagesPage::Aur)
          && self.selected_package_name().is_none() =>
      {
        let minimum = if self.page == PackagesPage::Aur { 2 } else { 1 };
        if self.query.trim().chars().count() < minimum {
          self.error(tr(
            self.lang,
            if self.page == PackagesPage::Aur {
              "A busca no AUR requer pelo menos 2 caracteres."
            } else {
              "Digite uma busca não vazia."
            },
            if self.page == PackagesPage::Aur {
              "AUR search requires at least 2 characters."
            } else {
              "Enter a non-empty search query."
            },
          ));
        } else {
          self.reload();
        }
      }
      KeyCode::Char('/') => {
        self.input = Some(self.query.clone());
        self.input_search = true;
      }
      KeyCode::Char('r') => self.reload(),
      KeyCode::Up
      | KeyCode::Down
      | KeyCode::Char('j')
      | KeyCode::Char('k')
      | KeyCode::Home
      | KeyCode::End
      | KeyCode::PageUp
      | KeyCode::PageDown => {
        self.selected.handle(key, self.item_count(), 8);
      }
      KeyCode::Enter | KeyCode::Right if self.page == PackagesPage::Home => {
        let pages = self.home_pages();
        self.page = pages
          .get(self.selected.index)
          .copied()
          .unwrap_or(PackagesPage::Mirrors);
        self.selected.index = 0;
        if matches!(self.page, PackagesPage::Search | PackagesPage::Installed) {
          self.query.clear();
          self.packages.clear();
        }
        self.reload()
      }
      KeyCode::Enter | KeyCode::Right
        if matches!(
          self.page,
          PackagesPage::Search
            | PackagesPage::Installed
            | PackagesPage::Updates
            | PackagesPage::Orphans
            | PackagesPage::Aur
        ) =>
      {
        if self.page == PackagesPage::Aur {
          if let Some(package) = self.aur.get(self.selected.index) {
            self.details = Some(PackageDetails {
              package: Package {
                name: package.name.clone(),
                version: package.version.clone(),
                repository: Some("AUR".into()),
                description: package.description.clone(),
                installed: package.installed,
                ..Default::default()
              },
              ..Default::default()
            });
            self.details_parent = PackagesPage::Aur;
            self.page = PackagesPage::Details(self.selected.index);
          }
        } else if let Some(_name) = self.selected_package_name() {
          self.details_parent = self.page;
          self.details = None;
          if self.page == PackagesPage::Updates {
            self.pending = Some(Action::UpgradePackage(_name));
            self.confirmation = ConfirmationState::default();
          } else {
            self.page = PackagesPage::Details(self.selected.index);
            self.reload()
          }
        }
      }
      KeyCode::Enter | KeyCode::Right if self.page == PackagesPage::Mirrors => {
        self.open_mirror_editor()
      }
      _ => {}
    }
    false
  }
  fn open_mirror_editor(&mut self) {
    if self.capabilities.has_reflector {
      let default_country = self
        .mirror_countries
        .as_deref()
        .filter(|countries| countries.iter().any(|country| country == "Brazil"))
        .map(|_| "Brazil".into())
        .or_else(|| {
          self
            .mirror_countries
            .as_deref()
            .and_then(|c| c.first().cloned())
        })
        .unwrap_or_else(|| "Brazil".into());
      self.mirror_editor = Some(MirrorEditor {
        selected: 0,
        options: ReflectorOptions {
          countries: vec![default_country],
          ..Default::default()
        },
      });
      if self.mirror_countries.is_none() && !self.busy() {
        self.spawn_country_list();
      }
    } else {
      self.error(tr(
        self.lang,
        "reflector não está disponível.",
        "reflector is unavailable.",
      ));
    }
  }
  fn spawn_country_list(&mut self) {
    let capabilities = self.capabilities.clone();
    self.job = Some(
      self
        .jobs
        .spawn(move |_| Ok(fetch_reflector_countries(capabilities).map(Loaded::MirrorCountries))),
    );
  }
  fn handle_mirror_editor(&mut self, key: KeyCode) -> bool {
    let Some(editor) = self.mirror_editor.as_mut() else {
      return false;
    };
    match key {
      KeyCode::Esc => self.mirror_editor = None,
      KeyCode::Up | KeyCode::Char('k') => editor.selected = editor.selected.saturating_sub(1),
      KeyCode::Down | KeyCode::Char('j') => editor.selected = (editor.selected + 1).min(5),
      KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') => match editor.selected {
        0 => cycle_country(
          &mut editor.options,
          self.mirror_countries.as_deref().unwrap_or_default(),
        ),
        1 => cycle_protocol(&mut editor.options),
        2 => {
          editor.options.age_hours = if matches!(key, KeyCode::Left) {
            editor.options.age_hours.saturating_sub(1).max(1)
          } else {
            (editor.options.age_hours + 1).min(24 * 365)
          }
        }
        3 => {
          editor.options.count = if matches!(key, KeyCode::Left) {
            editor.options.count.saturating_sub(1).max(1)
          } else {
            (editor.options.count + 1).min(100)
          }
        }
        4 => cycle_sort(&mut editor.options),
        _ => {}
      },
      KeyCode::Enter if editor.selected == 5 => {
        let options = editor.options.clone();
        self.start_mirror_preview(options);
      }
      _ => {}
    }
    false
  }
  fn start_mirror_preview(&mut self, options: ReflectorOptions) {
    if self.busy() {
      return;
    }
    let caps = self.capabilities.clone();
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(
        self.lang,
        "Gerando preview de mirrors...",
        "Generating mirror preview...",
      )
      .into(),
    });
    self.job = Some(
      self
        .jobs
        .spawn(move |_| Ok(generate_mirror_preview(options, caps).map(Loaded::MirrorPreview))),
    );
  }
  fn handle_input(&mut self, key: KeyCode) -> bool {
    match key {
      KeyCode::Char(c) if !c.is_control() && self.input.as_ref().is_some_and(|v| v.len() < 128) => {
        self.input.as_mut().unwrap().push(c)
      }
      KeyCode::Backspace => {
        self.input.as_mut().unwrap().pop();
      }
      KeyCode::Esc => {
        self.input = None;
        if self.input_search {
          self.query.clear();
          self.packages.clear();
          self.aur.clear();
          self.selected.index = 0;
        }
        self.input_search = false
      }
      KeyCode::Enter => {
        let candidate = self.input.take().unwrap_or_default();
        self.input_search = false;
        let minimum = if self.page == PackagesPage::Aur { 2 } else { 1 };
        if candidate.trim().chars().count() < minimum {
          self.error(if self.page == PackagesPage::Aur {
            tr(
              self.lang,
              "A busca no AUR requer pelo menos 2 caracteres.",
              "AUR search requires at least 2 characters.",
            )
          } else {
            tr(
              self.lang,
              "Digite uma busca não vazia.",
              "Enter a non-empty search query.",
            )
          });
          return false;
        }
        self.query = candidate;
        self.selected.index = 0;
        self.reload()
      }
      _ => {}
    }
    false
  }
  fn start_pending(&mut self) {
    let Some(a) = self.pending.take() else { return };
    let caps = self.capabilities.clone();
    self.preview = None;
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(self.lang, "Executando operação...", "Running operation...").into(),
    });
    let live = LiveProcess::new();
    self.transaction_live = Some(live.clone());
    self.transaction_open = true;
    self.transaction_scroll = 0;
    self.transaction_follow = true;
    self.action = Some(self.jobs.spawn(move |_| Ok(run_action(a, caps, live))));
  }
  fn begin_plan(&mut self, action: Action) {
    if self.busy() {
      return;
    }
    let caps = self.capabilities.clone();
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(
        self.lang,
        "Planejando transação...",
        "Planning transaction...",
      )
      .into(),
    });
    self.plan = Some(self.jobs.spawn(move |_| {
      let backend = PackageBackend::new(SystemProcessRunner, caps);
      let preview = match &action {
        Action::Install(packages) | Action::Remove(packages) => {
          if matches!(action, Action::Install(_)) {
            backend.plan_install(packages)
          } else {
            backend.plan_remove(packages)
          }
        }
        Action::Reinstall(package) => backend.plan_install(std::slice::from_ref(package)),
        Action::Upgrade | Action::UpgradePackage(_) => backend.plan_upgrade(),
        _ => Ok(TransactionPlan::default()),
      }
      .map_err(|error| error.to_string())?;
      Ok(Ok((action, preview)))
    }));
  }
  fn rows(&self) -> Vec<String> {
    match self.page {
      PackagesPage::Home => self.home_rows(),
      PackagesPage::Aur => {
        let mut rows = vec![self.search_row()];
        rows.extend(
          self
            .aur
            .iter()
            .filter(|package| self.matches_query(&package.name, &package.description))
            .map(|p| {
              format!(
                "{}  {}  [AUR]{}",
                p.name,
                p.version,
                if p.installed {
                  format!("   ● {}", tr(self.lang, "Instalado", "Installed"))
                } else {
                  String::new()
                }
              )
            }),
        );
        rows
      }
      PackagesPage::Search => {
        let mut rows = vec![self.search_row()];
        rows.extend(self.visible_packages().into_iter().map(|p| {
          let repo = p
            .repository
            .as_deref()
            .map(|value| format!("  [{value}]"))
            .unwrap_or_else(|| {
              if p.foreign {
                "  [AUR]".into()
              } else {
                String::new()
              }
            });
          let badge = if p.installed {
            format!("   ● {}", tr(self.lang, "Instalado", "Installed"))
          } else {
            String::new()
          };
          format!("{}  {}{}{}", p.name, p.version, repo, badge)
        }));
        rows
      }
      PackagesPage::Installed => {
        let mut rows = vec![self.search_row()];
        rows.extend(self.visible_packages().into_iter().map(|p| {
          let badge = if p.foreign {
            "  [AUR]".to_owned()
          } else {
            String::new()
          };
          format!("{}  {}{}", p.name, p.version, badge)
        }));
        rows
      }
      PackagesPage::Orphans => {
        let mut rows = vec![self.search_row()];
        rows.extend(
          self
            .visible_packages()
            .into_iter()
            .map(|p| format!("{}  {}", p.name, p.version)),
        );
        rows
      }
      PackagesPage::Updates => {
        let mut rows = vec![self.search_row()];
        rows.extend(
          self
            .visible_updates()
            .into_iter()
            .map(|p| format!("{}  {} → {}", p.name, p.current, p.available)),
        );
        rows
      }
      PackagesPage::Cache | PackagesPage::Downgrade => self
        .cache
        .iter()
        .map(|p| format!("{}  {}", cached_display_name(p), human_bytes(p.bytes)))
        .collect(),
      PackagesPage::History | PackagesPage::HistoryDetails(_) => self
        .history
        .iter()
        .map(|p| {
          let versions = match (&p.old_version, &p.new_version) {
            (Some(old), Some(new)) => format!("  ({old} → {new})"),
            _ => String::new(),
          };
          format!("{}  {}  {}{}", p.timestamp, p.action, p.package, versions)
        })
        .collect(),
      PackagesPage::Mirrors => vec![
        tr(
          self.lang,
          "Gerar mirrors com Reflector",
          "Generate mirrors with Reflector",
        )
        .into(),
      ],
      PackagesPage::Details(_) => self
        .details
        .as_ref()
        .map(|d| detail_rows(self.lang, d))
        .unwrap_or_default(),
    }
  }
  fn search_row(&self) -> String {
    format!(
      "{} {}: {}_",
      AppConfig::icon("🔍"),
      tr(self.lang, "Buscar", "Search"),
      self.query
    )
  }
  fn home_rows(&self) -> Vec<String> {
    let dashboard = &self.dashboard;
    let pending = tr(self.lang, "pendentes", "pending");
    let packages = tr(self.lang, "pacotes", "packages");
    let files = tr(self.lang, "arquivos", "files");
    let entries = tr(self.lang, "registros", "entries");
    let versions = tr(self.lang, "versões", "versions");
    let active = tr(self.lang, "ativos", "active");
    self
      .home_pages()
      .into_iter()
      .map(|page| match page {
        PackagesPage::Search => format!(
          "{} {}  ·  {} {}",
          AppConfig::icon("🔍"),
          tr(self.lang, "Instalar / Official", "Install / Official"),
          dashboard.available_count,
          packages
        ),
        PackagesPage::Aur => format!(
          "{} {}  ·  {}",
          AppConfig::icon("⭐"),
          tr(self.lang, "Instalar / AUR", "Install / AUR"),
          dashboard.aur_helper.as_deref().unwrap_or("—")
        ),
        PackagesPage::Installed => format!(
          "{} {}  ·  {} {}",
          AppConfig::icon("📦"),
          tr(self.lang, "Instalados / Official", "Installed / Official"),
          dashboard.installed_count,
          packages
        ),
        PackagesPage::Orphans => format!(
          "{} {}  ·  {} {}",
          AppConfig::icon("🧹"),
          tr(self.lang, "Instalados / Órfãos", "Installed / Orphans"),
          dashboard.orphan_count,
          tr(self.lang, "órfãos", "orphans")
        ),
        PackagesPage::Updates => format!(
          "{} {}  ·  {} {}",
          AppConfig::icon("🔄"),
          tr(self.lang, "Atualizações / Official", "Updates / Official"),
          dashboard.update_count,
          pending
        ),
        PackagesPage::Cache => format!(
          "{} {}  ·  {} {} · {}",
          AppConfig::icon("💾"),
          tr(self.lang, "Cache", "Cache"),
          dashboard.cache_count,
          files,
          human_bytes(dashboard.cache_bytes)
        ),
        PackagesPage::History => format!(
          "{} {}  ·  {} {}",
          AppConfig::icon("📜"),
          tr(self.lang, "Histórico", "History"),
          dashboard.history_count,
          entries
        ),
        PackagesPage::Downgrade => format!(
          "{} {}  ·  {} {}",
          AppConfig::icon("⏪"),
          tr(self.lang, "Downgrade", "Downgrade"),
          dashboard.cache_count,
          versions
        ),
        PackagesPage::Mirrors => format!(
          "{} {}  ·  {}/{} {}",
          AppConfig::icon("🌐"),
          tr(self.lang, "Mirrors", "Mirrors"),
          dashboard.mirrors_enabled,
          dashboard.mirrors_total,
          active
        ),
        _ => String::new(),
      })
      .collect()
  }
  pub fn draw(&self, f: &mut Frame) {
    let rows = self.rows();
    let breadcrumb = self.breadcrumb();
    let body = shell(f, f.area(), &self.theme, &breadcrumb, self.footer_hints());
    let buttons = self.buttons();
    let raw_buttons: Vec<Button> = buttons.iter().map(|(_, button)| button.clone()).collect();
    let (body, button_area) = if raw_buttons.is_empty() {
      (body, None)
    } else {
      let button_height = argvus_tui::buttons::height(&raw_buttons, body.width).min(body.height);
      let split =
        Layout::vertical([Constraint::Min(1), Constraint::Length(button_height)]).split(body);
      (split[0], Some(split[1]))
    };
    if self.page == PackagesPage::Mirrors {
      let mirror_rows = self
        .mirrors
        .iter()
        .map(|mirror| {
          Line::from(format!(
            "{} {}",
            if mirror.enabled { "●" } else { "○" },
            mirror.server
          ))
        })
        .collect::<Vec<_>>();
      let sections = Layout::vertical([
        Constraint::Length(mirror_rows.len().min(12) as u16),
        Constraint::Min(1),
      ])
      .split(body);
      readonly(f, sections[0], &self.theme, &mirror_rows);
      list(f, sections[1], &self.theme, &rows, self.selected.index);
    } else {
      list(
        f,
        body,
        &self.theme,
        &rows,
        self
          .selected
          .index
          .saturating_add(usize::from(matches!(
            self.page,
            PackagesPage::Search
              | PackagesPage::Installed
              | PackagesPage::Orphans
              | PackagesPage::Updates
              | PackagesPage::Aur
          )))
          .min(rows.len().saturating_sub(1)),
      );
    }
    if let Some(button_area) = button_area {
      let focus = if self.on_buttons {
        self.button_selected
      } else {
        usize::MAX
      };
      argvus_tui::buttons::draw(f, button_area, &raw_buttons, focus, &self.theme);
    }
    if let Some(input) = &self.input {
      let popup = argvus_tui::chrome::centered(f.area(), 50, 7);
      f.render_widget(Clear, popup);
      f.render_widget(
        Paragraph::new(vec![
          Line::from(if self.input_search {
            tr(self.lang, "Buscar pacotes:", "Search packages:")
          } else {
            tr(self.lang, "Entrada:", "Input:")
          }),
          Line::from(format!("{input}_")),
          Line::from(tr(
            self.lang,
            "Enter aplicar   Esc cancelar",
            "Enter apply   Esc cancel",
          )),
        ])
        .block(Block::bordered().title(tr(self.lang, "Buscar", "Search"))),
        popup,
      );
    }
    if let Some(editor) = &self.mirror_editor {
      let popup = argvus_tui::chrome::centered(f.area(), 60, 15);
      f.render_widget(Clear, popup);
      f.render_widget(
        Block::bordered().title(tr(self.lang, "Mirrors", "Mirrors")),
        popup,
      );
      let country = editor
        .options
        .countries
        .first()
        .map(String::as_str)
        .unwrap_or(tr(self.lang, "Todos", "All"));
      let fields = vec![
        format!("{}: {country}", tr(self.lang, "País", "Country")),
        format!(
          "{}: {}",
          tr(self.lang, "Protocolo", "Protocol"),
          editor.options.protocols.join(",")
        ),
        format!(
          "{}: {} h",
          tr(self.lang, "Idade máxima", "Maximum age"),
          editor.options.age_hours
        ),
        format!(
          "{}: {}",
          tr(self.lang, "Quantidade", "Count"),
          editor.options.count
        ),
        format!(
          "{}: {}",
          tr(self.lang, "Ordenação", "Sort"),
          editor.options.sort
        ),
        tr(self.lang, "Gerar preview", "Generate preview").into(),
      ];
      list(
        f,
        popup.inner(Margin::new(1, 1)),
        &self.theme,
        &fields,
        editor.selected,
      );
    }
    if let Some(a) = &self.pending {
      let message = format!(
        "{}{}",
        action_message(self.lang, a),
        self
          .preview
          .as_ref()
          .map(|plan| preview_message(self.lang, plan))
          .unwrap_or_default()
      );
      draw_confirmation(
        f,
        f.area(),
        &self.theme,
        ConfirmationDialog {
          title: tr(self.lang, "Confirmar transação", "Confirm transaction"),
          message: &message,
          confirm_label: tr(self.lang, "Aplicar", "Apply"),
          cancel_label: tr(self.lang, "Cancelar", "Cancel"),
          confirm_selected: self.confirmation.confirm_selected,
        },
      )
    }
    if let Some(s) = &self.status {
      status(f, f.area(), &self.theme, s)
    }
    if self.transaction_open
      && let Some(output) = self.transaction_live.as_ref().map(|live| live.output())
    {
      let popup = argvus_tui::chrome::centered(f.area(), 100, 20);
      let total = transaction_wrapped_lines(&output);
      let shown_total = total.max(1);
      let current = (self.transaction_scroll as usize + 1).min(shown_total);
      f.render_widget(Clear, popup);
      f.render_widget(
        Paragraph::new(output.as_str())
          .block(
            Block::new()
              .borders(Borders::ALL)
              .title(Line::styled(
                format!(
                  " {} · {} {} {} {} ",
                  tr(self.lang, "Processo da transação", "Transaction output"),
                  tr(self.lang, "linha", "line"),
                  current,
                  tr(self.lang, "de", "of"),
                  shown_total,
                ),
                Style::new().fg(self.theme.accent),
              ))
              .border_style(Style::new().fg(self.theme.border_active))
              .style(Style::new().bg(self.theme.background))
              .title_bottom(Line::from(tr(
                self.lang,
                "↑↓ rolar · PgUp/PgDn · Home/End · Esc fechar",
                "↑↓ scroll · PgUp/PgDn · Home/End · Esc close",
              ))),
          )
          .scroll((self.transaction_scroll, 0))
          .wrap(Wrap { trim: false }),
        popup,
      );
    }
  }
  fn breadcrumb(&self) -> String {
    if self.page == PackagesPage::Home {
      tr(self.lang, "Pacotes", "Packages").into()
    } else {
      format!(
        "{} > {}",
        tr(self.lang, "Pacotes", "Packages"),
        self.page_label()
      )
    }
  }
  fn page_label(&self) -> &'static str {
    match self.page {
      PackagesPage::Search | PackagesPage::Details(_) => tr(self.lang, "Buscar", "Search"),
      PackagesPage::Installed => tr(self.lang, "Instalados", "Installed"),
      PackagesPage::Updates => tr(self.lang, "Atualizações", "Updates"),
      PackagesPage::Orphans => tr(self.lang, "Órfãos", "Orphans"),
      PackagesPage::Cache => tr(self.lang, "Cache", "Cache"),
      PackagesPage::Aur => tr(self.lang, "AUR", "AUR"),
      PackagesPage::History | PackagesPage::HistoryDetails(_) => {
        tr(self.lang, "Histórico", "History")
      }
      PackagesPage::Downgrade => tr(self.lang, "Downgrade", "Downgrade"),
      PackagesPage::Mirrors => tr(self.lang, "Mirrors", "Mirrors"),
      PackagesPage::Home => tr(self.lang, "Pacotes", "Packages"),
    }
  }
  fn toggle_multi(&mut self) {
    if let Some(p) = self.multi.iter().position(|v| *v == self.selected.index) {
      self.multi.remove(p);
    } else {
      self.multi.push(self.selected.index)
    }
  }
  fn install_selected(&mut self) {
    let Some(name) = self.selected_package_name() else {
      return;
    };
    let installed = self.selection_installed();
    self.begin_plan(if installed {
      Action::Reinstall(name)
    } else if self.page == PackagesPage::Aur || self.details_parent == PackagesPage::Aur {
      Action::AurInstall(name)
    } else {
      Action::Install(vec![name])
    });
  }
  fn remove_selected(&mut self) {
    let installed = self.selection_installed();
    let v = self.selected_names();
    if installed && !v.is_empty() {
      self.begin_plan(Action::Remove(v))
    }
  }
  fn selection_installed(&self) -> bool {
    match self.page {
      PackagesPage::Details(_) => self
        .details
        .as_ref()
        .map(|details| details.package.installed)
        .unwrap_or(false),
      PackagesPage::Aur => self
        .aur
        .get(self.selected.index)
        .map(|package| package.installed)
        .unwrap_or(false),
      PackagesPage::Updates => self.selected_package_name().is_some(),
      _ => self
        .visible_packages()
        .get(self.selected.index)
        .map(|package| package.installed)
        .unwrap_or(false),
    }
  }
  fn toggle_buttons(&mut self, backwards: bool) {
    let count = self.buttons().len();
    if count == 0 {
      return;
    }
    if self.on_buttons {
      self.on_buttons = false;
      if let Some(index) = self.button_from.take() {
        self.selected.index = index;
      }
    } else {
      self.button_from = Some(self.selected.index);
      self.button_selected = if backwards { count - 1 } else { 0 };
      self.on_buttons = true;
    }
  }
  fn move_button(&mut self, delta: isize) {
    let count = self.buttons().len();
    if count == 0 {
      return;
    }
    self.button_selected =
      (self.button_selected as isize + delta).rem_euclid(count as isize) as usize;
  }
  fn activate_button(&mut self) {
    if self.busy() {
      return;
    }
    let actions = self.buttons();
    let Some((action, _)) = actions.get(self.button_selected) else {
      return;
    };
    match *action {
      ActionButton::Install => self.install_selected(),
      ActionButton::Remove => self.remove_selected(),
      ActionButton::Update => {
        if let Some(name) = self.selected_package_name() {
          self.pending = Some(Action::UpgradePackage(name));
          self.confirmation = ConfirmationState::default();
        }
      }
      ActionButton::Upgrade => self.begin_plan(Action::Upgrade),
      ActionButton::RefreshDatabase => {
        self.pending = Some(Action::RefreshDatabase);
        self.confirmation = ConfirmationState::default();
      }
      ActionButton::ToggleMulti => self.toggle_multi(),
      ActionButton::CleanKeepThree => self.pending = Some(Action::CleanCache("keep-three")),
      ActionButton::CleanKeepOne => self.pending = Some(Action::CleanCache("keep-one")),
      ActionButton::CleanUninstalled => self.pending = Some(Action::CleanCache("uninstalled")),
      ActionButton::Downgrade => {
        if let Some(p) = self.cache.get(self.selected.index) {
          self.pending = Some(Action::Downgrade(p.path.clone()));
        }
      }
    }
  }
  fn buttons(&self) -> Vec<(ActionButton, Button)> {
    let install = Button::new(tr(self.lang, "Instalar", "Install"), ButtonKind::Primary);
    let reinstall = Button::new(
      tr(self.lang, "Reinstalar", "Reinstall"),
      ButtonKind::Primary,
    );
    let remove = Button::new(tr(self.lang, "Remover", "Remove"), ButtonKind::Danger);
    match self.page {
      PackagesPage::Home
      | PackagesPage::History
      | PackagesPage::HistoryDetails(_)
      | PackagesPage::Mirrors => Vec::new(),
      PackagesPage::Installed if self.selected_package_name().is_some() => {
        vec![
          (ActionButton::Install, reinstall),
          (ActionButton::Remove, remove),
        ]
      }
      PackagesPage::Search | PackagesPage::Aur | PackagesPage::Details(_)
        if self.selected_package_name().is_some() =>
      {
        vec![
          (ActionButton::Install, install),
          (ActionButton::Remove, remove),
        ]
      }
      PackagesPage::Updates => {
        let mut buttons = vec![(
          ActionButton::RefreshDatabase,
          Button::new(
            tr(self.lang, "Atualizar banco", "Refresh database"),
            ButtonKind::Secondary,
          ),
        )];
        if !self.visible_updates().is_empty() {
          buttons.insert(
            0,
            (
              ActionButton::Update,
              Button::new(tr(self.lang, "Atualizar", "Update"), ButtonKind::Primary),
            ),
          );
          buttons.insert(
            1,
            (
              ActionButton::Upgrade,
              Button::new(
                tr(self.lang, "Atualizar tudo", "Upgrade all"),
                ButtonKind::Secondary,
              ),
            ),
          );
        }
        buttons
      }
      PackagesPage::Orphans if !self.packages.is_empty() => vec![
        (
          ActionButton::ToggleMulti,
          Button::new(tr(self.lang, "Marcar", "Select"), ButtonKind::Secondary),
        ),
        (ActionButton::Remove, remove),
      ],
      PackagesPage::Cache => vec![
        (
          ActionButton::CleanKeepThree,
          Button::new(
            tr(self.lang, "Limpar cache", "Clean cache"),
            ButtonKind::Primary,
          ),
        ),
        (
          ActionButton::CleanKeepOne,
          Button::new(tr(self.lang, "Manter 1", "Keep one"), ButtonKind::Secondary),
        ),
        (
          ActionButton::CleanUninstalled,
          Button::new(
            tr(self.lang, "Não instalados", "Uninstalled"),
            ButtonKind::Secondary,
          ),
        ),
      ],
      PackagesPage::Downgrade if self.cache.get(self.selected.index).is_some() => vec![(
        ActionButton::Downgrade,
        Button::new(tr(self.lang, "Rebaixar", "Downgrade"), ButtonKind::Primary),
      )],
      _ => Vec::new(),
    }
  }
  fn footer_hints(&self) -> &'static str {
    let home = tr(
      self.lang,
      "↑/↓ Navegar   →/Enter Abrir   ←/Esc Voltar   r Atualizar   ? Ajuda",
      "↑/↓ Navigate   →/Enter Open   ←/Esc Back   r Refresh   ? Help",
    );
    let action = tr(
      self.lang,
      "↑/↓ Navegar   Tab Ações   ←/→ Mover   Enter Ativar   r Atualizar   ←/Esc Voltar   ? Ajuda",
      "↑/↓ Navigate   Tab Actions   ←/→ Move   Enter Activate   r Refresh   ←/Esc Back   ? Help",
    );
    let readonly = tr(
      self.lang,
      "r Atualizar   ←/Esc Voltar   ? Ajuda",
      "r Refresh   ←/Esc Back   ? Help",
    );
    match self.page {
      PackagesPage::Home => home,
      PackagesPage::History | PackagesPage::HistoryDetails(_) => readonly,
      _ => {
        if self.buttons().is_empty() {
          readonly
        } else {
          action
        }
      }
    }
  }
}
fn detail_rows(lang: Lang, d: &PackageDetails) -> Vec<String> {
  let status = if d.package.installed {
    format!("● {}", tr(lang, "Instalado", "Installed"))
  } else {
    tr(lang, "Não instalado", "Not installed").into()
  };
  let installed_size = d
    .installed_size
    .map(human_bytes)
    .unwrap_or_else(|| "—".into());
  let download_size = d
    .download_size
    .map(human_bytes)
    .unwrap_or_else(|| "—".into());
  vec![
    format!(
      " {} {}",
      AppConfig::icon("📦"),
      tr(lang, "PACOTE", "PACKAGE")
    ),
    format!("   {:<18} {}", tr(lang, "Nome:", "Name:"), d.package.name),
    format!(
      "   {:<18} {}",
      tr(lang, "Versão:", "Version:"),
      d.package.version
    ),
    format!(
      "   {:<18} {}",
      tr(lang, "Repositório:", "Repository:"),
      d.package.repository.as_deref().unwrap_or("—")
    ),
    format!("   {:<18} {}", tr(lang, "Status:", "Status:"), status),
    "".into(),
    format!(
      "   {:<18} {}",
      tr(lang, "Descrição:", "Description:"),
      d.package.description
    ),
    format!(
      "   {:<18} {}",
      tr(lang, "Tamanho instalado:", "Installed size:"),
      installed_size
    ),
    format!(
      "   {:<18} {}",
      tr(lang, "Download:", "Download:"),
      download_size
    ),
    "".into(),
    format!(
      " {} {}",
      AppConfig::icon("🔗"),
      tr(lang, "ORIGEM", "SOURCE")
    ),
    format!(
      "   {:<18} {}",
      tr(lang, "Arquitetura:", "Architecture:"),
      d.architecture.as_deref().unwrap_or("—")
    ),
    format!(
      "   {:<18} {}",
      tr(lang, "URL:", "URL:"),
      d.url.as_deref().unwrap_or("—")
    ),
    format!(
      "   {:<18} {}",
      tr(lang, "Licenças:", "Licenses:"),
      join_or_dash(&d.licenses)
    ),
    format!(
      "   {:<18} {}",
      tr(lang, "Grupos:", "Groups:"),
      join_or_dash(&d.groups)
    ),
    format!(
      "   {:<18} {}",
      tr(lang, "Instalado em:", "Install date:"),
      d.install_date.as_deref().unwrap_or("—")
    ),
    "".into(),
    format!(
      " {} {}",
      AppConfig::icon("⚙"),
      tr(lang, "DEPENDÊNCIAS", "DEPENDENCIES")
    ),
    format!(
      "   {:<18} {}",
      tr(lang, "Depende de:", "Depends on:"),
      join_or_dash(&d.dependencies)
    ),
    format!(
      "   {:<18} {}",
      tr(lang, "Opcionais:", "Optional:"),
      join_or_dash(&d.optional_dependencies)
    ),
    format!(
      "   {:<18} {}",
      tr(lang, "Necessário p/:", "Required by:"),
      join_or_dash(&d.required_by)
    ),
    format!(
      "   {:<18} {}",
      tr(lang, "Provê:", "Provides:"),
      join_or_dash(&d.provides)
    ),
    format!(
      "   {:<18} {}",
      tr(lang, "Conflita com:", "Conflicts:"),
      join_or_dash(&d.conflicts)
    ),
    format!(
      "   {:<18} {}",
      tr(lang, "Substitui:", "Replaces:"),
      join_or_dash(&d.replaces)
    ),
    "".into(),
    tr(
      lang,
      "   [ Tab ] Ações     [ r ] Atualizar",
      "   [ Tab ] Actions  [ r ] Refresh",
    )
    .into(),
  ]
}
fn join_or_dash(values: &[String]) -> String {
  if values.is_empty() {
    "—".into()
  } else {
    values.join(" ")
  }
}
fn cached_display_name(package: &CachePackage) -> String {
  if !package.version.is_empty() {
    return format!("{} {}", package.name, package.version);
  }
  const SUFFIXES: &[&str] = &[
    ".pkg.tar.zst",
    ".pkg.tar.xz",
    ".pkg.tar.gz",
    ".pkg.tar.lrz",
    ".pkg.tar.lz4",
    ".pkg.tar.lzo",
    ".pkg.tar",
  ];
  let mut name = package.name.as_str();
  for suffix in SUFFIXES {
    if let Some(stripped) = name.strip_suffix(suffix) {
      name = stripped;
      break;
    }
  }
  name.to_owned()
}
fn human_bytes(bytes: u64) -> String {
  const KIB: u64 = 1024;
  const MIB: u64 = 1024 * 1024;
  const GIB: u64 = 1024 * 1024 * 1024;
  const TIB: u64 = 1024 * 1024 * 1024 * 1024;
  if bytes >= TIB {
    format!("{:.1} TiB", bytes as f64 / TIB as f64)
  } else if bytes >= GIB {
    format!("{:.1} GiB", bytes as f64 / GIB as f64)
  } else if bytes >= MIB {
    format!("{:.0} MiB", bytes as f64 / MIB as f64)
  } else if bytes >= KIB {
    format!("{:.0} KiB", bytes as f64 / KIB as f64)
  } else {
    format!("{bytes} B")
  }
}
fn action_message(lang: Lang, a: &Action) -> String {
  match a {
    Action::Install(v) => format!("{}: {}", tr(lang, "Instalar", "Install"), v.join(", ")),
    Action::Remove(v) => format!("{}: {}", tr(lang, "Remover", "Remove"), v.join(", ")),
    Action::Reinstall(v) => format!("{}: {v}", tr(lang, "Reinstalar", "Reinstall")),
    Action::Upgrade => tr(
      lang,
      "Atualizar o sistema completamente?",
      "Perform a full system upgrade?",
    )
    .into(),
    Action::UpgradePackage(v) => format!("{}: {v}", tr(lang, "Atualizar pacote", "Update package")),
    Action::RefreshDatabase => tr(
      lang,
      "Atualizar banco de dados dos pacotes?",
      "Refresh package database?",
    )
    .into(),
    Action::CleanCache(v) => format!("{} ({v})?", tr(lang, "Limpar cache", "Clean cache")),
    Action::Downgrade(v) => format!("{}: {v}", tr(lang, "Fazer downgrade", "Downgrade")),
    Action::AurInstall(v) => format!(
      "{} AUR: {v}? {}",
      tr(lang, "Instalar", "Install"),
      tr(
        lang,
        "Os PKGBUILDs executam instruções de build como usuário normal.",
        "PKGBUILDs execute build instructions as the normal user.",
      )
    ),
    Action::ApplyMirrors(content) => format!(
      "{}\n{}",
      tr(lang, "Aplicar estes mirrors?", "Apply these mirrors?"),
      mirror_preview_summary(content)
    ),
  }
}
fn preview_message(lang: Lang, plan: &TransactionPlan) -> String {
  format!(
    "\n\n{}: {}\n{}: {}\n{}: {}",
    tr(lang, "Instalar", "Install"),
    plan.install.len(),
    tr(lang, "Remover", "Remove"),
    plan.remove.len(),
    tr(lang, "Download bytes", "Download bytes"),
    plan.download_bytes
  )
}
fn run_action(a: Action, caps: Capabilities, live: LiveProcess) -> Result<String, String> {
  let executable = std::env::current_exe()
    .map_err(|error| error.to_string())?
    .to_string_lossy()
    .into_owned();
  let privileged = SystemSettingsOperation::new(SystemProcessRunner, executable);
  let request = |name: &str, args: Vec<String>| -> Result<String, String> {
    let r = PrivilegedRequest::new("package", name, args)?;
    let o = privileged.execute_live(&r, &live)?;
    if o.status == Some(0) {
      let stdout = terminal_text(&String::from_utf8_lossy(&o.stdout))
        .trim()
        .to_owned();
      Ok(if stdout.is_empty() {
        "Operation completed.".into()
      } else {
        stdout
      })
    } else {
      Err(terminal_text(&String::from_utf8_lossy(&o.stderr)))
    }
  };
  match a {
    Action::Install(v) => {
      live.push_line(&format!("$ pacman -S --needed {}", v.join(" ")));
      request("install", v)
    }
    Action::Remove(v) => {
      live.push_line(&format!("$ pacman -Rns {}", v.join(" ")));
      request("remove", v)
    }
    Action::Reinstall(v) => {
      live.push_line(&format!("$ pacman -S {v}"));
      request("reinstall", vec![v])
    }
    Action::Upgrade => {
      live.push_line("$ pacman -Syu");
      request("upgrade", vec![])
    }
    Action::UpgradePackage(v) => {
      live.push_line(&format!("$ pacman -S {v}"));
      request("upgrade-package", vec![v])
    }
    Action::RefreshDatabase => {
      live.push_line("$ pacman -Syy");
      request("refresh-db", vec![])
    }
    Action::CleanCache(v) => {
      live.push_line("$ paccache -r");
      request("clean-cache", vec![v.into()])
    }
    Action::Downgrade(v) => {
      live.push_line(&format!("$ pacman -U {v}"));
      request("downgrade", vec![v])
    }
    Action::ApplyMirrors(content) => {
      live.push_line("$ update mirrorlist");
      request("mirror-apply", vec![content])
    }
    Action::AurInstall(name) => {
      let helper = if caps.has_paru {
        "paru"
      } else if caps.has_yay {
        "yay"
      } else {
        return Err("AUR helper is unavailable".into());
      };
      live.push_line(&format!("$ {helper} -S {name}"));
      let o = SystemProcessRunner
        .run_live(&ProcessRequest::new(helper).arg("-S").arg(&name), &live)
        .map_err(|e| e.to_string())?;
      if o.status == Some(0) {
        Ok(terminal_text(&String::from_utf8_lossy(&o.stdout)))
      } else {
        Err(terminal_text(&String::from_utf8_lossy(&o.stderr)))
      }
    }
  }
}

fn generate_mirror_preview(
  options: ReflectorOptions,
  caps: Capabilities,
) -> Result<String, String> {
  if !caps.has_reflector {
    return Err("reflector is unavailable".into());
  }
  let args = reflector_args(&options).map_err(|error| error.to_string())?;
  let request = args
    .iter()
    .fold(ProcessRequest::new("reflector"), |request, argument| {
      request.arg(argument)
    });
  let output = SystemProcessRunner
    .run(&request)
    .map_err(|error| error.to_string())?;
  if output.status != Some(0) {
    return Err(terminal_text(&String::from_utf8_lossy(&output.stderr)));
  }
  let content = terminal_text(&String::from_utf8_lossy(&output.stdout));
  if !content
    .lines()
    .any(|line| line.trim_start().starts_with("Server = "))
  {
    return Err("reflector generated no valid mirrors".into());
  }
  Ok(content)
}

fn fetch_reflector_countries(caps: Capabilities) -> Result<Vec<String>, String> {
  if !caps.has_reflector {
    return Err("reflector is unavailable".into());
  }
  let request = ProcessRequest::new("reflector").arg("--list-countries");
  let output = SystemProcessRunner
    .run(&request)
    .map_err(|error| error.to_string())?;
  if output.status != Some(0) {
    return Err(terminal_text(&String::from_utf8_lossy(&output.stderr)));
  }
  Ok(parse_reflector_countries(&String::from_utf8_lossy(
    &output.stdout,
  )))
}

fn mirror_preview_summary(content: &str) -> String {
  let mirrors = content
    .lines()
    .filter(|line| line.trim_start().starts_with("Server = "))
    .collect::<Vec<_>>();
  let mut summary = format!("{} mirror(s)", mirrors.len());
  for mirror in mirrors.into_iter().take(3) {
    summary.push('\n');
    summary.push_str(mirror);
  }
  summary
}

fn cycle_country(options: &mut ReflectorOptions, countries: &[String]) {
  const FALLBACK: &[Option<&str>] = &[
    None,
    Some("Brazil"),
    Some("United States"),
    Some("Germany"),
    Some("France"),
  ];
  let current = options.countries.first().map(String::as_str);
  if countries.is_empty() {
    let index = FALLBACK
      .iter()
      .position(|country| *country == current)
      .unwrap_or(0);
    options.countries = FALLBACK[(index + 1) % FALLBACK.len()]
      .map(|country| vec![country.into()])
      .unwrap_or_default();
    return;
  }
  let mut names: Vec<Option<&str>> = vec![None];
  let mut seen = std::collections::HashSet::new();
  for country in countries {
    if seen.insert(country.as_str()) {
      names.push(Some(country.as_str()));
    }
  }
  let index = names
    .iter()
    .position(|country| *country == current)
    .unwrap_or(0);
  options.countries = names[(index + 1) % names.len()]
    .map(|country| vec![country.into()])
    .unwrap_or_default();
}

fn cycle_protocol(options: &mut ReflectorOptions) {
  const PROTOCOLS: &[&str] = &["https", "http", "rsync"];
  let current = options
    .protocols
    .first()
    .map(String::as_str)
    .unwrap_or("https");
  let index = PROTOCOLS
    .iter()
    .position(|protocol| *protocol == current)
    .unwrap_or(0);
  options.protocols = vec![PROTOCOLS[(index + 1) % PROTOCOLS.len()].into()];
}

fn cycle_sort(options: &mut ReflectorOptions) {
  const SORTS: &[&str] = &["rate", "age", "score", "delay", "country"];
  let index = SORTS
    .iter()
    .position(|sort| *sort == options.sort)
    .unwrap_or(0);
  options.sort = SORTS[(index + 1) % SORTS.len()].into();
}

#[cfg(test)]
mod tests {
  use super::*;
  use ratatui::{Terminal, backend::TestBackend};

  #[test]
  fn package_home_rows_act_as_a_status_dashboard() {
    let mut app = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.dashboard = PackageDashboard {
      installed_count: 1200,
      update_count: 7,
      orphan_count: 3,
      cache_count: 12,
      cache_bytes: 240_057_409_536,
      history_count: 40,
      mirrors_total: 6,
      mirrors_enabled: 4,
      available_count: 18_500,
      aur_helper: None,
      ..Default::default()
    };
    let rows = app.home_rows();
    assert_eq!(rows.len(), 8);
    assert!(rows[0].contains("18500"), "{}", rows[0]);
    assert!(
      rows[1].contains("1200") && rows[1].contains("Installed"),
      "{}",
      rows[1]
    );
    assert!(
      rows[3].contains("7") && rows[3].contains("pending"),
      "{}",
      rows[3]
    );
    assert!(
      rows[4].contains("12") && rows[4].contains("223.6 GiB"),
      "{}",
      rows[4]
    );
    assert!(
      rows[7].contains("4/6") && rows[7].contains("active"),
      "{}",
      rows[7]
    );
  }

  #[test]
  fn package_detail_pages_render_section_headers_and_aligned_rows() {
    let mut app = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.details = Some(PackageDetails {
      package: Package {
        name: "firefox".into(),
        version: "155.0.1-1".into(),
        repository: Some("extra".into()),
        installed: true,
        ..Default::default()
      },
      architecture: Some("x86_64".into()),
      url: Some("https://www.mozilla.org".into()),
      installed_size: Some(87_654_321),
      dependencies: vec!["gtk4".into(), "libx11".into()],
      ..Default::default()
    });
    app.page = PackagesPage::Details(0);
    let rows = app.rows();
    assert!(rows[0].contains("PACKAGE"), "{}", rows[0]);
    assert!(rows.iter().any(|r| r.contains("firefox")));
    assert!(rows.iter().any(|r| r.contains("x86_64")));
    assert!(rows.iter().any(|r| r.contains("gtk4")));
    assert!(rows.iter().any(|r| r.contains("● Installed")));
    assert!(rows.last().unwrap().contains("Actions"));
    assert!(rows.len() > 10);
  }

  #[test]
  fn cache_rows_strip_package_suffixes_and_humanize_bytes() {
    let mut app = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.page = PackagesPage::Cache;
    app.cache = vec![CachePackage {
      name: "firefox-155.0.1-1-x86_64.pkg.tar.zst".into(),
      bytes: 2_621_440,
      ..Default::default()
    }];
    let rows = app.rows();
    assert_eq!(rows[0], "firefox-155.0.1-1-x86_64  2 MiB");
  }

  #[test]
  fn package_home_is_navigable_and_uses_shared_chrome() {
    let mut app = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.handle(KeyCode::Down);
    app.handle(KeyCode::Enter);
    assert_eq!(app.page, PackagesPage::Installed);
    assert!(!app.handle(KeyCode::Esc));
    assert!(app.handle(KeyCode::Esc));

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
    assert!(text.contains("Packages"));
    assert!(text.contains(app.theme.name.as_str()));
    assert!(text.contains("Enter"));
  }

  #[test]
  fn empty_selection_does_not_create_package_action() {
    let mut app = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.page = PackagesPage::Search;
    app.handle(KeyCode::Tab);
    assert!(!app.on_buttons);
    app.handle(KeyCode::Enter);
    assert!(app.pending.is_none());
    app.handle(KeyCode::Right);
    app.handle(KeyCode::Enter);
    assert!(app.pending.is_none());
  }

  #[test]
  fn enter_opens_selected_package_details() {
    let mut app = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.page = PackagesPage::Search;
    app.packages.push(Package {
      name: "firefox".into(),
      version: "1.0".into(),
      ..Default::default()
    });
    app.handle(KeyCode::Enter);
    assert_eq!(app.page, PackagesPage::Details(0));
  }

  #[test]
  fn search_pages_do_not_spawn_until_a_valid_query_is_confirmed() {
    let mut app = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.page = PackagesPage::Search;
    app.reload();
    assert!(app.job.is_none());
    app.page = PackagesPage::Aur;
    app.reload();
    assert!(app.job.is_none());
    app.handle(KeyCode::Char('/'));
    app.handle(KeyCode::Char('a'));
    app.handle(KeyCode::Enter);
    assert!(app.job.is_none());
    assert!(app.status.as_ref().unwrap().text.contains("at least 2"));
  }

  #[test]
  fn cancelling_package_search_clears_the_query() {
    let mut app = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.page = PackagesPage::Search;
    app.query = "firefox".into();
    app.input = Some(app.query.clone());
    app.input_search = true;
    app.handle(KeyCode::Esc);
    assert!(app.query.is_empty());
    assert!(app.input.is_none());
  }

  #[test]
  fn opening_package_details_keeps_the_selected_name_until_metadata_arrives() {
    let mut app = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.page = PackagesPage::Search;
    app.packages.push(Package {
      name: "firefox".into(),
      version: "1.0".into(),
      ..Default::default()
    });
    app.details_parent = PackagesPage::Search;
    app.page = PackagesPage::Details(0);
    assert_eq!(app.selected_package_name().as_deref(), Some("firefox"));
  }

  #[test]
  fn package_confirmation_cancel_does_not_start_an_operation() {
    let mut app = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.pending = Some(Action::Upgrade);
    app.handle(KeyCode::Enter);
    assert!(app.pending.is_none());
    assert!(app.action.is_none());
  }

  #[test]
  fn mirror_editor_is_visible_and_configurable() {
    let caps = Capabilities {
      has_reflector: true,
      ..Default::default()
    };
    let mut app = PackagesApp::new(Lang::En, Theme::load(), caps);
    app.mirror_countries = Some(vec!["Brazil".into(), "Argentina".into()]);
    app.page = PackagesPage::Mirrors;
    app.handle(KeyCode::Tab);
    app.handle(KeyCode::Enter);
    assert!(app.mirror_editor.is_some());
    app.handle(KeyCode::Right);
    assert_eq!(
      app.mirror_editor.as_ref().unwrap().options.countries,
      ["Argentina"]
    );
    app.handle(KeyCode::Left);
    assert!(
      app
        .mirror_editor
        .as_ref()
        .unwrap()
        .options
        .countries
        .is_empty()
    );
    app.handle(KeyCode::Esc);
    assert!(app.mirror_editor.is_none());
  }

  #[test]
  fn country_cycling_falls_back_to_a_small_static_list_until_reflector_loads() {
    let mut options = ReflectorOptions {
      countries: vec!["Brazil".into()],
      ..Default::default()
    };
    cycle_country(&mut options, &[]);
    assert_eq!(options.countries, ["United States"]);
    cycle_country(&mut options, &["Brazil".into(), "Argentina".into()]);
    assert_eq!(options.countries, ["Brazil"]);
    cycle_country(&mut options, &["Brazil".into(), "Argentina".into()]);
    assert_eq!(options.countries, ["Argentina"]);
  }

  #[test]
  fn packages_expose_action_buttons_per_page_and_none_on_history() {
    let app = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    assert_eq!(app.buttons().len(), 0);
    let mut search = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    search.page = PackagesPage::Search;
    search.packages.push(Package {
      name: "firefox".into(),
      ..Default::default()
    });
    assert_eq!(search.buttons().len(), 2);
    let mut orphans = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    orphans.page = PackagesPage::Orphans;
    orphans.packages.push(Package {
      name: "orphan".into(),
      ..Default::default()
    });
    assert_eq!(orphans.buttons().len(), 2);
    let mut cache = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    cache.page = PackagesPage::Cache;
    assert_eq!(cache.buttons().len(), 3);
    let mut history = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    history.page = PackagesPage::History;
    assert_eq!(history.buttons().len(), 0);
  }

  #[test]
  fn stale_details_do_not_block_removal_on_list_pages() {
    let mut app = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.page = PackagesPage::Orphans;
    app.packages.push(Package {
      name: "orphan".into(),
      installed: true,
      ..Default::default()
    });
    app.details = Some(PackageDetails {
      package: Package {
        name: "firefox".into(),
        installed: false,
        ..Default::default()
      },
      ..Default::default()
    });
    app.remove_selected();
    assert!(
      app.plan.is_some(),
      "removal of an orphan must proceed despite stale uninstalled details"
    );
  }

  #[test]
  fn install_selected_follows_list_state_not_stale_details() {
    let mut installed_list = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    installed_list.page = PackagesPage::Installed;
    installed_list.packages.push(Package {
      name: "firefox".into(),
      installed: true,
      ..Default::default()
    });
    installed_list.details = Some(PackageDetails {
      package: Package {
        name: "zsh".into(),
        installed: false,
        ..Default::default()
      },
      ..Default::default()
    });
    installed_list.install_selected();
    assert!(
      installed_list.plan.is_some(),
      "installed list entry should trigger a plan (reinstall)"
    );

    let mut not_installed = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    not_installed.page = PackagesPage::Search;
    not_installed.packages.push(Package {
      name: "firefox".into(),
      installed: false,
      ..Default::default()
    });
    not_installed.details = Some(PackageDetails {
      package: Package {
        name: "firefox".into(),
        installed: true,
        ..Default::default()
      },
      ..Default::default()
    });
    not_installed.install_selected();
    assert!(
      not_installed.plan.is_some(),
      "uninstalled search entry should trigger an install plan"
    );
  }

  #[test]
  fn transaction_window_scrolls_closes_and_generates_action_plans() {
    let mut app = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    let live = LiveProcess::new();
    for _ in 0..40 {
      live.push_line("line");
    }
    app.transaction_live = Some(live.clone());
    app.transaction_open = true;
    let total = transaction_wrapped_lines(&live.output());
    assert_eq!(total, 40);
    app.handle(KeyCode::Down);
    assert_eq!(app.transaction_scroll, 1);
    app.handle(KeyCode::Up);
    assert_eq!(app.transaction_scroll, 0);
    app.handle(KeyCode::PageDown);
    assert_eq!(app.transaction_scroll, 10);
    app.handle(KeyCode::Home);
    assert_eq!(app.transaction_scroll, 0);
    app.handle(KeyCode::End);
    assert_eq!(
      app.transaction_scroll as usize,
      40usize.saturating_sub(TRANSACTION_CONTENT_HEIGHT)
    );
    app.handle(KeyCode::Esc);
    assert!(!app.transaction_open);
    assert_eq!(app.transaction_scroll, 0);
  }

  #[test]
  fn start_pending_opens_the_process_window_immediately() {
    let mut app = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.pending = Some(Action::RefreshDatabase);
    app.start_pending();
    assert!(
      app.transaction_open,
      "window must appear when the process starts"
    );
    assert!(app.transaction_live.is_some());
    assert!(app.transaction_follow);
    assert!(app.action.is_some());
    app.handle(KeyCode::Esc);
    assert!(!app.transaction_open);
  }

  #[test]
  fn transaction_bottom_offset_is_zero_for_short_output() {
    let mut app = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    let live = LiveProcess::new();
    live.push_line("done.");
    app.transaction_live = Some(live);
    app.transaction_open = true;
    app.handle(KeyCode::End);
    assert_eq!(app.transaction_scroll, 0);
    assert_eq!(transaction_bottom_offset("done."), 0);
  }

  #[test]
  fn transaction_window_uses_theme_background_and_border() {
    let theme = Theme::load();
    let mut app = PackagesApp::new(Lang::En, theme.clone(), Capabilities::default());
    let live = LiveProcess::new();
    live.push_line("ok");
    app.transaction_live = Some(live);
    app.transaction_open = true;
    let mut terminal = Terminal::new(TestBackend::new(120, 25)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    let buffer = terminal.backend().buffer();
    let styled = buffer.content.iter().any(|cell| {
      cell.symbol() == "│" && cell.bg == theme.background && cell.fg == theme.border_active
    });
    assert!(
      styled,
      "border should contrast with the background behind it"
    );
  }

  #[test]
  fn packages_tab_cycles_between_list_and_buttons_and_backtab_lands_last() {
    let mut app = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.page = PackagesPage::Cache;
    app.handle(KeyCode::Tab);
    assert!(app.on_buttons);
    app.handle(KeyCode::Tab);
    assert!(!app.on_buttons);
    app.handle(KeyCode::BackTab);
    assert!(app.on_buttons);
    assert_eq!(app.button_selected, 2);
    app.handle(KeyCode::Tab);
    assert!(!app.on_buttons);
  }

  #[test]
  fn packages_renders_button_bar_on_action_pages() {
    let mut app = PackagesApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.page = PackagesPage::Updates;
    app.updates.push(Update {
      name: "linux".into(),
      current: "1".into(),
      available: "2".into(),
      ..Default::default()
    });
    let mut terminal = Terminal::new(TestBackend::new(90, 25)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    let text = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(text.contains("Actions") || text.contains("Ações"));
  }
}
