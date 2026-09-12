use crate::{
  agent::AgentHandle,
  backend::BluetoothBackend,
  model::{
    AgentEvent, AgentPrompt, AgentReply, BluetoothPage, BluetoothSnapshot, Device, PromptStep,
  },
};
use argvus_control_center_core::{
  capabilities::Capabilities,
  jobs::{JobHandle, JobManager, JobState},
  process::SystemProcessRunner,
};
use argvus_i18n::{Lang, tr};
use argvus_theme::Theme;
use argvus_tui::{
  buttons::{Button, ButtonKind},
  chrome::centered,
  components::{
    ConfirmationDialog, ConfirmationOutcome, ConfirmationState, StatusKind, StatusMessage,
    draw_confirmation, draw_status,
  },
  page::{Selection, list, shell},
};
use crossterm::event::KeyCode;
use ratatui::{
  Frame,
  layout::{Alignment, Constraint, Layout, Rect},
  style::{Modifier, Style},
  text::{Line, Span},
  widgets::{Block, Clear, Paragraph, Wrap},
};

pub struct BluetoothApp {
  pub page: BluetoothPage,
  selected: Selection,
  on_buttons: bool,
  button_selected: usize,
  button_from: Option<usize>,
  snapshot: BluetoothSnapshot,
  job: Option<JobHandle<Result<BluetoothSnapshot, String>>>,
  action: Option<JobHandle<Result<String, String>>>,
  jobs: JobManager,
  lang: Lang,
  theme: Theme,
  capabilities: Capabilities,
  status: Option<StatusMessage>,
  confirm_remove: bool,
  confirmation: ConfirmationState,
  scanning: bool,
  agent: Option<AgentHandle>,
  agent_ready: bool,
  prompt: Option<AgentPrompt>,
  prompt_confirm: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionButton {
  Power,
  Discoverable,
  Pair,
  Connect,
  Disconnect,
  Trust,
  Remove,
}

impl BluetoothApp {
  pub fn new(lang: Lang, theme: Theme, capabilities: Capabilities) -> Self {
    Self {
      page: BluetoothPage::Home,
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
      status: None,
      confirm_remove: false,
      confirmation: ConfirmationState::default(),
      scanning: false,
      agent: None,
      agent_ready: false,
      prompt: None,
      prompt_confirm: false,
    }
  }
  pub fn reload(&mut self) {
    if self.job.is_some() || self.action.is_some() {
      return;
    }
    let caps = self.capabilities.clone();
    self.job = Some(self.jobs.spawn(move |_| {
      Ok(
        BluetoothBackend::new(SystemProcessRunner, caps)
          .snapshot()
          .map_err(|e| e.to_string()),
      )
    }));
  }
  pub fn poll(&mut self) -> bool {
    let mut changed = false;
    let mut events = Vec::new();
    if let Some(agent) = &self.agent {
      self.agent_ready = agent.is_ready();
      while let Some(event) = agent.try_event() {
        events.push(event);
      }
    }
    for event in events {
      match event {
        AgentEvent::Cancelled | AgentEvent::Released => {
          if self.prompt.is_some() {
            if let Some(agent) = &self.agent {
              agent.reply(AgentReply::Reject);
            }
            self.prompt = None;
            self.prompt_confirm = false;
            changed = true;
          }
        }
        start => self.open_prompt(start),
      }
    }
    if let Some(j) = self.job.take() {
      match j.try_state() {
        JobState::Running => self.job = Some(j),
        JobState::Finished(r) => {
          match r {
            Ok(Ok(s)) => {
              self.snapshot = s;
              self.selected.normalize(self.row_count());
            }
            Ok(Err(e)) | Err(e) => {
              self.status = Some(StatusMessage {
                kind: StatusKind::Error,
                text: e,
              })
            }
          };
          self.scanning = false;
          changed = true;
        }
      }
    }
    if let Some(j) = self.action.take() {
      match j.try_state() {
        JobState::Running => self.action = Some(j),
        JobState::Finished(r) => {
          match r {
            Ok(Ok(msg)) => {
              self.status = Some(StatusMessage {
                kind: StatusKind::Success,
                text: msg,
              });
              self.reload();
            }
            Ok(Err(e)) | Err(e) => {
              self.status = Some(StatusMessage {
                kind: StatusKind::Error,
                text: e,
              })
            }
          };
          if self.prompt.is_some() {
            if let Some(agent) = &self.agent {
              agent.reply(AgentReply::Reject);
            }
            self.prompt = None;
            self.prompt_confirm = false;
          }
          changed = true;
        }
      }
    }
    changed
  }
  fn row_count(&self) -> usize {
    match self.page {
      BluetoothPage::Home => 3,
      BluetoothPage::State => 2,
      BluetoothPage::Devices | BluetoothPage::Pair => self.snapshot.devices.len(),
    }
  }
  fn selected_device(&self) -> Option<&Device> {
    self.snapshot.devices.get(self.selected.index)
  }
  fn start_action(
    &mut self,
    operation: impl FnOnce(
      BluetoothBackend<SystemProcessRunner>,
    ) -> Result<(), crate::backend::BluetoothError>
    + Send
    + 'static,
    message: String,
  ) {
    if self.action.is_some() {
      return;
    }
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(
        self.lang,
        "Aplicando Bluetooth...",
        "Applying Bluetooth change...",
      )
      .into(),
    });
    let caps = self.capabilities.clone();
    self.action = Some(self.jobs.spawn(move |_| {
      Ok(
        operation(BluetoothBackend::new(SystemProcessRunner, caps))
          .map(|_| message)
          .map_err(|e| e.to_string()),
      )
    }));
  }
  fn act_device(&mut self, action: &'static str) {
    if let Some(d) = self.selected_device() {
      let address = d.address.clone();
      let msg = tr(
        self.lang,
        "Operação Bluetooth concluída.",
        "Bluetooth operation completed.",
      )
      .into();
      self.start_action(move |b| b.action(&address, action), msg);
    }
  }
  fn act_pair(&mut self) {
    if self.action.is_some() {
      return;
    }
    let Some(d) = self.selected_device() else {
      return;
    };
    let address = d.address.clone();
    let via_agent = self.agent_ready;
    let msg = tr(self.lang, "Dispositivo pareado.", "Device paired.").into();
    self.start_action(move |b| b.pair(&address, via_agent), msg);
  }
  fn open_agent(&mut self) {
    if self.agent.is_some() {
      return;
    }
    self.agent_ready = false;
    self.agent = Some(AgentHandle::start());
  }
  fn stop_agent(&mut self) {
    self.prompt = None;
    self.prompt_confirm = false;
    self.agent_ready = false;
    if let Some(mut agent) = self.agent.take() {
      agent.unregister();
    }
  }
  fn open_prompt(&mut self, event: AgentEvent) {
    if self.prompt.is_some() {
      return;
    }
    if let Some(prompt) = AgentPrompt::from_event(event) {
      self.prompt = Some(prompt);
      self.prompt_confirm = false;
    }
  }
  fn start_scan(&mut self) {
    if self.action.is_some() || self.scanning {
      return;
    }
    self.scanning = true;
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(
        self.lang,
        "Procurando dispositivos...",
        "Scanning for devices...",
      )
      .into(),
    });
    let caps = self.capabilities.clone();
    self.action = Some(self.jobs.spawn(move |_| {
      let b = BluetoothBackend::new(SystemProcessRunner, caps);
      b.scan(true).map_err(|e| e.to_string())?;
      std::thread::sleep(std::time::Duration::from_secs(2));
      b.scan(false).map_err(|e| e.to_string())?;
      b.snapshot().map_err(|e| e.to_string())?;
      Ok(Ok("Scan concluído.".into()))
    }));
  }
  pub fn handle(&mut self, key: KeyCode) -> bool {
    if let Some(mut prompt) = self.prompt.take() {
      let step = prompt.handle_key(key, &mut self.prompt_confirm);
      match step {
        PromptStep::Accepted(reply) => {
          if let Some(agent) = &self.agent {
            agent.reply(reply);
          }
          self.prompt_confirm = false;
        }
        PromptStep::Dismissed => {
          if let Some(agent) = &self.agent {
            agent.reply(AgentReply::Reject);
          }
          self.prompt_confirm = false;
        }
        PromptStep::Idle | PromptStep::Updated => {
          self.prompt = Some(prompt);
        }
      }
      return false;
    }
    if self.confirm_remove {
      match self.confirmation.handle(key) {
        ConfirmationOutcome::Confirmed => {
          self.confirm_remove = false;
          self.confirmation = ConfirmationState::default();
          self.act_device("remove");
        }
        ConfirmationOutcome::Cancelled => {
          self.confirm_remove = false;
          self.confirmation = ConfirmationState::default();
        }
        ConfirmationOutcome::Pending => {}
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
      if self.page == BluetoothPage::Home {
        return true;
      }
      self.page = BluetoothPage::Home;
      self.selected.index = 0;
      self.on_buttons = false;
      self.button_from = None;
      self.stop_agent();
      return false;
    }
    if key == KeyCode::Char('r') {
      if self.page == BluetoothPage::Pair {
        self.start_scan();
      } else {
        self.reload();
      }
      return false;
    }
    if self.action.is_some() {
      return false;
    }
    if self.page == BluetoothPage::Home && matches!(key, KeyCode::Enter | KeyCode::Right) {
      self.page = match self.selected.index {
        0 => BluetoothPage::State,
        1 => BluetoothPage::Devices,
        _ => BluetoothPage::Pair,
      };
      self.selected.index = 0;
      self.on_buttons = false;
      self.button_selected = 0;
      self.button_from = None;
      self.status = None;
      if self.page == BluetoothPage::Pair {
        self.open_agent();
      }
      self.reload();
      return false;
    }
    if self.selected.handle(key, self.row_count(), 8) {
      return false;
    }
    match (self.page, key) {
      (_, KeyCode::Tab | KeyCode::BackTab) => self.toggle_buttons(key == KeyCode::BackTab),
      (BluetoothPage::Pair, KeyCode::Enter) => self.act_pair(),
      (BluetoothPage::Devices, KeyCode::Enter) => {
        if self.selected_device().map(|d| d.connected).unwrap_or(false) {
          self.act_device("disconnect");
        } else if self.selected_device().map(|d| d.paired).unwrap_or(false) {
          self.act_device("connect");
        } else {
          self.act_device("pair");
        }
      }
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
      ActionButton::Power => {
        let on = !self
          .snapshot
          .adapter
          .as_ref()
          .map(|a| a.powered)
          .unwrap_or(false);
        self.start_action(
          move |b| b.power(on),
          tr(
            self.lang,
            "Estado do adaptador alterado.",
            "Adapter state changed.",
          )
          .into(),
        );
      }
      ActionButton::Discoverable => {
        let on = !self
          .snapshot
          .adapter
          .as_ref()
          .map(|a| a.discoverable)
          .unwrap_or(false);
        self.start_action(
          move |b| b.discoverable(on),
          tr(
            self.lang,
            "Visibilidade alterada.",
            "Discoverability changed.",
          )
          .into(),
        );
      }
      ActionButton::Pair => self.act_pair(),
      ActionButton::Connect => self.act_device("connect"),
      ActionButton::Disconnect => self.act_device("disconnect"),
      ActionButton::Trust => {
        let action = if self.selected_device().map(|d| d.trusted).unwrap_or(false) {
          "untrust"
        } else {
          "trust"
        };
        self.act_device(action);
      }
      ActionButton::Remove => self.confirm_remove = true,
    }
  }
  fn buttons(&self) -> Vec<(ActionButton, Button)> {
    match self.page {
      BluetoothPage::State => vec![
        (
          ActionButton::Power,
          Button::new(
            tr(self.lang, "Ligar/Desligar", "Power"),
            ButtonKind::Secondary,
          ),
        ),
        (
          ActionButton::Discoverable,
          Button::new(
            tr(self.lang, "Visível", "Discoverable"),
            ButtonKind::Secondary,
          ),
        ),
      ],
      BluetoothPage::Devices => vec![
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
          ActionButton::Trust,
          Button::new(tr(self.lang, "Confiar", "Trust"), ButtonKind::Secondary),
        ),
        (
          ActionButton::Remove,
          Button::new(tr(self.lang, "Remover", "Remove"), ButtonKind::Danger),
        ),
      ],
      BluetoothPage::Pair => vec![
        (
          ActionButton::Pair,
          Button::new(tr(self.lang, "Parear", "Pair"), ButtonKind::Primary),
        ),
        (
          ActionButton::Connect,
          Button::new(tr(self.lang, "Conectar", "Connect"), ButtonKind::Secondary),
        ),
        (
          ActionButton::Disconnect,
          Button::new(
            tr(self.lang, "Desconectar", "Disconnect"),
            ButtonKind::Secondary,
          ),
        ),
        (
          ActionButton::Trust,
          Button::new(tr(self.lang, "Confiar", "Trust"), ButtonKind::Secondary),
        ),
        (
          ActionButton::Remove,
          Button::new(tr(self.lang, "Remover", "Remove"), ButtonKind::Danger),
        ),
      ],
      BluetoothPage::Home => Vec::new(),
    }
  }
  fn footer_hints(&self) -> &'static str {
    match self.page {
      BluetoothPage::Home => tr(
        self.lang,
        "↑/↓ Navegar   →/Enter Abrir   ←/Esc Voltar   r Atualizar   ? Ajuda",
        "↑/↓ Navigate   →/Enter Open   ←/Esc Back   r Refresh   ? Help",
      ),
      BluetoothPage::State | BluetoothPage::Devices | BluetoothPage::Pair => tr(
        self.lang,
        "↑/↓ Navegar   Tab Ações   ←/→ Mover   Enter Ativar   r Atualizar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Navigate   Tab Actions   ←/→ Move   Enter Activate   r Refresh   ←/Esc Back   ? Help",
      ),
    }
  }
  pub fn draw(&self, f: &mut Frame) {
    let breadcrumb = match self.page {
      BluetoothPage::Home => tr(self.lang, "Bluetooth", "Bluetooth"),
      BluetoothPage::State => tr(self.lang, "Bluetooth > Estado", "Bluetooth > State"),
      BluetoothPage::Devices => tr(self.lang, "Bluetooth > Dispositivos", "Bluetooth > Devices"),
      BluetoothPage::Pair => tr(self.lang, "Bluetooth > Parear", "Bluetooth > Pair"),
    };
    let area = shell(f, f.area(), &self.theme, breadcrumb, self.footer_hints());
    let buttons = self.buttons();
    let raw_buttons: Vec<Button> = buttons.iter().map(|(_, button)| button.clone()).collect();
    let (body, button_area) = if raw_buttons.is_empty() {
      (area, None)
    } else {
      let button_height = argvus_tui::buttons::height(&raw_buttons, area.width).min(area.height);
      let split =
        Layout::vertical([Constraint::Min(1), Constraint::Length(button_height)]).split(area);
      (split[0], Some(split[1]))
    };
    let rows: Vec<String> = match self.page {
      BluetoothPage::Home => vec![
        tr(self.lang, "Estado", "State").into(),
        tr(self.lang, "Dispositivos", "Devices").into(),
        tr(self.lang, "Parear", "Pair").into(),
      ],
      BluetoothPage::State => {
        let a = self.snapshot.adapter.as_ref();
        vec![
          format!(
            "{}  {}",
            tr(self.lang, "Adaptador", "Adapter"),
            a.map(|x| x.name.as_str()).unwrap_or("—")
          ),
          format!(
            "{}  {}   {}",
            tr(self.lang, "Powered", "Powered"),
            a.map(|x| if x.powered { "Sim" } else { "Não" })
              .unwrap_or("—"),
            if self.scanning { "[SCAN]" } else { "" }
          ),
        ]
      }
      BluetoothPage::Devices | BluetoothPage::Pair => self
        .snapshot
        .devices
        .iter()
        .map(|d| {
          let state = if d.connected {
            tr(self.lang, "[CONECTADO]", "[CONNECTED]")
          } else if d.paired {
            tr(self.lang, "[PAREADO]", "[PAIRED]")
          } else {
            tr(self.lang, "[CONHECIDO]", "[KNOWN]")
          };
          let trusted = if d.trusted {
            tr(self.lang, " [CONFIÁVEL]", " [TRUSTED]")
          } else {
            ""
          };
          format!(
            "{}  {}  {}{}{}",
            if d.name.is_empty() {
              tr(self.lang, "(sem nome)", "(unnamed)")
            } else {
              &d.name
            },
            d.address,
            state,
            trusted,
            d.battery.map(|v| format!(" {}%", v)).unwrap_or_default()
          )
        })
        .collect(),
    };
    list(
      f,
      body,
      &self.theme,
      &rows,
      self.selected.index.min(rows.len().saturating_sub(1)),
    );
    if let Some(button_area) = button_area {
      let focus = if self.on_buttons {
        self.button_selected
      } else {
        usize::MAX
      };
      argvus_tui::buttons::draw(f, button_area, &raw_buttons, focus, &self.theme);
    }
    if let Some(s) = &self.status {
      draw_status(
        f,
        Rect::new(
          body.x,
          body.y + body.height.saturating_sub(1),
          body.width,
          1,
        ),
        &self.theme,
        s,
      );
    }
    if self.confirm_remove {
      draw_confirmation(
        f,
        f.area(),
        &self.theme,
        ConfirmationDialog {
          title: tr(self.lang, "Remover dispositivo", "Remove device"),
          message: tr(
            self.lang,
            "O dispositivo será removido da lista conhecida.",
            "The device will be removed from known devices.",
          ),
          confirm_label: tr(self.lang, "Remover", "Remove"),
          cancel_label: tr(self.lang, "Cancelar", "Cancel"),
          confirm_selected: self.confirmation.confirm_selected,
        },
      );
    }
    if let Some(prompt) = &self.prompt {
      self.draw_agent_prompt(f, prompt);
    }
  }
  fn draw_agent_prompt(&self, f: &mut Frame, prompt: &AgentPrompt) {
    let device_name = prompt.device();
    let selected = |label: &str, active: bool| {
      if active {
        Span::styled(
          format!("[ {label} ]"),
          Style::new()
            .fg(self.theme.selected_foreground)
            .bg(self.theme.selected_background)
            .add_modifier(Modifier::BOLD),
        )
      } else {
        Span::styled(format!("[ {label} ]"), Style::new().fg(self.theme.accent))
      }
    };
    let input_line = |label: &str, value: &str| {
      Line::from(vec![
        Span::styled(format!("{label} "), Style::new().fg(self.theme.muted)),
        Span::styled(format!("{value}_"), Style::new().fg(self.theme.foreground)),
      ])
    };
    let mut lines = vec![Line::from("")];
    let (title, height) = match prompt {
      AgentPrompt::Pin { input, .. } => {
        lines.push(Line::from(tr(
          self.lang,
          "Digite o PIN mostrado pelo dispositivo:",
          "Enter the PIN shown by the device:",
        )));
        lines.push(Line::from(""));
        lines.push(input_line("PIN:", input));
        lines.push(Line::from(""));
        (
          tr(self.lang, "PIN do dispositivo", "Device PIN").to_string(),
          9,
        )
      }
      AgentPrompt::Passkey { input, .. } => {
        lines.push(Line::from(tr(
          self.lang,
          "Digite o código de verificação do dispositivo:",
          "Enter the verification code from the device:",
        )));
        lines.push(Line::from(""));
        lines.push(input_line("Código:", input));
        lines.push(Line::from(""));
        (
          tr(self.lang, "Código de verificação", "Verification code").to_string(),
          9,
        )
      }
      AgentPrompt::DisplayPasskey { passkey, .. } => {
        lines.push(Line::from(match self.lang {
          Lang::Pt => format!("Mostrando o número, confira em {device_name}:"),
          Lang::En => format!("Showing the number, check it on {device_name}:"),
        }));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
          format!("{passkey:06}"),
          Style::new()
            .fg(self.theme.accent)
            .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
          selected(tr(self.lang, "Continuar", "Continue"), self.prompt_confirm),
          Span::raw("  "),
          selected(tr(self.lang, "Cancelar", "Cancel"), !self.prompt_confirm),
        ]));
        (
          tr(self.lang, "Número de verificação", "Verification number").to_string(),
          9,
        )
      }
      AgentPrompt::Confirm { passkey, .. } => {
        lines.push(Line::from(match self.lang {
          Lang::Pt => format!("{device_name} exibe o número:"),
          Lang::En => format!("{device_name} shows the number:"),
        }));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
          format!("{passkey:06}"),
          Style::new()
            .fg(self.theme.accent)
            .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
          selected(tr(self.lang, "Sim", "Yes"), self.prompt_confirm),
          Span::raw("  "),
          selected(tr(self.lang, "Não", "No"), !self.prompt_confirm),
        ]));
        (
          tr(self.lang, "Confirmar pareamento", "Confirm pairing").to_string(),
          9,
        )
      }
      AgentPrompt::Authorize { .. } => {
        lines.push(Line::from(match self.lang {
          Lang::Pt => format!("Permitir que {device_name} seja pareado?"),
          Lang::En => format!("Allow {device_name} to be paired?"),
        }));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
          selected(tr(self.lang, "Autorizar", "Allow"), self.prompt_confirm),
          Span::raw("  "),
          selected(tr(self.lang, "Cancelar", "Cancel"), !self.prompt_confirm),
        ]));
        (
          tr(self.lang, "Autorizar pareamento", "Authorize pairing").to_string(),
          8,
        )
      }
      AgentPrompt::Service { uuid, .. } => {
        lines.push(Line::from(match self.lang {
          Lang::Pt => format!("{device_name} pede acesso ao serviço:"),
          Lang::En => format!("{device_name} requests access to the service:"),
        }));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
          uuid,
          Style::new().fg(self.theme.foreground),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
          selected(tr(self.lang, "Permitir", "Allow"), self.prompt_confirm),
          Span::raw("  "),
          selected(tr(self.lang, "Negar", "Deny"), !self.prompt_confirm),
        ]));
        (
          tr(self.lang, "Acesso ao serviço", "Service access").to_string(),
          9,
        )
      }
    };
    lines.push(Line::from(tr(
      self.lang,
      "Enter OK · Esc Cancelar",
      "Enter OK · Esc Cancel",
    )));
    let popup = centered(
      f.area(),
      f.area().width.saturating_sub(8).clamp(36, 72),
      height,
    );
    f.render_widget(Clear, popup);
    f.render_widget(
      Paragraph::new(lines)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .block(
          Block::bordered()
            .title(format!(" {title} "))
            .border_style(Style::new().fg(self.theme.border_active))
            .style(Style::new().bg(self.theme.background)),
        ),
      popup,
    );
  }
}
