use crate::{
  backend::{AudioBackend, clamp_volume},
  model::{AudioDevice, AudioPage, AudioSnapshot},
};
use argvus_control_center_core::{
  capabilities::Capabilities,
  config::AppConfig,
  jobs::{JobHandle, JobManager, JobState},
  process::SystemProcessRunner,
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
  layout::{Constraint, Layout},
  text::Line,
  widgets::{Block, Clear, Paragraph},
};

#[derive(Debug, Clone)]
enum Pending {
  Action(AudioAction),
}

#[derive(Debug, Clone)]
enum AudioAction {
  SetDefault(u32),
}

pub struct AudioApp {
  pub page: AudioPage,
  selected: Selection,
  on_buttons: bool,
  button_selected: usize,
  button_from: Option<usize>,
  snapshot: AudioSnapshot,
  job: Option<JobHandle<Result<AudioSnapshot, String>>>,
  action: Option<JobHandle<Result<String, String>>>,
  pending: Option<Pending>,
  confirmation: ConfirmationState,
  jobs: JobManager,
  lang: Lang,
  theme: Theme,
  capabilities: Capabilities,
  pub status: Option<StatusMessage>,
  volume_input: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionButton {
  SetDefault,
  VolumeUp,
  VolumeDown,
  Mute,
  SetVolume,
}

impl AudioApp {
  pub fn new(lang: Lang, theme: Theme, capabilities: Capabilities) -> Self {
    Self {
      page: AudioPage::Home,
      selected: Selection::default(),
      on_buttons: false,
      button_selected: 0,
      button_from: None,
      snapshot: Default::default(),
      job: None,
      action: None,
      pending: None,
      confirmation: ConfirmationState::default(),
      jobs: JobManager::default(),
      lang,
      theme,
      capabilities,
      status: None,
      volume_input: None,
    }
  }
  pub fn reload(&mut self) {
    if self.job.is_some() || self.action.is_some() {
      return;
    }
    let caps = self.capabilities.clone();
    self.job = Some(self.jobs.spawn(move |_| {
      Ok(
        AudioBackend::new(SystemProcessRunner, caps)
          .snapshot()
          .map_err(|e| e.to_string()),
      )
    }));
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(self.lang, "Carregando áudio...", "Loading audio...").into(),
    });
  }
  pub fn poll(&mut self) -> bool {
    let mut changed = false;
    if let Some(j) = &self.job
      && let JobState::Finished(r) = j.try_state()
    {
      self.job = None;
      match r {
        Ok(Ok(s)) => {
          self.snapshot = s;
          self.normalize();
          self.success(tr(self.lang, "Áudio atualizado", "Audio refreshed"));
        }
        Ok(Err(e)) | Err(e) => self.error(e),
      };
      changed = true;
    }
    if let Some(j) = &self.action
      && let JobState::Finished(r) = j.try_state()
    {
      self.action = None;
      match r {
        Ok(Ok(msg)) => {
          self.success(msg);
          self.reload();
        }
        Ok(Err(e)) | Err(e) => {
          self.error(e);
          self.reload();
        }
      };
      changed = true;
    }
    changed
  }
  fn success(&mut self, text: impl Into<String>) {
    self.status = Some(StatusMessage {
      kind: StatusKind::Success,
      text: text.into(),
    });
  }
  fn error(&mut self, text: impl Into<String>) {
    self.status = Some(StatusMessage {
      kind: StatusKind::Error,
      text: text.into(),
    });
  }
  fn busy(&self) -> bool {
    self.job.is_some() || self.action.is_some()
  }
  fn normalize(&mut self) {
    self.selected.normalize(self.row_count());
  }
  fn row_count(&self) -> usize {
    match self.page {
      AudioPage::Home => 4,
      AudioPage::Summary => 1,
      AudioPage::Output => self.snapshot.outputs.len(),
      AudioPage::Input => self.snapshot.inputs.len(),
      AudioPage::Devices => self.snapshot.outputs.len() + self.snapshot.inputs.len(),
    }
  }
  fn devices(&self) -> Vec<&AudioDevice> {
    match self.page {
      AudioPage::Output => self.snapshot.outputs.iter().collect(),
      AudioPage::Input => self.snapshot.inputs.iter().collect(),
      AudioPage::Devices => self
        .snapshot
        .outputs
        .iter()
        .chain(self.snapshot.inputs.iter())
        .collect(),
      AudioPage::Home | AudioPage::Summary => Vec::new(),
    }
  }
  fn selected_device(&self) -> Option<&AudioDevice> {
    self
      .devices()
      .get(self.selected.index)
      .copied()
      .filter(|device| device.id > 0)
  }
  fn start_action(
    &mut self,
    action: impl FnOnce(AudioBackend<SystemProcessRunner>) -> Result<(), crate::backend::AudioError>
    + Send
    + 'static,
    success: String,
  ) {
    if self.action.is_some() {
      return;
    }
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(self.lang, "Aplicando áudio...", "Applying audio change...").into(),
    });
    let caps = self.capabilities.clone();
    self.action = Some(self.jobs.spawn(move |_| {
      Ok(
        action(AudioBackend::new(SystemProcessRunner, caps))
          .map(|_| success)
          .map_err(|e| e.to_string()),
      )
    }));
  }
  fn start_pending(&mut self) {
    let Some(Pending::Action(action)) = self.pending.take() else {
      return;
    };
    match action {
      AudioAction::SetDefault(id) => {
        self.start_action(
          move |b| b.set_default(id),
          tr(
            self.lang,
            "Dispositivo padrão atualizado.",
            "Default device updated.",
          )
          .into(),
        );
      }
    }
  }
  fn request_default(&mut self) {
    if let Some(d) = self.selected_device() {
      self.pending = Some(Pending::Action(AudioAction::SetDefault(d.id)));
    } else {
      self.warn_disappeared();
    }
  }
  fn adjust_volume(&mut self, delta: i16) {
    if let Some(d) = self.selected_device() {
      let id = d.id;
      let current = d.volume.unwrap_or(0) as i16;
      let value = clamp_volume(current + delta);
      self.start_action(
        move |b| b.set_volume(id, value),
        tr(self.lang, "Volume alterado.", "Volume changed.").into(),
      );
    } else {
      self.warn_disappeared();
    }
  }
  fn toggle_mute(&mut self) {
    if let Some(d) = self.selected_device() {
      let id = d.id;
      let mute = !d.muted;
      self.start_action(
        move |b| b.set_mute(id, mute),
        tr(self.lang, "Mute alterado.", "Mute changed.").into(),
      );
    } else {
      self.warn_disappeared();
    }
  }
  fn warn_disappeared(&mut self) {
    self.error(tr(
      self.lang,
      "Dispositivo não está mais disponível. Pressione r para atualizar.",
      "Device is no longer available. Press r to refresh.",
    ));
  }
  pub fn handle(&mut self, key: KeyCode) -> bool {
    if self.pending.is_some() {
      match self.confirmation.handle(key) {
        ConfirmationOutcome::Confirmed => {
          self.confirmation = ConfirmationState::default();
          self.start_pending();
        }
        ConfirmationOutcome::Cancelled => {
          self.pending = None;
          self.confirmation = ConfirmationState::default();
        }
        ConfirmationOutcome::Pending => {}
      }
      return false;
    }
    if self.volume_input.is_some() {
      return self.handle_volume_input(key);
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
      if self.page == AudioPage::Home {
        return true;
      }
      self.page = AudioPage::Home;
      self.selected.index = 0;
      self.on_buttons = false;
      self.button_from = None;
      return false;
    }
    match key {
      KeyCode::Char('r') => self.reload(),
      KeyCode::Tab | KeyCode::BackTab => self.toggle_buttons(key == KeyCode::BackTab),
      KeyCode::Up
      | KeyCode::Char('k')
      | KeyCode::Down
      | KeyCode::Char('j')
      | KeyCode::Home
      | KeyCode::End
      | KeyCode::PageUp
      | KeyCode::PageDown => {
        self.selected.handle(key, self.row_count(), 8);
      }
      KeyCode::Enter | KeyCode::Right => self.open_selected(),
      _ => {}
    }
    false
  }
  fn open_selected(&mut self) {
    if self.page == AudioPage::Home {
      self.page = match self.selected.index {
        0 => AudioPage::Summary,
        1 => AudioPage::Output,
        2 => AudioPage::Input,
        _ => AudioPage::Devices,
      };
      self.selected.index = 0;
      self.reload();
    } else if matches!(self.page, AudioPage::Output | AudioPage::Input) {
      self.request_default();
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
      ActionButton::SetDefault => self.request_default(),
      ActionButton::VolumeUp => self.adjust_volume(5),
      ActionButton::VolumeDown => self.adjust_volume(-5),
      ActionButton::Mute => self.toggle_mute(),
      ActionButton::SetVolume => self.volume_input = Some(String::new()),
    }
  }
  fn buttons(&self) -> Vec<(ActionButton, Button)> {
    if !matches!(self.page, AudioPage::Output | AudioPage::Input) || self.devices().is_empty() {
      return Vec::new();
    }
    vec![
      (
        ActionButton::SetDefault,
        Button::new(tr(self.lang, "Padrão", "Default"), ButtonKind::Primary),
      ),
      (
        ActionButton::VolumeUp,
        Button::new(tr(self.lang, "Volume +", "Volume +"), ButtonKind::Secondary),
      ),
      (
        ActionButton::VolumeDown,
        Button::new(tr(self.lang, "Volume -", "Volume -"), ButtonKind::Secondary),
      ),
      (
        ActionButton::Mute,
        Button::new(tr(self.lang, "Mudo", "Mute"), ButtonKind::Secondary),
      ),
      (
        ActionButton::SetVolume,
        Button::new(tr(self.lang, "Valor", "Value"), ButtonKind::Secondary),
      ),
    ]
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
    if self.page == AudioPage::Home {
      home
    } else if !self.buttons().is_empty() {
      action
    } else {
      readonly
    }
  }
  fn handle_volume_input(&mut self, key: KeyCode) -> bool {
    let input = self.volume_input.as_mut().unwrap();
    match key {
      KeyCode::Esc => self.volume_input = None,
      KeyCode::Enter => {
        let value = input
          .parse::<i16>()
          .ok()
          .filter(|value| (0..=100).contains(value))
          .map(clamp_volume);
        self.volume_input = None;
        if let (Some(v), Some(d)) = (value, self.selected_device()) {
          let id = d.id;
          self.start_action(
            move |b| b.set_volume(id, v),
            tr(self.lang, "Volume definido.", "Volume set.").into(),
          );
        } else {
          self.error(tr(
            self.lang,
            "Volume inválido (0-100).",
            "Invalid volume (0-100).",
          ));
        }
      }
      KeyCode::Backspace => {
        input.pop();
      }
      KeyCode::Char(c) if c.is_ascii_digit() && input.len() < 3 => input.push(c),
      _ => {}
    }
    false
  }
  pub fn draw(&self, frame: &mut Frame) {
    let area = frame.area();
    let body = shell(
      frame,
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
    if matches!(self.page, AudioPage::Summary | AudioPage::Devices) {
      readonly(
        frame,
        body,
        &self.theme,
        &rows.into_iter().map(Line::from).collect::<Vec<_>>(),
      );
    } else {
      list(
        frame,
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
      argvus_tui::buttons::draw(frame, button_area, &raw_buttons, focus, &self.theme);
    }
    if let Some(value) = &self.volume_input {
      let popup = argvus_tui::chrome::centered(area, 48, 7);
      frame.render_widget(Clear, popup);
      frame.render_widget(
        Paragraph::new(vec![
          Line::from(tr(
            self.lang,
            "Digite o volume (0-100):",
            "Enter volume (0-100):",
          )),
          Line::from(format!("{value}_")),
          Line::from(tr(
            self.lang,
            "Enter aplicar   Esc cancelar",
            "Enter apply   Esc cancel",
          )),
        ])
        .block(Block::bordered().title(tr(self.lang, "Volume", "Volume"))),
        popup,
      );
    }
    if let Some(Pending::Action(AudioAction::SetDefault(id))) = &self.pending {
      let dev_name = self
        .snapshot
        .outputs
        .iter()
        .chain(self.snapshot.inputs.iter())
        .find(|d| d.id == *id)
        .map(|d| d.description.as_str())
        .unwrap_or("dispositivo");
      let message = format!(
        "{} '{}' {}?",
        tr(self.lang, "Definir", "Set"),
        dev_name,
        tr(
          self.lang,
          "como dispositivo padrão de áudio",
          "as default audio device"
        )
      );
      draw_confirmation(
        frame,
        area,
        &self.theme,
        ConfirmationDialog {
          title: tr(
            self.lang,
            "Confirmar operação de áudio",
            "Confirm audio operation",
          ),
          message: &message,
          confirm_label: tr(self.lang, "Continuar", "Continue"),
          cancel_label: tr(self.lang, "Cancelar", "Cancel"),
          confirm_selected: self.confirmation.confirm_selected,
        },
      );
    }
    if let Some(status_message) = &self.status {
      status(frame, body, &self.theme, status_message);
    }
  }
  fn rows(&self) -> Vec<String> {
    match self.page {
      AudioPage::Home => self.home_rows(),
      AudioPage::Summary => self.summary_rows(),
      AudioPage::Output | AudioPage::Input => self.device_rows(),
      AudioPage::Devices => self.all_devices_rows(),
    }
  }
  fn home_rows(&self) -> Vec<String> {
    let summary_label = tr(self.lang, "Resumo", "Summary");
    let output_label = tr(self.lang, "Saída", "Output");
    let input_label = tr(self.lang, "Entrada", "Input");
    let devices_label = tr(self.lang, "Dispositivos", "Devices");

    let status_str = if self.snapshot.available {
      tr(self.lang, "Ativo", "Active")
    } else {
      tr(self.lang, "Indisponível", "Unavailable")
    };

    let default_out_str = self
      .snapshot
      .default_output
      .and_then(|id| self.snapshot.outputs.iter().find(|d| d.id == id))
      .map(|d| d.description.as_str())
      .unwrap_or("—");

    let default_in_str = self
      .snapshot
      .default_input
      .and_then(|id| self.snapshot.inputs.iter().find(|d| d.id == id))
      .map(|d| d.description.as_str())
      .unwrap_or("—");

    let total_devs = self.snapshot.outputs.len() + self.snapshot.inputs.len();

    vec![
      format!(
        "{} {}  ·  {} · {}",
        AppConfig::icon("💻"),
        summary_label,
        if self.snapshot.backend.is_empty() {
          "PipeWire/WirePlumber".into()
        } else {
          self.snapshot.backend.clone()
        },
        status_str
      ),
      format!(
        "{} {}  ·  {}  ·  {}",
        AppConfig::icon("🔊"),
        output_label,
        format!(
          "{} {}",
          self.snapshot.outputs.len(),
          tr(self.lang, "saídas", "outputs")
        ),
        default_out_str
      ),
      format!(
        "{} {}  ·  {}  ·  {}",
        AppConfig::icon("🎙️"),
        input_label,
        format!(
          "{} {}",
          self.snapshot.inputs.len(),
          tr(self.lang, "entradas", "inputs")
        ),
        default_in_str
      ),
      format!(
        "{} {}  ·  {}",
        AppConfig::icon("🎧"),
        devices_label,
        format!(
          "{} {}",
          total_devs,
          tr(self.lang, "dispositivos", "devices")
        )
      ),
    ]
  }
  fn summary_rows(&self) -> Vec<String> {
    if !self.snapshot.available {
      return vec![
        format!(
          " {} {}",
          AppConfig::icon("💻"),
          tr(self.lang, "SISTEMA DE ÁUDIO", "AUDIO SYSTEM")
        ),
        format!(
          "   Status: {}",
          tr(self.lang, "Indisponível", "Unavailable")
        ),
      ];
    }

    let default_out = self
      .snapshot
      .default_output
      .and_then(|id| self.snapshot.outputs.iter().find(|d| d.id == id))
      .map(|d| {
        format!(
          "{} ({}%)",
          d.description,
          d.volume
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into())
        )
      })
      .unwrap_or_else(|| "—".into());

    let default_in = self
      .snapshot
      .default_input
      .and_then(|id| self.snapshot.inputs.iter().find(|d| d.id == id))
      .map(|d| {
        format!(
          "{} ({}%)",
          d.description,
          d.volume
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into())
        )
      })
      .unwrap_or_else(|| "—".into());

    vec![
      format!(
        " {} {}",
        AppConfig::icon("💻"),
        tr(self.lang, "SISTEMA DE ÁUDIO", "AUDIO SYSTEM")
      ),
      format!(
        "   Servidor:    {}",
        if self.snapshot.backend.is_empty() {
          "PipeWire / WirePlumber".into()
        } else {
          self.snapshot.backend.clone()
        }
      ),
      format!("   Status:      {}", tr(self.lang, "Ativo", "Active")),
      "".into(),
      format!(
        " {} {}",
        AppConfig::icon("🔊"),
        tr(self.lang, "DISPOSITIVOS PADRÃO", "DEFAULT DEVICES")
      ),
      format!("   Saída:       {}", default_out),
      format!("   Entrada:     {}", default_in),
      "".into(),
      format!(
        " {} {}",
        AppConfig::icon("🎧"),
        tr(self.lang, "DISPOSITIVOS DISPONÍVEIS", "AVAILABLE DEVICES")
      ),
      format!("   Saídas:      {}", self.snapshot.outputs.len()),
      format!("   Entradas:    {}", self.snapshot.inputs.len()),
    ]
  }
  fn device_rows(&self) -> Vec<String> {
    if !self.snapshot.available {
      return vec![
        tr(
          self.lang,
          "PipeWire/WirePlumber não está disponível.",
          "PipeWire/WirePlumber is unavailable.",
        )
        .into(),
      ];
    }
    let devices = self.devices();
    if devices.is_empty() {
      return vec![
        tr(
          self.lang,
          "Nenhum dispositivo encontrado.",
          "No devices found.",
        )
        .into(),
      ];
    }
    devices
      .iter()
      .map(|d| {
        let mut badges = Vec::new();
        if Some(d.id)
          == (if d.direction == "output" {
            self.snapshot.default_output
          } else {
            self.snapshot.default_input
          })
        {
          badges.push(format!("● {}", tr(self.lang, "Padrão", "Default")));
        }
        if d.muted {
          badges.push(format!("✖ {}", tr(self.lang, "Mudo", "Muted")));
        }
        let vol = d
          .volume
          .map(|v| format!("{}%", v))
          .unwrap_or_else(|| "—".into());
        let badge_str = if badges.is_empty() {
          String::new()
        } else {
          format!("   {}", badges.join(" · "))
        };
        format!("{} · {}{}", d.description, vol, badge_str)
      })
      .collect()
  }
  fn all_devices_rows(&self) -> Vec<String> {
    if !self.snapshot.available {
      return vec![
        tr(
          self.lang,
          "PipeWire/WirePlumber não está disponível.",
          "PipeWire/WirePlumber is unavailable.",
        )
        .into(),
      ];
    }
    let mut rows = Vec::new();

    rows.push(format!(
      " {} {}",
      AppConfig::icon("🔊"),
      tr(self.lang, "DISPOSITIVOS DE SAÍDA", "OUTPUT DEVICES")
    ));
    if self.snapshot.outputs.is_empty() {
      rows.push(format!(
        "   {}",
        tr(self.lang, "Nenhuma saída encontrada", "No output found")
      ));
    } else {
      for d in &self.snapshot.outputs {
        let is_def = Some(d.id) == self.snapshot.default_output;
        let mut flags = Vec::new();
        if is_def {
          flags.push(tr(self.lang, "Padrão", "Default"));
        }
        if d.muted {
          flags.push(tr(self.lang, "Mudo", "Muted"));
        }
        let flag_str = if flags.is_empty() {
          String::new()
        } else {
          format!(" [{}]", flags.join(", "))
        };
        let vol = d
          .volume
          .map(|v| format!("{}%", v))
          .unwrap_or_else(|| "—".into());
        rows.push(format!("   {} ({}){}", d.description, vol, flag_str));
      }
    }

    rows.push("".into());
    rows.push(format!(
      " {} {}",
      AppConfig::icon("🎙️"),
      tr(self.lang, "DISPOSITIVOS DE ENTRADA", "INPUT DEVICES")
    ));
    if self.snapshot.inputs.is_empty() {
      rows.push(format!(
        "   {}",
        tr(self.lang, "Nenhuma entrada encontrada", "No input found")
      ));
    } else {
      for d in &self.snapshot.inputs {
        let is_def = Some(d.id) == self.snapshot.default_input;
        let mut flags = Vec::new();
        if is_def {
          flags.push(tr(self.lang, "Padrão", "Default"));
        }
        if d.muted {
          flags.push(tr(self.lang, "Mudo", "Muted"));
        }
        let flag_str = if flags.is_empty() {
          String::new()
        } else {
          format!(" [{}]", flags.join(", "))
        };
        let vol = d
          .volume
          .map(|v| format!("{}%", v))
          .unwrap_or_else(|| "—".into());
        rows.push(format!("   {} ({}){}", d.description, vol, flag_str));
      }
    }

    rows
  }
  fn breadcrumb(&self) -> String {
    let root = tr(self.lang, "Áudio", "Audio");
    if self.page == AudioPage::Home {
      root.into()
    } else {
      format!("{root} > {}", self.page_label())
    }
  }
  fn page_label(&self) -> &'static str {
    match self.page {
      AudioPage::Home => tr(self.lang, "Áudio", "Audio"),
      AudioPage::Summary => tr(self.lang, "Resumo", "Summary"),
      AudioPage::Output => tr(self.lang, "Saída", "Output"),
      AudioPage::Input => tr(self.lang, "Entrada", "Input"),
      AudioPage::Devices => tr(self.lang, "Dispositivos", "Devices"),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use ratatui::{Terminal, backend::TestBackend};

  #[test]
  fn audio_home_rows_act_as_a_status_dashboard() {
    let mut app = AudioApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.snapshot.available = true;
    app.snapshot.backend = "PipeWire".into();
    app.snapshot.outputs = vec![AudioDevice {
      id: 42,
      description: "Speakers".into(),
      direction: "output".into(),
      volume: Some(80),
      ..Default::default()
    }];
    app.snapshot.inputs = vec![AudioDevice {
      id: 43,
      description: "Mic".into(),
      direction: "input".into(),
      volume: Some(50),
      ..Default::default()
    }];
    app.snapshot.default_output = Some(42);
    app.snapshot.default_input = Some(43);

    let rows = app.rows();
    assert_eq!(rows.len(), 4);
    assert!(rows[0].contains("Summary") || rows[0].contains("Resumo"));
    assert!(rows[0].contains("PipeWire"));
    assert!(rows[1].contains("Output") || rows[1].contains("Saída"));
    assert!(rows[1].contains("Speakers"));
    assert!(rows[2].contains("Input") || rows[2].contains("Entrada"));
    assert!(rows[2].contains("Mic"));
    assert!(rows[3].contains("2"));
  }

  #[test]
  fn audio_device_rows_render_clean_badges() {
    let mut app = AudioApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.snapshot.available = true;
    app.snapshot.outputs = vec![
      AudioDevice {
        id: 1,
        description: "Built-in Audio".into(),
        direction: "output".into(),
        volume: Some(70),
        muted: false,
        ..Default::default()
      },
      AudioDevice {
        id: 2,
        description: "USB Headset".into(),
        direction: "output".into(),
        volume: Some(50),
        muted: true,
        ..Default::default()
      },
    ];
    app.snapshot.default_output = Some(1);
    app.page = AudioPage::Output;

    let rows = app.rows();
    assert_eq!(rows.len(), 2);
    assert!(rows[0].contains("Built-in Audio · 70%"));
    assert!(rows[0].contains("Default") || rows[0].contains("Padrão"));
    assert!(rows[1].contains("USB Headset · 50%"));
    assert!(rows[1].contains("Muted") || rows[1].contains("Mudo"));
  }

  #[test]
  fn audio_home_and_details_are_keyboard_navigable() {
    let mut app = AudioApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.snapshot.outputs = vec![AudioDevice {
      id: 1,
      description: "Speakers".into(),
      direction: "output".into(),
      ..Default::default()
    }];
    app.handle(KeyCode::Down); // select Output
    app.handle(KeyCode::Enter);
    assert_eq!(app.page, AudioPage::Output);
    app.job = None;
    app.handle(KeyCode::Esc);
    assert_eq!(app.page, AudioPage::Home);
  }

  #[test]
  fn audio_confirmation_dialog_shown_and_cancelable() {
    let mut app = AudioApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.snapshot.outputs = vec![AudioDevice {
      id: 10,
      description: "Headphones".into(),
      direction: "output".into(),
      ..Default::default()
    }];
    app.page = AudioPage::Output;
    app.handle(KeyCode::Enter); // trigger default request
    assert!(app.pending.is_some());
    app.handle(KeyCode::Esc); // cancel
    assert!(app.pending.is_none());
  }

  #[test]
  fn audio_tab_cycles_between_list_and_buttons() {
    let mut app = AudioApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.snapshot.outputs = vec![AudioDevice {
      id: 1,
      description: "Speakers".into(),
      direction: "output".into(),
      ..Default::default()
    }];
    app.page = AudioPage::Output;
    app.selected.index = 0;
    app.handle(KeyCode::Tab);
    assert!(app.on_buttons);
    app.handle(KeyCode::Tab);
    assert!(!app.on_buttons);
  }

  #[test]
  fn audio_uses_shared_chrome_and_contextual_footer() {
    let app = AudioApp::new(Lang::En, Theme::load(), Capabilities::default());
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
    assert!(text.contains("Audio") || text.contains("Áudio"));
    assert!(text.contains("Enter") && text.contains("Back"));
  }
}
