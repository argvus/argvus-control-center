use crate::{
  backend::NetworkBackend,
  model::{NetworkPage, NetworkSnapshot},
};
use argvus_control_center_core::{
  capabilities::Capabilities,
  config::AppConfig,
  jobs::{JobHandle, JobManager, JobState},
  privileged::{PrivilegedOperation, PrivilegedRequest, SystemSettingsOperation},
  process::SystemProcessRunner,
  sanitize::terminal_text,
};
use argvus_control_center_settings::{
  App as SettingsApp, Page as SettingsPage, event as settings_event, ui as settings_ui,
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
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::{
  Frame,
  layout::{Constraint, Layout},
  text::Line,
  widgets::{Block, Clear, Paragraph},
};

pub struct NetworkApp {
  pub page: NetworkPage,
  detail_parent: NetworkPage,
  selected: Selection,
  on_buttons: bool,
  button_selected: usize,
  button_from: Option<usize>,
  snapshot: NetworkSnapshot,
  job: Option<JobHandle<Result<NetworkSnapshot, String>>>,
  action: Option<JobHandle<Result<String, String>>>,
  jobs: JobManager,
  lang: Lang,
  theme: Theme,
  capabilities: Capabilities,
  password: Option<String>,
  pending_wifi: Option<usize>,
  dns_input: Option<String>,
  search: Option<String>,
  status: Option<StatusMessage>,
  scan: bool,
  confirm_forget: bool,
  confirmation: ConfirmationState,
  firewall: Option<SettingsApp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionButton {
  Connect,
  Disconnect,
  Forget,
  WifiToggle,
  DnsManual,
  DnsAutomatic,
  Refresh,
}

impl NetworkApp {
  pub fn new(lang: Lang, theme: Theme, capabilities: Capabilities) -> Self {
    Self {
      page: NetworkPage::Home,
      detail_parent: NetworkPage::Interfaces,
      selected: Selection::default(),
      on_buttons: false,
      button_selected: 0,
      button_from: None,
      snapshot: Default::default(),
      job: None,
      action: None,
      jobs: JobManager::default(),
      lang,
      theme,
      capabilities,
      password: None,
      pending_wifi: None,
      dns_input: None,
      search: None,
      status: None,
      scan: false,
      confirm_forget: false,
      confirmation: ConfirmationState::default(),
      firewall: None,
    }
  }
  pub fn reload(&mut self) {
    if self.page == NetworkPage::Firewall {
      self.ensure_firewall();
      return;
    }
    self.start_refresh(false);
  }
  fn ensure_firewall(&mut self) {
    if self.firewall.is_none() {
      self.firewall = Some(SettingsApp::with_context(
        SettingsPage::Firewall,
        self.lang,
        self.theme.clone(),
      ));
    }
  }
  fn start_refresh(&mut self, scan: bool) {
    if self.job.is_some() || self.action.is_some() {
      return;
    }
    let cap = self.capabilities.clone();
    self.scan = scan;
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: if scan {
        tr(self.lang, "Buscando redes...", "Scanning for networks...")
      } else {
        tr(self.lang, "Atualizando rede...", "Refreshing network...")
      }
      .into(),
    });
    self.job = Some(self.jobs.spawn(move |_| {
      Ok(
        NetworkBackend::new(SystemProcessRunner, cap)
          .snapshot(scan)
          .map_err(|e| e.to_string()),
      )
    }));
  }
  pub fn poll(&mut self) -> bool {
    if self.page == NetworkPage::Firewall {
      return self
        .firewall
        .as_mut()
        .is_some_and(|settings| settings.expire_status());
    }
    let mut changed = false;
    if let Some(job) = &self.job
      && let JobState::Finished(result) = job.try_state()
    {
      self.job = None;
      match result {
        Ok(Ok(snapshot)) => {
          self.snapshot = snapshot;
          self.selected.normalize(self.row_count());
          self.status = Some(StatusMessage {
            kind: StatusKind::Success,
            text: if self.scan {
              tr(self.lang, "Redes atualizadas", "Networks updated")
            } else {
              tr(self.lang, "Rede atualizada", "Network refreshed")
            }
            .into(),
          });
        }
        Ok(Err(error)) | Err(error) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Error,
            text: error,
          })
        }
      }
      self.scan = false;
      changed = true;
    }
    if let Some(job) = &self.action
      && let JobState::Finished(result) = job.try_state()
    {
      self.action = None;
      let (kind, text) = match result {
        Ok(Ok(text)) => (StatusKind::Success, text),
        Ok(Err(error)) | Err(error) => (StatusKind::Error, error),
      };
      self.status = Some(StatusMessage { kind, text });
      self.start_refresh(false);
      changed = true;
    }
    changed
  }
  fn action<F>(&mut self, label: &'static str, task: F)
  where
    F: FnOnce(NetworkBackend<SystemProcessRunner>) -> Result<(), crate::backend::NetworkError>
      + Send
      + 'static,
  {
    if self.action.is_some() || self.job.is_some() {
      return;
    }
    let cap = self.capabilities.clone();
    let success = self.action_label(label);
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: format!("{}...", self.action_label(label)),
    });
    self.action = Some(self.jobs.spawn(move |_| {
      Ok(
        task(NetworkBackend::new(SystemProcessRunner, cap))
          .map(|_| success)
          .map_err(|e| e.to_string()),
      )
    }));
  }
  fn action_label(&self, label: &str) -> String {
    let english = match label {
      "Conectando" => "Connecting",
      "Desconectando" => "Disconnecting",
      "Esquecendo" => "Forgetting",
      "Alterando Wi-Fi" => "Changing Wi-Fi",
      "Aplicando DNS" => "Applying DNS",
      other => other,
    };
    tr(self.lang, label, english).into()
  }
  fn home_pages(&self) -> Vec<NetworkPage> {
    let mut pages = vec![
      NetworkPage::Status,
      NetworkPage::Interfaces,
      NetworkPage::Ethernet,
    ];
    let has_wifi_hardware = self.snapshot.interfaces.iter().any(|i| {
      i.kind.to_ascii_lowercase().contains("wifi")
        || i.kind.to_ascii_lowercase().contains("wireless")
        || i.kind.eq_ignore_ascii_case("802-11-wireless")
    }) || !self.snapshot.wifi.is_empty()
      || has_wifi_hardware_sysfs();
    if has_wifi_hardware {
      pages.push(NetworkPage::Wifi);
    }
    let has_vpn = !self.snapshot.vpn.is_empty()
      || self
        .snapshot
        .interfaces
        .iter()
        .any(|i| i.kind.to_ascii_lowercase().contains("vpn"))
      || self
        .snapshot
        .interfaces
        .iter()
        .any(|i| i.kind.to_ascii_lowercase().contains("wireguard"));
    if has_vpn {
      pages.push(NetworkPage::Vpn);
    }
    pages.extend([NetworkPage::Dns, NetworkPage::Proxy, NetworkPage::Firewall]);
    pages
  }
  fn breadcrumb(&self) -> String {
    let root = tr(self.lang, "Rede", "Network");
    if self.page == NetworkPage::Home {
      root.into()
    } else {
      format!("{root} > {}", self.page_label())
    }
  }
  fn page_label(&self) -> &'static str {
    match self.page {
      NetworkPage::Home => tr(self.lang, "Rede", "Network"),
      NetworkPage::Status => tr(self.lang, "Status", "Status"),
      NetworkPage::Interfaces => tr(self.lang, "Interfaces", "Interfaces"),
      NetworkPage::Wifi => tr(self.lang, "Wi-Fi", "Wi-Fi"),
      NetworkPage::Ethernet => tr(self.lang, "Ethernet", "Ethernet"),
      NetworkPage::Vpn => tr(self.lang, "VPN", "VPN"),
      NetworkPage::Dns => tr(self.lang, "DNS", "DNS"),
      NetworkPage::Proxy => tr(self.lang, "Proxy", "Proxy"),
      NetworkPage::Firewall => tr(self.lang, "Firewall", "Firewall"),
      NetworkPage::Detail(_) => tr(self.lang, "Interface", "Interface"),
    }
  }
  fn connect_wifi(&mut self, index: usize) {
    let Some(network) = self.snapshot.wifi.get(index).cloned() else {
      return;
    };
    if !network.security.is_empty()
      && network.security != "--"
      && !network.security.eq_ignore_ascii_case("none")
    {
      self.pending_wifi = Some(index);
      self.password = Some(String::new());
    } else {
      self.run_wifi(index, None);
    }
  }
  fn run_wifi(&mut self, index: usize, password: Option<String>) {
    let Some(network) = self.snapshot.wifi.get(index).cloned() else {
      return;
    };
    let cap = self.capabilities.clone();
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(self.lang, "Conectando...", "Connecting...").into(),
    });
    self.action = Some(self.jobs.spawn(move |_| {
      Ok(
        NetworkBackend::new(SystemProcessRunner, cap)
          .connect_wifi(&network.ssid, password.as_deref())
          .map(|_| "Conectado".into())
          .map_err(|e| e.to_string()),
      )
    }));
  }
  fn visible_wifi(&self) -> Vec<usize> {
    let mut indexes: Vec<usize> = self
      .snapshot
      .wifi
      .iter()
      .enumerate()
      .filter(|(_, w)| {
        self.search.as_ref().is_none_or(|q| {
          w.ssid
            .to_ascii_lowercase()
            .contains(&q.to_ascii_lowercase())
        })
      })
      .map(|(i, _)| i)
      .collect();
    indexes.sort_by_key(|i| {
      let w = &self.snapshot.wifi[*i];
      (
        !w.connected,
        std::cmp::Reverse(w.signal.unwrap_or(0)),
        w.ssid.clone(),
      )
    });
    indexes
  }
  fn visible_interfaces(&self) -> Vec<usize> {
    self
      .snapshot
      .interfaces
      .iter()
      .enumerate()
      .filter(|(_, i)| {
        self.page != NetworkPage::Ethernet || i.kind.to_ascii_lowercase().contains("ethernet")
      })
      .map(|(i, _)| i)
      .collect()
  }
  fn visible_vpn(&self) -> Vec<usize> {
    self
      .snapshot
      .vpn
      .iter()
      .enumerate()
      .filter(|(_, v)| {
        self.search.as_ref().is_none_or(|q| {
          v.name
            .to_ascii_lowercase()
            .contains(&q.to_ascii_lowercase())
        })
      })
      .map(|(i, _)| i)
      .collect()
  }
  fn current_index(&self) -> Option<usize> {
    match self.page {
      NetworkPage::Wifi => self.visible_wifi().get(self.selected.index).copied(),
      NetworkPage::Interfaces | NetworkPage::Ethernet => {
        self.visible_interfaces().get(self.selected.index).copied()
      }
      NetworkPage::Detail(index) => Some(index),
      NetworkPage::Vpn => self.visible_vpn().get(self.selected.index).copied(),
      _ => None,
    }
  }
  pub fn handle(&mut self, key: KeyCode) -> bool {
    if self.page == NetworkPage::Firewall {
      return self.handle_firewall(key);
    }
    if self.confirm_forget {
      match self.confirmation.handle(key) {
        ConfirmationOutcome::Confirmed => {
          self.confirm_forget = false;
          self.confirmation = ConfirmationState::default();
          self.perform_forget();
        }
        ConfirmationOutcome::Cancelled => {
          self.confirm_forget = false;
          self.confirmation = ConfirmationState::default();
        }
        ConfirmationOutcome::Pending => {}
      }
      return false;
    }
    if let Some(password) = &mut self.password {
      match key {
        KeyCode::Esc => {
          self.password = None;
          self.pending_wifi = None;
        }
        KeyCode::Backspace => {
          password.pop();
        }
        KeyCode::Char(c) => password.push(c),
        KeyCode::Enter => {
          let secret = std::mem::take(password);
          let index = self.pending_wifi.take();
          self.password = None;
          if let Some(index) = index {
            self.run_wifi(index, Some(secret));
          }
        }
        _ => {}
      }
      return false;
    }
    if let Some(input) = &mut self.dns_input {
      match key {
        KeyCode::Esc => self.dns_input = None,
        KeyCode::Backspace => {
          input.pop();
        }
        KeyCode::Char(c) if !c.is_control() => input.push(c),
        KeyCode::Enter => {
          let value = std::mem::take(input);
          self.dns_input = None;
          self.apply_dns(&value);
        }
        _ => {}
      }
      return false;
    }
    if let Some(search) = &mut self.search {
      match key {
        KeyCode::Esc => self.search = None,
        KeyCode::Backspace => {
          search.pop();
        }
        KeyCode::Char(c) => search.push(c),
        KeyCode::Enter => {}
        _ => {}
      }
      self.selected.index = 0;
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
    match key {
      KeyCode::Esc | KeyCode::Left => {
        if self.page == NetworkPage::Home {
          return true;
        }
        if matches!(self.page, NetworkPage::Detail(_)) {
          self.page = self.detail_parent;
        } else {
          self.page = NetworkPage::Home;
        }
        self.selected.index = 0;
        self.on_buttons = false;
        self.button_from = None;
      }
      KeyCode::Tab | KeyCode::BackTab => self.toggle_buttons(key == KeyCode::BackTab),
      KeyCode::Char('r') => self.start_refresh(self.page == NetworkPage::Wifi),
      KeyCode::Char('/')
        if matches!(
          self.page,
          NetworkPage::Interfaces | NetworkPage::Ethernet | NetworkPage::Wifi | NetworkPage::Vpn
        ) =>
      {
        self.search = Some(String::new())
      }
      key if self.selected.handle(key, self.row_count(), 8) => {}
      KeyCode::Enter | KeyCode::Right => self.open(),
      _ => {}
    }
    false
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
    let actions = self.buttons();
    let Some((action, _)) = actions.get(self.button_selected) else {
      return;
    };
    match *action {
      ActionButton::Connect => self.connect_selected(),
      ActionButton::Disconnect => self.disconnect_selected(),
      ActionButton::Forget => self.forget_selected(),
      ActionButton::WifiToggle => self.toggle_wifi(),
      ActionButton::DnsManual => self.dns_input = Some(String::new()),
      ActionButton::DnsAutomatic => self.apply_dns(""),
      ActionButton::Refresh => self.start_refresh(self.page == NetworkPage::Wifi),
    }
  }
  fn buttons(&self) -> Vec<(ActionButton, Button)> {
    match self.page {
      NetworkPage::Status => vec![
        (
          ActionButton::WifiToggle,
          Button::new(
            if self.snapshot.wifi_enabled == Some(false) {
              tr(self.lang, "Ligar Wi-Fi", "Enable Wi-Fi")
            } else {
              tr(self.lang, "Desligar Wi-Fi", "Disable Wi-Fi")
            },
            ButtonKind::Secondary,
          ),
        ),
        (
          ActionButton::Refresh,
          Button::new(tr(self.lang, "Atualizar", "Refresh"), ButtonKind::Secondary),
        ),
      ],
      NetworkPage::Interfaces | NetworkPage::Ethernet | NetworkPage::Vpn => vec![
        (
          ActionButton::Connect,
          Button::new(tr(self.lang, "Conectar", "Connect"), ButtonKind::Primary),
        ),
        (
          ActionButton::Disconnect,
          Button::new(
            tr(self.lang, "Desconectar", "Disconnect"),
            ButtonKind::Danger,
          ),
        ),
        (
          ActionButton::Refresh,
          Button::new(tr(self.lang, "Atualizar", "Refresh"), ButtonKind::Secondary),
        ),
      ],
      NetworkPage::Wifi => vec![
        (
          ActionButton::Connect,
          Button::new(tr(self.lang, "Conectar", "Connect"), ButtonKind::Primary),
        ),
        (
          ActionButton::Disconnect,
          Button::new(
            tr(self.lang, "Desconectar", "Disconnect"),
            ButtonKind::Secondary,
          ),
        ),
        (
          ActionButton::Forget,
          Button::new(tr(self.lang, "Esquecer", "Forget"), ButtonKind::Danger),
        ),
        (
          ActionButton::Refresh,
          Button::new(tr(self.lang, "Atualizar", "Refresh"), ButtonKind::Secondary),
        ),
      ],
      NetworkPage::Dns => vec![
        (
          ActionButton::DnsManual,
          Button::new(
            tr(self.lang, "DNS manual", "Manual DNS"),
            ButtonKind::Secondary,
          ),
        ),
        (
          ActionButton::DnsAutomatic,
          Button::new(
            tr(self.lang, "DNS automático", "Automatic DNS"),
            ButtonKind::Primary,
          ),
        ),
        (
          ActionButton::Refresh,
          Button::new(tr(self.lang, "Atualizar", "Refresh"), ButtonKind::Secondary),
        ),
      ],
      NetworkPage::Proxy | NetworkPage::Detail(_) => vec![(
        ActionButton::Refresh,
        Button::new(tr(self.lang, "Atualizar", "Refresh"), ButtonKind::Secondary),
      )],
      _ => Vec::new(),
    }
  }
  fn footer_hints(&self) -> &'static str {
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
    let home = tr(
      self.lang,
      "↑/↓ Navegar   →/Enter Abrir   ←/Esc Voltar   r Atualizar   ? Ajuda",
      "↑/↓ Navigate   →/Enter Open   ←/Esc Back   r Refresh   ? Help",
    );
    if self.page == NetworkPage::Home {
      home
    } else if self.page == NetworkPage::Firewall {
      ""
    } else if !self.buttons().is_empty() {
      action
    } else {
      readonly
    }
  }
  fn handle_firewall(&mut self, key: KeyCode) -> bool {
    let Some(settings) = &mut self.firewall else {
      self.page = NetworkPage::Home;
      self.selected.index = 0;
      return false;
    };
    let event = Event::Key(KeyEvent::new_with_kind_and_state(
      key,
      KeyModifiers::NONE,
      KeyEventKind::Press,
      KeyEventState::NONE,
    ));
    settings_event::handle(settings, event);
    if settings.page() == SettingsPage::Main {
      self.firewall = None;
      self.page = NetworkPage::Home;
      self.selected.index = 0;
    }
    false
  }
  fn open(&mut self) {
    if self.page == NetworkPage::Home {
      let pages = self.home_pages();
      self.page = pages
        .get(self.selected.index)
        .copied()
        .unwrap_or(NetworkPage::Home);
      self.selected.index = 0;
      if self.page == NetworkPage::Firewall {
        self.reload();
      } else {
        self.start_refresh(self.page == NetworkPage::Wifi);
      }
      return;
    }
    if matches!(self.page, NetworkPage::Interfaces | NetworkPage::Ethernet) {
      if let Some(i) = self.current_index() {
        self.detail_parent = self.page;
        self.page = NetworkPage::Detail(i);
        self.selected.index = 0;
      }
    } else if self.page == NetworkPage::Wifi {
      if let Some(i) = self.current_index() {
        self.connect_wifi(i);
      }
    } else if self.page == NetworkPage::Vpn {
      self.connect_selected();
    }
  }
  fn connect_selected(&mut self) {
    match self.page {
      NetworkPage::Wifi => {
        if let Some(i) = self.current_index() {
          self.connect_wifi(i);
        }
      }
      NetworkPage::Interfaces | NetworkPage::Ethernet | NetworkPage::Detail(_) => {
        if let Some(i) = self.current_index() {
          if let Some(name) = self
            .snapshot
            .interfaces
            .get(i)
            .and_then(|v| v.connection.clone())
          {
            self.action("Conectando", move |b| b.connection_action(&name, "up"));
          } else if let Some(name) = self.snapshot.interfaces.get(i).map(|v| v.name.clone()) {
            self.action("Conectando", move |b| b.device_action(&name, "connect"));
          }
        }
      }
      NetworkPage::Vpn => {
        if let Some(i) = self
          .visible_vpn()
          .get(self.selected.index)
          .and_then(|i| self.snapshot.vpn.get(*i))
        {
          let name = i.name.clone();
          self.action("Conectando", move |b| b.connection_action(&name, "up"));
        }
      }
      _ => {}
    }
  }
  fn disconnect_selected(&mut self) {
    match self.page {
      NetworkPage::Wifi => {
        if let Some(i) = self.current_index()
          && let Some(name) = self.snapshot.wifi.get(i).map(|v| v.ssid.clone())
        {
          self.action("Desconectando", move |b| b.connection_action(&name, "down"));
        }
      }
      NetworkPage::Interfaces | NetworkPage::Ethernet => {
        if let Some(i) = self.current_index() {
          let name = self.snapshot.interfaces[i].name.clone();
          self.action("Desconectando", move |b| {
            b.device_action(&name, "disconnect")
          });
        }
      }
      NetworkPage::Vpn => {
        if let Some(i) = self
          .visible_vpn()
          .get(self.selected.index)
          .and_then(|i| self.snapshot.vpn.get(*i))
        {
          let name = i.name.clone();
          self.action("Desconectando", move |b| b.connection_action(&name, "down"));
        }
      }
      _ => {}
    }
  }
  fn forget_selected(&mut self) {
    if let Some(i) = self.current_index()
      && self.snapshot.wifi.get(i).is_some()
    {
      self.confirmation = ConfirmationState::default();
      self.confirm_forget = true;
    }
  }
  fn perform_forget(&mut self) {
    if let Some(i) = self.current_index()
      && let Some(name) = self
        .snapshot
        .wifi
        .get(i)
        .map(|network| network.ssid.clone())
    {
      self.action("Esquecendo", move |backend| {
        backend.connection_action(&name, "delete")
      });
    }
  }
  fn toggle_wifi(&mut self) {
    let enabled = !self.snapshot.wifi_enabled.unwrap_or(true);
    self.action("Alterando Wi-Fi", move |b| b.set_wifi_enabled(enabled));
  }
  fn apply_dns(&mut self, raw: &str) {
    let values: Vec<String> = raw
      .split(|c: char| c == ',' || c.is_whitespace())
      .filter(|v| !v.is_empty())
      .map(str::to_owned)
      .collect();
    if values
      .iter()
      .all(|v| crate::backend::valid_ipv4(v) || crate::backend::valid_ipv6(v))
    {
      if let Some(connection) = self
        .snapshot
        .interfaces
        .iter()
        .find(|v| v.state == "connected")
        .and_then(|v| v.connection.clone())
      {
        let automatic = values.is_empty();
        self.apply_dns_privileged(connection, values, automatic);
      }
    } else {
      self.status = Some(StatusMessage {
        kind: StatusKind::Error,
        text: tr(self.lang, "Servidor DNS inválido.", "Invalid DNS server.").into(),
      });
    }
  }
  fn apply_dns_privileged(&mut self, connection: String, values: Vec<String>, automatic: bool) {
    if self.action.is_some() || self.job.is_some() {
      return;
    }
    let executable = match std::env::current_exe() {
      Ok(path) => path.to_string_lossy().into_owned(),
      Err(error) => {
        self.status = Some(StatusMessage {
          kind: StatusKind::Error,
          text: error.to_string(),
        });
        return;
      }
    };
    let lang = self.lang;
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: self.action_label("Aplicando DNS"),
    });
    self.action = Some(self.jobs.spawn(move |_| {
      Ok(apply_dns_privileged(
        executable, lang, connection, automatic, values,
      ))
    }));
  }
  fn row_count(&self) -> usize {
    match self.page {
      NetworkPage::Home => self.home_pages().len(),
      NetworkPage::Interfaces | NetworkPage::Ethernet => self.visible_interfaces().len(),
      NetworkPage::Detail(_) => 0,
      NetworkPage::Wifi => self.visible_wifi().len(),
      NetworkPage::Vpn => self.visible_vpn().len(),
      NetworkPage::Status | NetworkPage::Dns | NetworkPage::Proxy | NetworkPage::Firewall => 0,
    }
  }
  pub fn draw(&mut self, f: &mut Frame) {
    if self.page == NetworkPage::Firewall {
      if let Some(settings) = &mut self.firewall {
        settings_ui::draw(settings, f);
      }
      return;
    }
    let area = f.area();
    let readonly_page = matches!(self.page, NetworkPage::Status | NetworkPage::Detail(_));
    let body = shell(
      f,
      area,
      &self.theme,
      &self.breadcrumb(),
      self.footer_hints(),
    );
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
    let rows = self.rows();
    if readonly_page {
      readonly(
        f,
        body,
        &self.theme,
        &rows.into_iter().map(Line::from).collect::<Vec<_>>(),
      );
    } else {
      list(
        f,
        body,
        &self.theme,
        &rows,
        self.selected.index.min(rows.len().saturating_sub(1)),
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
    if let Some(input) = &self.dns_input {
      let popup = argvus_tui::chrome::centered(area, 48, 7);
      f.render_widget(Clear, popup);
      f.render_widget(
        Paragraph::new(vec![
          Line::from(tr(
            self.lang,
            "Servidores DNS (separados por vírgula):",
            "DNS servers (comma separated):",
          )),
          Line::from(format!("{input}_")),
          Line::from(tr(
            self.lang,
            "Enter aplicar   Esc cancelar",
            "Enter apply   Esc cancel",
          )),
        ])
        .block(Block::bordered().title(tr(self.lang, "Servidores DNS", "DNS servers"))),
        popup,
      );
    } else if let Some(password) = &self.password {
      let popup = argvus_tui::chrome::centered(area, 48, 7);
      f.render_widget(Clear, popup);
      let shown = "•".repeat(argvus_tui::text::display_width(password));
      f.render_widget(
        Paragraph::new(vec![
          Line::from(tr(
            self.lang,
            "Senha da rede Wi-Fi:",
            "Wi-Fi network password:",
          )),
          Line::from(format!("{shown}_")),
          Line::from(tr(
            self.lang,
            "Enter conectar   Esc cancelar",
            "Enter connect   Esc cancel",
          )),
        ])
        .block(Block::bordered().title(tr(self.lang, "Senha Wi-Fi", "Wi-Fi password"))),
        popup,
      );
    }
    if self.confirm_forget {
      let name = self
        .current_index()
        .and_then(|i| self.snapshot.wifi.get(i))
        .map(|w| terminal_text(&w.ssid))
        .unwrap_or_default();
      draw_confirmation(
        f,
        area,
        &self.theme,
        ConfirmationDialog {
          title: tr(self.lang, "Esquecer rede", "Forget network"),
          message: &format!(
            "{} {}?",
            tr(self.lang, "Esquecer a rede", "Forget network"),
            name
          ),
          confirm_label: tr(self.lang, "Esquecer", "Forget"),
          cancel_label: tr(self.lang, "Cancelar", "Cancel"),
          confirm_selected: self.confirmation.confirm_selected,
        },
      );
    }
    if let Some(status_message) = &self.status {
      status(f, body, &self.theme, status_message);
    }
  }
  fn rows(&self) -> Vec<String> {
    match self.page {
      NetworkPage::Home => self.home_rows(),
      NetworkPage::Status => {
        let active = self
          .snapshot
          .interfaces
          .iter()
          .find(|i| i.state == "connected");
        vec![
          format!(
            " {} {}",
            AppConfig::icon("🌐"),
            tr(self.lang, "CONECTIVIDADE", "CONNECTIVITY")
          ),
          format!(
            "   {:<14} {}",
            tr(self.lang, "Estado:", "State:"),
            self.snapshot.connectivity
          ),
          format!(
            "   {:<14} {}",
            tr(self.lang, "Interface:", "Interface:"),
            active.map(|i| i.name.as_str()).unwrap_or("—")
          ),
          format!(
            "   {:<14} {}",
            tr(self.lang, "IPv4:", "IPv4:"),
            active
              .and_then(|i| i.ipv4.first())
              .map(String::as_str)
              .unwrap_or("—")
          ),
          format!(
            "   {:<14} {}",
            tr(self.lang, "IPv6:", "IPv6:"),
            active
              .map(|i| i.ipv6.join(", "))
              .filter(|v| !v.is_empty())
              .unwrap_or_else(|| "—".into())
          ),
          format!(
            "   {:<14} {}",
            tr(self.lang, "Gateway:", "Gateway:"),
            active.and_then(|i| i.gateway.as_deref()).unwrap_or("—")
          ),
          "".into(),
          format!(
            " {} {}",
            AppConfig::icon("📶"),
            tr(self.lang, "CONEXÕES", "CONNECTIONS")
          ),
          format!(
            "   {:<14} {}",
            tr(self.lang, "Wi-Fi:", "Wi-Fi:"),
            if self.snapshot.wifi_enabled == Some(false) {
              tr(self.lang, "Desligado", "Disabled")
            } else {
              self
                .snapshot
                .wifi
                .iter()
                .find(|w| w.connected)
                .map(|w| w.ssid.as_str())
                .unwrap_or_else(|| tr(self.lang, "Inativo", "Inactive"))
            }
          ),
          format!(
            "   {:<14} {}",
            tr(self.lang, "DNS:", "DNS:"),
            if self.snapshot.dns.servers.is_empty() {
              "—".into()
            } else {
              self.snapshot.dns.servers.join(", ")
            }
          ),
          format!(
            "   {:<14} {}",
            tr(self.lang, "VPN:", "VPN:"),
            self
              .snapshot
              .vpn
              .iter()
              .find(|v| v.active)
              .map(|v| terminal_text(&v.name))
              .unwrap_or_else(|| tr(self.lang, "Inativa", "Inactive").into())
          ),
        ]
      }
      NetworkPage::Interfaces | NetworkPage::Ethernet => self
        .visible_interfaces()
        .into_iter()
        .map(|i| {
          let v = &self.snapshot.interfaces[i];
          let status_badge = if v.state == "connected" {
            format!("   ★ {}", tr(self.lang, "Conectado", "Connected"))
          } else {
            String::new()
          };
          let conn = v
            .connection
            .clone()
            .unwrap_or_else(|| tr(self.lang, "sem conexão", "no connection").into());
          format!("{} ({})  ·  {}{}", v.name, v.kind, conn, status_badge)
        })
        .collect(),
      NetworkPage::Detail(i) => self
        .snapshot
        .interfaces
        .get(i)
        .map(|v| {
          let status = if v.state == "connected" {
            format!("★ {}", tr(self.lang, "Conectado", "Connected"))
          } else {
            v.state.clone()
          };
          vec![
            format!(
              " {} {}",
              AppConfig::icon("🔌"),
              tr(self.lang, "INTERFACE DE REDE", "NETWORK INTERFACE")
            ),
            format!("   {:<12} {}", tr(self.lang, "Nome:", "Name:"), v.name),
            format!("   {:<12} {}", tr(self.lang, "Tipo:", "Type:"), v.kind),
            format!("   {:<12} {}", tr(self.lang, "Status:", "Status:"), status),
            format!(
              "   {:<12} {}",
              tr(self.lang, "Driver:", "Driver:"),
              v.driver.as_deref().unwrap_or("—")
            ),
            "".into(),
            format!(
              "   {:<12} {}",
              tr(self.lang, "Operstate:", "Operstate:"),
              v.operstate
            ),
            format!(
              "   {:<12} {}",
              tr(self.lang, "MAC:", "MAC:"),
              v.mac.as_deref().unwrap_or("—")
            ),
            format!(
              "   {:<12} {}",
              tr(self.lang, "IPv4:", "IPv4:"),
              if v.ipv4.is_empty() {
                "—".into()
              } else {
                v.ipv4.join(", ")
              }
            ),
            format!(
              "   {:<12} {}",
              tr(self.lang, "IPv6:", "IPv6:"),
              if v.ipv6.is_empty() {
                "—".into()
              } else {
                v.ipv6.join(", ")
              }
            ),
            format!(
              "   {:<12} {}",
              tr(self.lang, "MTU:", "MTU:"),
              v.mtu.map(|m| m.to_string()).unwrap_or_else(|| "—".into())
            ),
          ]
        })
        .unwrap_or_default(),
      NetworkPage::Wifi => self
        .visible_wifi()
        .into_iter()
        .map(|i| {
          let w = &self.snapshot.wifi[i];
          let badge = if w.connected {
            format!("   ★ {}", tr(self.lang, "Conectado", "Connected"))
          } else if w.known {
            format!("   ● {}", tr(self.lang, "Salva", "Saved"))
          } else {
            String::new()
          };
          let signal = w
            .signal
            .map(|s| format!("{s}%"))
            .unwrap_or_else(|| "—".into());
          format!(
            "{}  ·  {}  ·  {}{}",
            terminal_text(&w.ssid),
            signal,
            w.security,
            badge
          )
        })
        .collect(),
      NetworkPage::Vpn => self
        .visible_vpn()
        .into_iter()
        .map(|i| {
          let v = &self.snapshot.vpn[i];
          let badge = if v.active {
            format!("   ★ {}", tr(self.lang, "Ativa", "Active"))
          } else {
            String::new()
          };
          format!("{} ({}){}", terminal_text(&v.name), v.kind, badge)
        })
        .collect(),
      NetworkPage::Dns => vec![
        format!(
          " {} {}",
          AppConfig::icon("🔎"),
          tr(self.lang, "CONFIGURAÇÃO DE DNS", "DNS CONFIGURATION")
        ),
        format!(
          "   {:<16} {}",
          tr(self.lang, "Fonte:", "Source:"),
          terminal_text(&self.snapshot.dns.source)
        ),
        format!(
          "   {:<16} {}",
          tr(self.lang, "Servidores:", "Servers:"),
          if self.snapshot.dns.servers.is_empty() {
            "—".into()
          } else {
            self.snapshot.dns.servers.join(", ")
          }
        ),
        format!(
          "   {:<16} {}",
          tr(self.lang, "Search Domains:", "Search Domains:"),
          if self.snapshot.dns.search_domains.is_empty() {
            "—".into()
          } else {
            self.snapshot.dns.search_domains.join(", ")
          }
        ),
      ],
      NetworkPage::Proxy => {
        let http = self.snapshot.proxy.http.as_deref().unwrap_or("—");
        let https = self.snapshot.proxy.https.as_deref().unwrap_or("—");
        let all = self.snapshot.proxy.all.as_deref().unwrap_or("—");
        let no_proxy = self.snapshot.proxy.no_proxy.as_deref().unwrap_or("—");
        vec![
          format!(
            " {} {}",
            AppConfig::icon("🛡"),
            tr(self.lang, "CONFIGURAÇÃO DE PROXY", "PROXY CONFIGURATION")
          ),
          format!("   {:<14} {}", "HTTP_PROXY:", http),
          format!("   {:<14} {}", "HTTPS_PROXY:", https),
          format!("   {:<14} {}", "ALL_PROXY:", all),
          format!("   {:<14} {}", "NO_PROXY:", no_proxy),
        ]
      }
      NetworkPage::Firewall => Vec::new(),
    }
  }
  fn home_rows(&self) -> Vec<String> {
    let active = self
      .snapshot
      .interfaces
      .iter()
      .find(|i| i.state == "connected");
    let active_iface = active
      .map(|i| i.name.as_str())
      .unwrap_or_else(|| tr(self.lang, "Nenhuma", "None"));
    let ipv4 = active
      .and_then(|i| i.ipv4.first())
      .map(String::as_str)
      .unwrap_or("—");
    let wifi_status = if self.snapshot.wifi_enabled == Some(false) {
      tr(self.lang, "Desligado", "Disabled")
    } else {
      self
        .snapshot
        .wifi
        .iter()
        .find(|w| w.connected)
        .map(|w| w.ssid.as_str())
        .unwrap_or_else(|| tr(self.lang, "Desconectado", "Disconnected"))
    };
    let active_vpn = self
      .snapshot
      .vpn
      .iter()
      .find(|v| v.active)
      .map(|v| terminal_text(&v.name))
      .unwrap_or_else(|| tr(self.lang, "Inativa", "Inactive").into());
    let dns_count = self.snapshot.dns.servers.len();
    let dns_str = if dns_count > 0 {
      self.snapshot.dns.servers.join(", ")
    } else {
      tr(self.lang, "Automático", "Automatic").into()
    };
    let proxy_str = if self.snapshot.proxy.http.is_some() || self.snapshot.proxy.https.is_some() {
      tr(self.lang, "Ativo", "Active")
    } else {
      tr(self.lang, "Desativado", "Disabled")
    };

    let pages = self.home_pages();
    pages
      .iter()
      .map(|page| match page {
        NetworkPage::Status => format!(
          "{} {}  ·  {} · {}",
          AppConfig::icon("🌐"),
          tr(self.lang, "Status", "Status"),
          self.snapshot.connectivity,
          active_iface
        ),
        NetworkPage::Interfaces => format!(
          "{} {}  ·  {} ({})",
          AppConfig::icon("🔌"),
          tr(self.lang, "Interfaces", "Interfaces"),
          self.snapshot.interfaces.len(),
          ipv4
        ),
        NetworkPage::Ethernet => format!(
          "{} {}  ·  {}",
          AppConfig::icon("🖧"),
          tr(self.lang, "Ethernet", "Ethernet"),
          self
            .snapshot
            .interfaces
            .iter()
            .filter(|i| i.kind.to_ascii_lowercase().contains("ethernet"))
            .count()
        ),
        NetworkPage::Wifi => format!(
          "{} {}  ·  {}",
          AppConfig::icon("📶"),
          tr(self.lang, "Wi-Fi", "Wi-Fi"),
          wifi_status
        ),
        NetworkPage::Vpn => format!(
          "{} {}  ·  {}",
          AppConfig::icon("🔒"),
          tr(self.lang, "VPN", "VPN"),
          active_vpn
        ),
        NetworkPage::Dns => format!(
          "{} {}  ·  {}",
          AppConfig::icon("🔎"),
          tr(self.lang, "DNS", "DNS"),
          dns_str
        ),
        NetworkPage::Proxy => format!(
          "{} {}  ·  {}",
          AppConfig::icon("🛡"),
          tr(self.lang, "Proxy", "Proxy"),
          proxy_str
        ),
        NetworkPage::Firewall => format!(
          "{} {}",
          AppConfig::icon("🔥"),
          tr(self.lang, "Firewall", "Firewall")
        ),
        NetworkPage::Home | NetworkPage::Detail(_) => String::new(),
      })
      .collect()
  }
}

fn apply_dns_privileged(
  executable: String,
  lang: Lang,
  connection: String,
  automatic: bool,
  values: Vec<String>,
) -> Result<String, String> {
  let operation = SystemSettingsOperation::new(SystemProcessRunner, executable);
  let request = PrivilegedRequest::new(
    "network",
    "dns",
    vec![
      connection,
      if automatic {
        "automatic".into()
      } else {
        "manual".into()
      },
      values.join(","),
    ],
  )?;
  let output = operation.execute(&request)?;
  if output.status == Some(0) {
    Ok(tr(lang, "DNS aplicado", "DNS applied").into())
  } else {
    Err(terminal_text(&String::from_utf8_lossy(&output.stderr)))
  }
}

fn has_wifi_hardware_sysfs() -> bool {
  std::fs::read_dir("/sys/class/net")
    .ok()
    .into_iter()
    .flatten()
    .filter_map(std::result::Result::ok)
    .any(|entry| entry.path().join("wireless").is_dir())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::InterfaceInfo;
  use argvus_control_center_core::capabilities::Capabilities;
  use argvus_i18n::Lang;

  fn app() -> NetworkApp {
    NetworkApp::new(
      Lang::En,
      argvus_theme::Theme::load(),
      Capabilities::default(),
    )
  }

  #[test]
  fn home_opens_firewall_and_back_returns_home() {
    let mut app = app();
    for _ in 0..5 {
      app.handle(KeyCode::Down);
    }
    app.handle(KeyCode::Enter);
    assert_eq!(app.page, NetworkPage::Firewall);
    assert!(app.firewall.is_some());
    assert!(!app.handle(KeyCode::Esc));
    assert_eq!(app.page, NetworkPage::Home);
    assert!(app.firewall.is_none());
  }

  #[test]
  fn wifi_is_only_listed_when_a_card_exists() {
    let mut app = app();
    assert!(!app.home_pages().contains(&NetworkPage::Wifi));
    app.snapshot.interfaces.push(InterfaceInfo {
      name: "wlan0".into(),
      kind: "wifi".into(),
      ..Default::default()
    });
    assert!(app.home_pages().contains(&NetworkPage::Wifi));
  }

  #[test]
  fn vpn_is_only_listed_when_vpn_is_detected() {
    let mut app = app();
    assert!(!app.home_pages().contains(&NetworkPage::Vpn));
    app.snapshot.vpn.push(crate::model::VpnConnection {
      name: "company".into(),
      kind: "vpn".into(),
      active: false,
    });
    assert!(app.home_pages().contains(&NetworkPage::Vpn));
    app.snapshot.vpn.clear();
    app.snapshot.interfaces.push(InterfaceInfo {
      name: "wg0".into(),
      kind: "wireguard".into(),
      ..Default::default()
    });
    assert!(app.home_pages().contains(&NetworkPage::Vpn));
  }

  #[test]
  fn firewall_page_delegates_to_settings() {
    let mut app = app();
    app.page = NetworkPage::Firewall;
    app.reload();
    assert!(app.firewall.is_some());
    assert!(!app.handle(KeyCode::Esc));
    assert_eq!(app.page, NetworkPage::Home);
    assert!(app.handle(KeyCode::Esc));
  }

  #[test]
  fn initial_firewall_route_creates_embedded_context() {
    let mut app = app();
    app.page = NetworkPage::Firewall;
    app.reload();
    assert!(app.firewall.is_some());
    assert_eq!(
      app.firewall.as_ref().unwrap().page(),
      SettingsPage::Firewall
    );
  }

  #[test]
  fn network_home_rows_act_as_a_status_dashboard() {
    let mut app = app();
    app.snapshot.connectivity = "full".into();
    app.snapshot.interfaces = vec![InterfaceInfo {
      name: "eno1".into(),
      kind: "ethernet".into(),
      state: "connected".into(),
      ipv4: vec!["190.168.1.100".into()],
      ..Default::default()
    }];
    app.snapshot.wifi_enabled = Some(true);
    let rows = app.rows();
    assert_eq!(rows.len(), 6);
    assert!(rows[0].contains("Status") && rows[0].contains("full") && rows[0].contains("eno1"));
    assert!(rows[1].contains("Interfaces") && rows[1].contains("190.168.1.100"));
    assert!(rows[2].contains("Ethernet"));
    assert!(rows[3].contains("DNS"));
  }

  #[test]
  fn interface_detail_has_only_refresh_button_and_stays_informational() {
    let mut app = app();
    app.snapshot.interfaces.push(InterfaceInfo {
      name: "eno1".into(),
      kind: "ethernet".into(),
      state: "connected".into(),
      connection: Some("Wired connection 1".into()),
      ..Default::default()
    });
    app.page = NetworkPage::Detail(0);
    let actions: Vec<ActionButton> = app.buttons().into_iter().map(|(a, _)| a).collect();
    assert_eq!(actions, vec![ActionButton::Refresh]);
    assert!(!app.footer_hints().contains("Conectar"));
    assert!(!app.footer_hints().contains("Connect"));
    assert!(
      app.rows()[0].contains("NETWORK INTERFACE") || app.rows()[0].contains("INTERFACE DE REDE")
    );
    assert!(app.rows().iter().any(|r| r.contains("eno1")));
  }

  #[test]
  fn list_pages_expose_action_buttons_with_refresh_last() {
    let mut app = app();
    app.page = NetworkPage::Interfaces;
    let actions: Vec<ActionButton> = app.buttons().into_iter().map(|(a, _)| a).collect();
    assert_eq!(
      actions,
      vec![
        ActionButton::Connect,
        ActionButton::Disconnect,
        ActionButton::Refresh
      ]
    );

    app.page = NetworkPage::Wifi;
    let actions: Vec<ActionButton> = app.buttons().into_iter().map(|(a, _)| a).collect();
    assert_eq!(
      actions,
      vec![
        ActionButton::Connect,
        ActionButton::Disconnect,
        ActionButton::Forget,
        ActionButton::Refresh
      ]
    );

    app.page = NetworkPage::Dns;
    let actions: Vec<ActionButton> = app.buttons().into_iter().map(|(a, _)| a).collect();
    assert_eq!(
      actions,
      vec![
        ActionButton::DnsManual,
        ActionButton::DnsAutomatic,
        ActionButton::Refresh
      ]
    );

    app.page = NetworkPage::Status;
    let actions: Vec<ActionButton> = app.buttons().into_iter().map(|(a, _)| a).collect();
    assert_eq!(
      actions,
      vec![ActionButton::WifiToggle, ActionButton::Refresh]
    );
  }

  #[test]
  fn tab_cycles_focus_between_list_and_buttons_and_restores_selection() {
    let mut app = app();
    app.page = NetworkPage::Interfaces;
    app.snapshot.interfaces = vec![
      InterfaceInfo {
        name: "eno1".into(),
        ..Default::default()
      },
      InterfaceInfo {
        name: "wlan0".into(),
        ..Default::default()
      },
    ];
    app.handle(KeyCode::Down);
    assert_eq!(app.selected.index, 1);
    app.handle(KeyCode::Tab);
    assert!(app.on_buttons);
    assert_eq!(app.button_selected, 0);
    app.handle(KeyCode::Right);
    assert_eq!(app.button_selected, 1);
    app.handle(KeyCode::Tab);
    assert!(!app.on_buttons);
    assert_eq!(app.selected.index, 1);
  }

  #[test]
  fn backtab_lands_on_last_button_and_escape_returns_to_list() {
    let mut app = app();
    app.page = NetworkPage::Wifi;
    app.handle(KeyCode::BackTab);
    assert!(app.on_buttons);
    assert_eq!(app.button_selected, 3);
    app.handle(KeyCode::Esc);
    assert!(!app.on_buttons);
  }

  #[test]
  fn entering_button_bar_on_readonly_status_keeps_focus_there() {
    let mut app = app();
    app.page = NetworkPage::Status;
    app.handle(KeyCode::Tab);
    assert!(app.on_buttons);
  }

  #[test]
  fn detail_page_and_proxy_expose_only_a_refresh_button() {
    let mut app = app();
    app.page = NetworkPage::Detail(0);
    app.handle(KeyCode::Tab);
    assert!(app.on_buttons);
    app.handle(KeyCode::Tab);
    assert!(!app.on_buttons);
    app.page = NetworkPage::Proxy;
    app.handle(KeyCode::Tab);
    assert!(app.on_buttons);
  }

  #[test]
  fn interfaces_page_renders_button_bar_instead_of_footer_actions() {
    let mut app = app();
    app.page = NetworkPage::Interfaces;
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(90, 25)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    let text = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(text.contains("[ Connect ]"));
    assert!(text.contains("[ Disconnect ]"));
    assert!(text.contains("Tab Actions"));
    assert!(!text.contains("c Connect"));
    assert!(!text.contains("x Disconnect"));
  }
}
