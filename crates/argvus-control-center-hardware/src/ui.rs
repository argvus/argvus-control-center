use crate::{
  backend,
  model::{HardwarePage, HardwareSnapshot, format_kib},
};
use argvus_control_center_core::{
  capabilities::Capabilities,
  config::AppConfig,
  jobs::{JobHandle, JobManager, JobState},
  privileged::{PrivilegedOperation, PrivilegedRequest, SystemSettingsOperation},
  process::{ProcessRequest, ProcessRunner, SystemProcessRunner},
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
};

#[derive(Debug, Clone)]
enum Pending {
  Action(HardwareAction),
}

#[derive(Debug, Clone)]
enum HardwareAction {
  SetGovernor(String),
  SetProfile(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionButton {
  ApplyGovernor,
  ApplyProfile,
}

pub struct HardwareApp {
  pub page: HardwarePage,
  selected: Selection,
  on_buttons: bool,
  button_selected: usize,
  button_from: Option<usize>,
  pub snapshot: HardwareSnapshot,
  job: Option<JobHandle<HardwareSnapshot>>,
  action: Option<JobHandle<String>>,
  pending: Option<Pending>,
  confirmation: ConfirmationState,
  manager: JobManager,
  cap: Capabilities,
  pub lang: Lang,
  pub theme: Theme,
  pub status: Option<StatusMessage>,
}

impl HardwareApp {
  pub fn new(lang: Lang, theme: Theme, cap: Capabilities) -> Self {
    let mut app = Self {
      page: HardwarePage::Home,
      selected: Selection::default(),
      on_buttons: false,
      button_selected: 0,
      button_from: None,
      snapshot: Default::default(),
      job: None,
      action: None,
      pending: None,
      confirmation: ConfirmationState::default(),
      manager: JobManager::default(),
      cap,
      lang,
      theme,
      status: None,
    };
    app.refresh();
    app
  }

  pub fn refresh(&mut self) {
    if self.job.is_some() || self.action.is_some() {
      return;
    }
    let cap = self.cap.clone();
    self.job = Some(self.manager.spawn(move |_| Ok(backend::collect(&cap))));
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(self.lang, "Carregando hardware...", "Loading hardware...").into(),
    });
  }

  pub fn poll(&mut self) -> bool {
    let mut changed = false;
    if let Some(job) = &self.action
      && let JobState::Finished(result) = job.try_state()
    {
      self.action = None;
      match result {
        Ok(msg) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Success,
            text: msg,
          });
        }
        Err(err) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Error,
            text: err,
          });
        }
      }
      self.refresh();
      changed = true;
    }
    if let Some(job) = &self.job
      && let JobState::Finished(result) = job.try_state()
    {
      self.job = None;
      match result {
        Ok(snapshot) => {
          self.snapshot = snapshot;
          self.normalize();
          self.status = Some(StatusMessage {
            kind: StatusKind::Success,
            text: tr(self.lang, "Hardware atualizado", "Hardware refreshed").into(),
          });
        }
        Err(error) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Error,
            text: error,
          });
        }
      }
      changed = true;
    }
    changed
  }

  pub fn busy(&self) -> bool {
    self.job.is_some() || self.action.is_some()
  }

  fn normalize(&mut self) {
    self.selected.normalize(self.selection_len());
  }

  fn selection_len(&self) -> usize {
    match self.page {
      HardwarePage::Home => 6,
      HardwarePage::Cpu => self.snapshot.cpu.governors.len(),
      HardwarePage::Gpu => self.snapshot.gpus.len(),
      HardwarePage::Power => self.snapshot.energy.profiles.len(),
      HardwarePage::Devices => self.snapshot.devices.len(),
      _ => 1,
    }
  }

  pub fn handle(&mut self, key: KeyCode) -> bool {
    if self.pending.is_some() {
      match self.confirmation.handle(key) {
        ConfirmationOutcome::Confirmed => {
          if let Some(Pending::Action(act)) = self.pending.take() {
            self.execute_action(act);
          }
        }
        ConfirmationOutcome::Cancelled => {
          self.pending = None;
        }
        ConfirmationOutcome::Pending => {}
      }
      return false;
    }

    if matches!(key, KeyCode::Esc | KeyCode::Left) && !self.on_buttons {
      if self.page == HardwarePage::Home {
        return true;
      }
      self.navigate_back();
      return false;
    }

    if self.busy() {
      return false;
    }

    let buttons = self.buttons();

    if key == KeyCode::Tab && !buttons.is_empty() {
      if self.on_buttons {
        self.on_buttons = false;
        if let Some(old) = self.button_from {
          self.selected.index = old;
        }
      } else {
        self.on_buttons = true;
        self.button_from = Some(self.selected.index);
        self.button_selected = 0;
      }
      return false;
    }

    if self.on_buttons {
      match key {
        KeyCode::Left | KeyCode::Char('h') => {
          self.button_selected = self.button_selected.saturating_sub(1);
        }
        KeyCode::Right | KeyCode::Char('l') => {
          self.button_selected += 1;
          if !buttons.is_empty() {
            self.button_selected = self.button_selected.min(buttons.len() - 1);
          }
        }
        KeyCode::Enter => {
          if let Some((action, _)) = buttons.get(self.button_selected) {
            self.trigger_button(*action);
          }
        }
        KeyCode::Esc => {
          self.on_buttons = false;
          if let Some(old) = self.button_from {
            self.selected.index = old;
          }
        }
        _ => {}
      }
      return false;
    }

    if self.selected.handle(key, self.selection_len(), 8) {
      return false;
    }

    match key {
      KeyCode::Char('r') => self.refresh(),

      KeyCode::Char('g') if self.page == HardwarePage::Cpu => {
        if let Some(gov) = self.snapshot.cpu.governors.get(self.selected.index) {
          self.pending = Some(Pending::Action(HardwareAction::SetGovernor(gov.clone())));
          self.confirmation = ConfirmationState::default();
        }
      }
      KeyCode::Char('e') if self.page == HardwarePage::Power => {
        if let Some(prof) = self.snapshot.energy.profiles.get(self.selected.index) {
          self.pending = Some(Pending::Action(HardwareAction::SetProfile(prof.clone())));
          self.confirmation = ConfirmationState::default();
        }
      }

      KeyCode::Enter | KeyCode::Right => match self.page {
        HardwarePage::Home => {
          self.page = [
            HardwarePage::Summary,
            HardwarePage::Cpu,
            HardwarePage::Gpu,
            HardwarePage::Memory,
            HardwarePage::Power,
            HardwarePage::Devices,
          ][self.selected.index];
          self.selected.index = 0;
        }
        HardwarePage::Devices if self.snapshot.devices.get(self.selected.index).is_some() => {
          self.page = HardwarePage::DeviceDetail(self.selected.index);
        }
        HardwarePage::Gpu if self.snapshot.gpus.get(self.selected.index).is_some() => {
          self.page = HardwarePage::GpuDetail(self.selected.index);
          self.selected.index = 0;
        }
        HardwarePage::Cpu => {
          if let Some(gov) = self.snapshot.cpu.governors.get(self.selected.index) {
            self.pending = Some(Pending::Action(HardwareAction::SetGovernor(gov.clone())));
            self.confirmation = ConfirmationState::default();
          }
        }
        HardwarePage::Power => {
          if let Some(prof) = self.snapshot.energy.profiles.get(self.selected.index) {
            self.pending = Some(Pending::Action(HardwareAction::SetProfile(prof.clone())));
            self.confirmation = ConfirmationState::default();
          }
        }
        _ => {}
      },
      _ => {}
    }
    false
  }

  fn navigate_back(&mut self) {
    self.page = match self.page {
      HardwarePage::DeviceDetail(_) => HardwarePage::Devices,
      HardwarePage::GpuDetail(_) => HardwarePage::Gpu,
      _ => HardwarePage::Home,
    };
    self.selected.index = 0;
    self.on_buttons = false;
  }

  fn execute_action(&mut self, action: HardwareAction) {
    let cap = self.cap.clone();
    match action {
      HardwareAction::SetGovernor(gov) => {
        self.status = Some(StatusMessage {
          kind: StatusKind::Info,
          text: tr(self.lang, "Alterando governor...", "Changing governor...").into(),
        });
        if cap.has_power_profiles_daemon {
          let profile = match gov.as_str() {
            "performance" => "performance".to_string(),
            "powersave" => "power-saver".to_string(),
            _ => gov.clone(),
          };
          self.action = Some(self.manager.spawn(move |_| run_profile(&profile)));
        } else {
          self.action = Some(self.manager.spawn(move |_| run_governor(&gov)));
        }
      }
      HardwareAction::SetProfile(prof) => {
        self.status = Some(StatusMessage {
          kind: StatusKind::Info,
          text: tr(self.lang, "Alterando perfil...", "Changing profile...").into(),
        });
        self.action = Some(self.manager.spawn(move |_| run_profile(&prof)));
      }
    }
  }

  fn trigger_button(&mut self, btn: ActionButton) {
    match btn {
      ActionButton::ApplyGovernor => {
        if let Some(gov) = self.snapshot.cpu.governors.get(self.selected.index) {
          self.pending = Some(Pending::Action(HardwareAction::SetGovernor(gov.clone())));
          self.confirmation = ConfirmationState::default();
        }
      }
      ActionButton::ApplyProfile => {
        if let Some(prof) = self.snapshot.energy.profiles.get(self.selected.index) {
          self.pending = Some(Pending::Action(HardwareAction::SetProfile(prof.clone())));
          self.confirmation = ConfirmationState::default();
        }
      }
    }
  }

  fn buttons(&self) -> Vec<(ActionButton, Button)> {
    match self.page {
      HardwarePage::Cpu if !self.snapshot.cpu.governors.is_empty() => vec![(
        ActionButton::ApplyGovernor,
        Button::new(
          tr(self.lang, "Aplicar Governor", "Apply Governor"),
          ButtonKind::Primary,
        ),
      )],
      HardwarePage::Power if !self.snapshot.energy.profiles.is_empty() => vec![(
        ActionButton::ApplyProfile,
        Button::new(
          tr(self.lang, "Aplicar Perfil", "Apply Profile"),
          ButtonKind::Primary,
        ),
      )],
      _ => vec![],
    }
  }

  pub fn breadcrumb(&self) -> String {
    let root = tr(self.lang, "Hardware", "Hardware");
    if self.page == HardwarePage::Home {
      root.into()
    } else {
      format!("{root} > {}", self.label(self.page))
    }
  }

  fn label(&self, page: HardwarePage) -> &'static str {
    match page {
      HardwarePage::Summary => tr(self.lang, "Resumo", "Summary"),
      HardwarePage::Cpu => "CPU",
      HardwarePage::Gpu => "GPU",
      HardwarePage::Memory => tr(self.lang, "Memória", "Memory"),
      HardwarePage::Power => tr(self.lang, "Energia", "Power"),
      HardwarePage::Devices => tr(self.lang, "Dispositivos", "Devices"),
      HardwarePage::Home => "Hardware",
      HardwarePage::DeviceDetail(_) => tr(self.lang, "Detalhes", "Details"),
      HardwarePage::GpuDetail(_) => tr(self.lang, "GPU > Detalhes", "GPU > Details"),
    }
  }

  pub fn draw(&mut self, frame: &mut Frame) {
    let area = frame.area();
    let hints = if self.on_buttons {
      tr(
        self.lang,
        "←/→ Selecionar botão   Enter Confirmar   Tab Alternar foco   Esc Voltar",
        "←/→ Select button   Enter Confirm   Tab Switch focus   Esc Back",
      )
    } else if matches!(
      self.page,
      HardwarePage::Summary
        | HardwarePage::Memory
        | HardwarePage::GpuDetail(_)
        | HardwarePage::DeviceDetail(_)
    ) {
      tr(
        self.lang,
        "↑/↓ Rolar   r Atualizar   ←/Esc Voltar   ? Ajuda",
        "↑/↓ Scroll   r Refresh   ←/Esc Back   ? Help",
      )
    } else if matches!(self.page, HardwarePage::Cpu) {
      tr(
        self.lang,
        "↑/↓ Navegar   Enter/g Aplicar   Tab Foco nos botões   ←/Esc Voltar   r Atualizar",
        "↑/↓ Navigate   Enter/g Apply   Tab Focus buttons   ←/Esc Back   r Refresh",
      )
    } else if matches!(self.page, HardwarePage::Power) {
      tr(
        self.lang,
        "↑/↓ Navegar   Enter/e Aplicar   Tab Foco nos botões   ←/Esc Voltar   r Atualizar",
        "↑/↓ Navigate   Enter/e Apply   Tab Focus buttons   ←/Esc Back   r Refresh",
      )
    } else {
      tr(
        self.lang,
        "↑/↓ Navegar   →/Enter Abrir   ←/Esc Voltar   r Atualizar   ? Ajuda",
        "↑/↓ Navigate   →/Enter Open   ←/Esc Back   r Refresh   ? Help",
      )
    };

    let body = shell(frame, area, &self.theme, &self.breadcrumb(), hints);
    let buttons = self.buttons();

    let (content_area, button_area) = if buttons.is_empty() {
      (body, None)
    } else {
      let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(body);
      (chunks[0], Some(chunks[1]))
    };

    if matches!(
      self.page,
      HardwarePage::Summary
        | HardwarePage::Memory
        | HardwarePage::GpuDetail(_)
        | HardwarePage::DeviceDetail(_)
    ) {
      readonly(frame, content_area, &self.theme, &detail_lines(self));
    } else {
      let rows = self.rows();
      list(
        frame,
        content_area,
        &self.theme,
        &rows,
        self.selected.index.min(rows.len().saturating_sub(1)),
      );
    }

    if let Some(btn_area) = button_area {
      let raw_buttons: Vec<Button> = buttons.iter().map(|(_, b)| b.clone()).collect();
      let active_idx = if self.on_buttons {
        self.button_selected
      } else {
        usize::MAX
      };
      argvus_tui::buttons::draw(frame, btn_area, &raw_buttons, active_idx, &self.theme);
    }

    if let Some(Pending::Action(action)) = &self.pending {
      let message = match action {
        HardwareAction::SetGovernor(gov) => format!(
          "{}: {}",
          tr(
            self.lang,
            "Confirmar alteração de Governor da CPU para",
            "Confirm CPU Governor change to"
          ),
          gov
        ),
        HardwareAction::SetProfile(prof) => format!(
          "{}: {}",
          tr(
            self.lang,
            "Confirmar alteração de Perfil de Energia para",
            "Confirm Power Profile change to"
          ),
          prof
        ),
      };
      draw_confirmation(
        frame,
        area,
        &self.theme,
        ConfirmationDialog {
          title: tr(
            self.lang,
            "Confirmar operação de hardware",
            "Confirm hardware operation",
          ),
          message: &message,
          confirm_label: tr(self.lang, "Continuar", "Continue"),
          cancel_label: tr(self.lang, "Cancelar", "Cancel"),
          confirm_selected: self.confirmation.confirm_selected,
        },
      );
    }

    if let Some(message) = &self.status {
      status(frame, body, &self.theme, message);
    }
  }

  fn rows(&self) -> Vec<String> {
    match self.page {
      HardwarePage::Home => self.home_rows(),
      HardwarePage::Cpu => self
        .snapshot
        .cpu
        .governors
        .iter()
        .map(|gov| {
          let badge = if self.snapshot.cpu.governor.as_deref() == Some(gov) {
            format!("   ★ {}", tr(self.lang, "Ativo", "Active"))
          } else {
            String::new()
          };
          format!("{gov}{badge}")
        })
        .collect(),
      HardwarePage::Gpu => self
        .snapshot
        .gpus
        .iter()
        .map(|gpu| {
          format!(
            "GPU {}  {}  {}",
            gpu.index,
            gpu
              .model
              .as_deref()
              .unwrap_or(tr(self.lang, "Não disponível", "Unavailable")),
            gpu
              .driver
              .as_deref()
              .unwrap_or(tr(self.lang, "sem driver", "no driver")),
          )
        })
        .collect(),
      HardwarePage::Power => self
        .snapshot
        .energy
        .profiles
        .iter()
        .map(|prof| {
          let badge = if self.snapshot.energy.profile.as_deref() == Some(prof) {
            format!("   ★ {}", tr(self.lang, "Ativo", "Active"))
          } else {
            String::new()
          };
          format!("{prof}{badge}")
        })
        .collect(),
      HardwarePage::Devices => self
        .snapshot
        .devices
        .iter()
        .map(|dev| format!("[{}] {}", dev.kind, dev.name))
        .collect(),
      _ => vec![],
    }
  }

  fn home_rows(&self) -> Vec<String> {
    let summary_label = tr(self.lang, "Resumo", "Summary");
    let cpu_label = "CPU";
    let gpu_label = "GPU";
    let memory_label = tr(self.lang, "Memória", "Memory");
    let power_label = tr(self.lang, "Energia", "Power");
    let devices_label = tr(self.lang, "Dispositivos", "Devices");

    let cpu_str = self
      .snapshot
      .cpu
      .model
      .clone()
      .unwrap_or_else(|| na(self.lang));

    let gpu_str = if self.snapshot.gpus.is_empty() {
      tr(self.lang, "Nenhuma GPU", "No GPU").into()
    } else {
      format!("{} GPU(s)", self.snapshot.gpus.len())
    };

    let mem_str = format_kib(self.snapshot.memory.total_kib);

    let power_str = self
      .snapshot
      .energy
      .profile
      .clone()
      .unwrap_or_else(|| tr(self.lang, "Padrão", "Default").into());

    let dev_str = format!(
      "{} {}",
      self.snapshot.devices.len(),
      tr(self.lang, "dispositivos", "devices")
    );

    vec![
      format!(
        "{} {}  ·  {} · {}",
        AppConfig::icon("💻"),
        summary_label,
        self.snapshot.architecture,
        self.snapshot.kernel
      ),
      format!(
        "{} {}  ·  {} · {}",
        AppConfig::icon("🧠"),
        cpu_label,
        cpu_str,
        self.snapshot.cpu.governor.as_deref().unwrap_or("—")
      ),
      format!("{} {}  ·  {}", AppConfig::icon("🎮"), gpu_label, gpu_str),
      format!("{} {}  ·  {}", AppConfig::icon("⚡"), memory_label, mem_str),
      format!(
        "{} {}  ·  {}",
        AppConfig::icon("🔋"),
        power_label,
        power_str
      ),
      format!(
        "{} {}  ·  {}",
        AppConfig::icon("🔌"),
        devices_label,
        dev_str
      ),
    ]
  }
}

fn detail_lines(app: &HardwareApp) -> Vec<Line<'static>> {
  let s = &app.snapshot;
  let mut lines = Vec::new();
  let mut add = |key: &str, value: String| {
    lines.push(Line::from(format!(
      "{:<20} {value}",
      field_label(app.lang, key)
    )))
  };
  match app.page {
    HardwarePage::Summary => {
      add(
        "Manufacturer",
        s.manufacturer.clone().unwrap_or_else(|| na(app.lang)),
      );
      add("Model", s.model.clone().unwrap_or_else(|| na(app.lang)));
      add("Chassis", s.chassis.clone().unwrap_or_else(|| na(app.lang)));
      add("Architecture", s.architecture.clone());
      add("Kernel", s.kernel.clone());
      add("CPU", s.cpu.model.clone().unwrap_or_else(|| na(app.lang)));
      add(
        "Firmware",
        s.firmware.clone().unwrap_or_else(|| na(app.lang)),
      );
      add(
        "Threads",
        s.cpu
          .threads
          .map(|v| v.to_string())
          .unwrap_or_else(|| na(app.lang)),
      );
      add(
        "Physical cores",
        s.cpu
          .physical_cores
          .map(|v| v.to_string())
          .unwrap_or_else(|| na(app.lang)),
      );
      add(
        "Sockets",
        s.cpu
          .sockets
          .map(|v| v.to_string())
          .unwrap_or_else(|| na(app.lang)),
      );
      add("GPU(s)", s.gpus.len().to_string());
      add("RAM", format_kib(s.memory.total_kib));
      add("Boot", s.boot.clone());
      add(
        "Virtualization",
        s.virtualization
          .clone()
          .unwrap_or_else(|| tr(app.lang, "Nenhuma detectada", "None detected").into()),
      );
      add(
        "Battery",
        if s.battery.is_some() {
          tr(app.lang, "Presente", "Present").into()
        } else {
          tr(app.lang, "Nenhuma detectada", "None detected").into()
        },
      );
    }
    HardwarePage::Cpu => {
      add(
        "Vendor",
        s.cpu.vendor.clone().unwrap_or_else(|| na(app.lang)),
      );
      add("Model", s.cpu.model.clone().unwrap_or_else(|| na(app.lang)));
      add(
        "Threads",
        s.cpu
          .threads
          .map(|v| v.to_string())
          .unwrap_or_else(|| na(app.lang)),
      );
      add(
        "Current",
        s.cpu
          .current_mhz
          .map(|v| format!("{v} MHz"))
          .unwrap_or_else(|| na(app.lang)),
      );
      add(
        "Min/Max",
        format!(
          "{} / {} MHz",
          s.cpu
            .min_mhz
            .map(|v| v.to_string())
            .unwrap_or_else(|| na(app.lang)),
          s.cpu
            .max_mhz
            .map(|v| v.to_string())
            .unwrap_or_else(|| na(app.lang))
        ),
      );
      add(
        "Governor",
        s.cpu.governor.clone().unwrap_or_else(|| na(app.lang)),
      );
      add(
        "Available",
        if s.cpu.governors.is_empty() {
          na(app.lang)
        } else {
          s.cpu.governors.join(", ")
        },
      );
      add(
        "Scaling driver",
        s.cpu.driver.clone().unwrap_or_else(|| na(app.lang)),
      );
      add(
        "Management",
        s.cpu.management.clone().unwrap_or_else(|| na(app.lang)),
      );
    }
    HardwarePage::Gpu | HardwarePage::GpuDetail(_) => {
      let gpus: Vec<&crate::model::GpuInfo> = match app.page {
        HardwarePage::GpuDetail(index) => s.gpus.get(index).into_iter().collect(),
        _ => s.gpus.iter().collect(),
      };
      for gpu in gpus {
        add(
          &format!("GPU {}", gpu.index),
          gpu
            .model
            .clone()
            .or(gpu.vendor.clone())
            .unwrap_or_else(|| na(app.lang)),
        );
        add("PCI", gpu.pci.clone().unwrap_or_else(|| na(app.lang)));
        add(
          "Vendor ID",
          gpu.vendor_id.clone().unwrap_or_else(|| na(app.lang)),
        );
        add(
          "Device ID",
          gpu.device_id.clone().unwrap_or_else(|| na(app.lang)),
        );
        add("Driver", gpu.driver.clone().unwrap_or_else(|| na(app.lang)));
        add("Module", gpu.module.clone().unwrap_or_else(|| na(app.lang)));
        add(
          "Driver status",
          gpu.driver_status.clone().unwrap_or_else(|| na(app.lang)),
        );
        add("DRM", gpu.drm.clone().unwrap_or_else(|| na(app.lang)));
        add("Render", gpu.render.clone().unwrap_or_else(|| na(app.lang)));
        add("OpenGL", gpu.opengl.clone().unwrap_or_else(|| na(app.lang)));
        add("Vulkan", gpu.vulkan.clone().unwrap_or_else(|| na(app.lang)));
      }
      if s.gpus.iter().any(|g| g.driver.as_deref() == Some("vmwgfx")) && s.virtualization.is_some()
      {
        add(
          "Note",
          tr(
            app.lang,
            "vmwgfx detectado em máquina virtual.",
            "vmwgfx detected in virtual machine.",
          )
          .into(),
        );
      }
      add(
        "OpenGL software",
        if s.software_rendering {
          tr(app.lang, "ativa", "active").into()
        } else {
          tr(app.lang, "inativa", "inactive").into()
        },
      );
    }
    HardwarePage::Memory => {
      add("Total", format_kib(s.memory.total_kib));
      add("Used", format_kib(s.memory.used_kib));
      add("Available", format_kib(s.memory.available_kib));
      add("Swap total", format_kib(s.memory.swap_total_kib));
      add("Swap free", format_kib(s.memory.swap_free_kib));
    }
    HardwarePage::Power => {
      add(
        "Backend",
        s.energy.backend.clone().unwrap_or_else(|| na(app.lang)),
      );
      add(
        "Profile",
        s.energy.profile.clone().unwrap_or_else(|| na(app.lang)),
      );
      if s.energy.profiles.is_empty() {
        add(
          "Profiles",
          tr(
            app.lang,
            "Nenhum perfil disponível.",
            "No profile available.",
          )
          .into(),
        );
      }
      if let Some(battery) = &s.battery {
        add("Battery", battery.device.clone());
        if let Some(manufacturer) = &battery.manufacturer {
          add("Manufacturer", manufacturer.clone());
        }
        if let Some(model) = &battery.model {
          add("Model", model.clone());
        }
        add(
          "Status",
          battery.status.clone().unwrap_or_else(|| na(app.lang)),
        );
        add(
          "Capacity",
          battery
            .percent
            .map(|v| format!("{v}%"))
            .unwrap_or_else(|| na(app.lang)),
        );
        add(
          "Health",
          battery
            .health
            .map(|v| format!("{v:.1}%"))
            .unwrap_or_else(|| na(app.lang)),
        );
        if let Some(ac) = battery.ac_online {
          add(
            "AC",
            if ac {
              tr(app.lang, "Conectada", "Connected").into()
            } else {
              tr(app.lang, "Desconectada", "Disconnected").into()
            },
          );
        }
      } else {
        add(
          "Battery",
          tr(
            app.lang,
            "Nenhuma bateria detectada.",
            "No battery detected.",
          )
          .into(),
        );
      }
    }
    HardwarePage::Devices => {
      for device in &s.devices {
        add(&device.kind, device.name.clone());
      }
      if s.devices.is_empty() {
        add(
          "Devices",
          tr(
            app.lang,
            "Nenhum dispositivo detectado.",
            "No device detected.",
          )
          .into(),
        );
      }
    }
    HardwarePage::DeviceDetail(index) => {
      if let Some(device) = s.devices.get(index) {
        add("Name", device.name.clone());
        add("Type", device.kind.clone());
        add(
          "Vendor",
          device.vendor.clone().unwrap_or_else(|| na(app.lang)),
        );
        add(
          "Driver",
          device.driver.clone().unwrap_or_else(|| na(app.lang)),
        );
        add(
          "Product",
          device.product.clone().unwrap_or_else(|| na(app.lang)),
        );
        add("Bus", device.bus.clone().unwrap_or_else(|| na(app.lang)));
        add("Path", device.path.clone().unwrap_or_else(|| na(app.lang)));
      }
    }
    HardwarePage::Home => {}
  }
  lines
}

fn run_governor(value: &str) -> Result<String, String> {
  let request = PrivilegedRequest::new("governor", "set", vec![value.into()])?;
  let executable = std::env::current_exe()
    .map_err(|error| error.to_string())?
    .to_string_lossy()
    .into_owned();
  let output = SystemSettingsOperation::new(SystemProcessRunner, executable).execute(&request)?;
  output
    .status
    .is_some_and(|v| v == 0)
    .then(|| format!("Governor: {value}"))
    .ok_or_else(|| String::from_utf8_lossy(&output.stderr).trim().into())
}

fn field_label(lang: Lang, key: &str) -> String {
  let (pt, en) = match key {
    "Manufacturer" => ("Fabricante", "Manufacturer"),
    "Model" => ("Modelo", "Model"),
    "Chassis" => ("Chassis", "Chassis"),
    "Architecture" => ("Arquitetura", "Architecture"),
    "Kernel" => ("Kernel", "Kernel"),
    "CPU" => ("CPU", "CPU"),
    "Threads" => ("Threads", "Threads"),
    "GPU(s)" => ("GPU(s)", "GPU(s)"),
    "RAM" => ("RAM", "RAM"),
    "Boot" => ("Boot", "Boot"),
    "Virtualization" => ("Virtualização", "Virtualization"),
    "Battery" => ("Bateria", "Battery"),
    "Firmware" => ("Firmware", "Firmware"),
    "AC" => ("Energia AC", "AC power"),
    "Management" => ("Gerenciamento", "Management"),
    "Vendor" => ("Fabricante", "Vendor"),
    "Current" => ("Atual", "Current"),
    "Min/Max" => ("Mín/Máx", "Min/Max"),
    "Governor" => ("Governor", "Governor"),
    "Available" => ("Disponíveis", "Available"),
    "Scaling driver" => ("Driver de escala", "Scaling driver"),
    "Vendor ID" => ("ID do fabricante", "Vendor ID"),
    "Device ID" => ("ID do dispositivo", "Device ID"),
    "Driver" => ("Driver", "Driver"),
    "DRM" => ("DRM", "DRM"),
    "Render" => ("Render", "Render"),
    "OpenGL software" => ("OpenGL por software", "OpenGL software"),
    "Total" => ("Total", "Total"),
    "Used" => ("Usada", "Used"),
    "Swap total" => ("Swap total", "Swap total"),
    "Swap free" => ("Swap livre", "Swap free"),
    "Backend" => ("Backend", "Backend"),
    "Profile" => ("Perfil", "Profile"),
    "Profiles" => ("Perfis", "Profiles"),
    "Status" => ("Estado", "Status"),
    "Capacity" => ("Capacidade", "Capacity"),
    "Health" => ("Saúde", "Health"),
    "Name" => ("Nome", "Name"),
    "Type" => ("Tipo", "Type"),
    "Product" => ("Produto", "Product"),
    "Bus" => ("Barramento", "Bus"),
    "Path" => ("Caminho", "Path"),
    "Module" => ("Módulo", "Module"),
    "Driver status" => ("Estado do driver", "Driver status"),
    "Devices" => ("Dispositivos", "Devices"),
    _ => (key, key),
  };
  tr(lang, pt, en).into()
}

fn na(lang: Lang) -> String {
  tr(lang, "Não disponível", "Unavailable").into()
}

fn run_profile(value: &str) -> Result<String, String> {
  let output = SystemProcessRunner
    .run(
      &ProcessRequest::new("powerprofilesctl")
        .arg("set")
        .arg(value),
    )
    .map_err(|e| e.to_string())?;
  output
    .status
    .is_some_and(|v| v == 0)
    .then(|| format!("Profile: {value}"))
    .ok_or_else(|| String::from_utf8_lossy(&output.stderr).trim().into())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn home_selection_opens_cpu_without_render_time_probe() {
    let mut app = HardwareApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.job = None;
    app.handle(KeyCode::Down);
    app.handle(KeyCode::Enter);
    assert_eq!(app.page, HardwarePage::Cpu);
  }

  #[test]
  fn cpu_governor_selection_is_bounded() {
    let mut app = HardwareApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.job = None;
    app.snapshot.cpu.governors = vec!["schedutil".into(), "performance".into()];
    app.page = HardwarePage::Cpu;
    app.handle(KeyCode::End);
    assert_eq!(app.selected.index, 1);
    app.handle(KeyCode::Down);
    assert_eq!(app.selected.index, 1);
  }

  #[test]
  fn cpu_governor_enter_dispatches_even_with_power_profiles_daemon() {
    let cap = Capabilities {
      has_power_profiles_daemon: true,
      ..Capabilities::default()
    };
    let mut app = HardwareApp::new(Lang::En, Theme::load(), cap);
    app.job = None;
    app.action = None;
    app.snapshot.cpu.governors = vec!["performance".into(), "powersave".into()];
    app.page = HardwarePage::Cpu;
    app.handle(KeyCode::Enter);
    assert!(
      app.pending.is_some(),
      "Enter on a CPU governor must trigger confirmation dialog"
    );
  }

  #[test]
  fn cpu_governor_enter_does_not_dispatch_when_no_governors() {
    let mut app = HardwareApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.job = None;
    app.action = None;
    app.page = HardwarePage::Cpu;
    app.handle(KeyCode::Enter);
    assert!(app.pending.is_none());
  }
}
