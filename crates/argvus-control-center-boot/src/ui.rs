use crate::{
  backend::BootBackend,
  model::{BootPage, BootSnapshot, BootloaderKind},
};
use argvus_control_center_core::{
  capabilities::Capabilities,
  config::AppConfig,
  jobs::{JobHandle, JobManager, JobState},
  privileged::{PrivilegedRequest, SystemSettingsOperation},
  process::{LiveProcess, SystemProcessRunner},
  sanitize::terminal_text,
};
use argvus_i18n::{Lang, tr};
use argvus_theme::Theme;
use argvus_tui::{
  buttons::{Button, ButtonKind},
  chrome,
  components::{
    ConfirmationDialog, ConfirmationOutcome, ConfirmationState, StatusKind, StatusMessage,
    draw_confirmation,
  },
  page::{Selection, list, readonly, shell, status},
  text,
};
use crossterm::event::KeyCode;
use ratatui::{
  Frame,
  layout::{Constraint, Layout},
  style::Style,
  text::Line,
  widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

#[derive(Debug, Clone)]
enum Pending {
  Action(BootAction),
}
#[derive(Debug, Clone)]
enum BootAction {
  SystemdDefault(String),
  SystemdTimeout(u32),
  GrubTimeout(u32),
  GrubCmdline(String),
  GrubRegenerate,
  Initramfs,
  Plymouth(String),
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum ActionResult {
  Success,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
  Timeout,
  GrubCmdline,
}

const TRANSACTION_POPUP_WIDTH: u16 = 100;
const TRANSACTION_POPUP_HEIGHT: u16 = 20;
const TRANSACTION_CONTENT_WIDTH: usize = TRANSACTION_POPUP_WIDTH as usize - 2;
const TRANSACTION_CONTENT_HEIGHT: usize = TRANSACTION_POPUP_HEIGHT as usize - 2;

fn transaction_wrapped_lines(output: &str) -> usize {
  output
    .lines()
    .map(|line| (text::display_width(line).div_ceil(TRANSACTION_CONTENT_WIDTH)).max(1))
    .sum()
}

fn transaction_bottom_offset(output: &str) -> u16 {
  transaction_wrapped_lines(output).saturating_sub(TRANSACTION_CONTENT_HEIGHT) as u16
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionButton {
  SetDefault,
  Timeout,
  GrubCmdline,
  Regenerate,
  ApplyTheme,
}

pub struct BootApp {
  pub page: BootPage,
  selected: Selection,
  on_buttons: bool,
  button_selected: usize,
  button_from: Option<usize>,
  snapshot: BootSnapshot,
  job: Option<JobHandle<Result<BootSnapshot, String>>>,
  action: Option<JobHandle<(ActionResult, String)>>,
  pending: Option<Pending>,
  confirmation: ConfirmationState,
  timeout_input: Option<String>,
  input_mode: Option<InputMode>,
  transaction_live: Option<LiveProcess>,
  transaction_open: bool,
  transaction_scroll: u16,
  transaction_follow: bool,
  jobs: JobManager,
  lang: Lang,
  theme: Theme,
  capabilities: Capabilities,
  pub status: Option<StatusMessage>,
}

impl BootApp {
  pub fn new(lang: Lang, theme: Theme, capabilities: Capabilities) -> Self {
    Self {
      page: BootPage::Home,
      selected: Selection::default(),
      on_buttons: false,
      button_selected: 0,
      button_from: None,
      snapshot: Default::default(),
      job: None,
      action: None,
      pending: None,
      confirmation: ConfirmationState::default(),
      timeout_input: None,
      input_mode: None,
      transaction_live: None,
      transaction_open: false,
      transaction_scroll: 0,
      transaction_follow: false,
      jobs: JobManager::default(),
      lang,
      theme,
      capabilities,
      status: None,
    }
  }
  pub fn reload(&mut self) {
    if self.job.is_some() || self.action.is_some() {
      return;
    }
    let capabilities = self.capabilities.clone();
    self.job = Some(self.jobs.spawn(move |_| {
      Ok(Ok(
        BootBackend::new(SystemProcessRunner, capabilities).collect(),
      ))
    }));
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(self.lang, "Carregando boot...", "Loading boot...").into(),
    });
  }
  pub fn poll(&mut self) -> bool {
    let mut changed = false;
    if let Some(job) = &self.job
      && let JobState::Finished(result) = job.try_state()
    {
      self.job = None;
      match result {
        Ok(Ok(snapshot)) => {
          self.snapshot = snapshot;
          self.normalize();
          self.success(tr(self.lang, "Boot atualizado", "Boot refreshed"));
        }
        Ok(Err(error)) | Err(error) => self.error(error),
      }
      changed = true;
    }
    if let Some(live) = &self.transaction_live
      && self.transaction_open
      && self.transaction_follow
      && self
        .action
        .as_ref()
        .is_some_and(|job| matches!(job.try_state(), JobState::Running))
    {
      self.transaction_scroll = transaction_bottom_offset(&live.output());
      changed = true;
    }
    if let Some(job) = &self.action
      && let JobState::Finished(result) = job.try_state()
    {
      self.action = None;
      match result {
        Ok((outcome, _)) => match outcome {
          ActionResult::Success => self.success(tr(
            self.lang,
            "Operação concluída. A alteração será usada no próximo boot.",
            "Operation completed. The change will be used on the next boot.",
          )),
        },
        Err(error) => self.error(error),
      }
      self.reload();
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
    self.selected.normalize(self.selection_len());
  }
  fn selection_len(&self) -> usize {
    match self.page {
      BootPage::Home => 5,
      BootPage::Kernel => self.snapshot.kernels.len(),
      BootPage::Bootloader => self.snapshot.bootloader_info.entries.len(),
      BootPage::Initramfs => self.snapshot.initramfs.presets.len() + 1,
      BootPage::Plymouth => self.snapshot.plymouth.themes.len(),
      _ => 1,
    }
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
    if self.timeout_input.is_some() {
      match key {
        KeyCode::Char(c)
          if c.is_ascii_digit() && self.timeout_input.as_ref().is_some_and(|v| v.len() < 2) =>
        {
          self.timeout_input.as_mut().unwrap().push(c)
        }
        KeyCode::Backspace => {
          self.timeout_input.as_mut().unwrap().pop();
        }
        KeyCode::Enter => self.apply_input(),
        KeyCode::Esc => {
          self.timeout_input = None;
          self.input_mode = None;
        }
        _ => {}
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
      if self.page == BootPage::Home {
        return true;
      }
      self.page = match self.page {
        BootPage::KernelDetail(_) => BootPage::Kernel,
        BootPage::BootloaderDetail(_) => BootPage::Bootloader,
        BootPage::InitramfsDetail(_) => BootPage::Initramfs,
        _ => BootPage::Home,
      };
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
        self.selected.handle(key, self.selection_len(), 8);
      }
      KeyCode::Enter | KeyCode::Right => self.open_selected(),
      _ => {}
    }
    false
  }
  fn open_selected(&mut self) {
    match self.page {
      BootPage::Home => {
        self.page = match self.selected.index {
          0 => BootPage::Summary,
          1 => BootPage::Kernel,
          2 => BootPage::Bootloader,
          3 => BootPage::Initramfs,
          _ => BootPage::Plymouth,
        };
        self.selected.index = 0;
        self.reload();
      }
      BootPage::Kernel if self.snapshot.kernels.get(self.selected.index).is_some() => {
        self.page = BootPage::KernelDetail(self.selected.index);
      }
      BootPage::Bootloader
        if self
          .snapshot
          .bootloader_info
          .entries
          .get(self.selected.index)
          .is_some() =>
      {
        self.page = BootPage::BootloaderDetail(self.selected.index);
      }
      BootPage::Initramfs
        if self
          .snapshot
          .initramfs
          .presets
          .get(self.selected.index)
          .is_some() =>
      {
        self.page = BootPage::InitramfsDetail(self.selected.index);
      }
      BootPage::Initramfs => {
        self.confirmation = ConfirmationState::default();
        self.pending = Some(Pending::Action(BootAction::Initramfs));
      }
      BootPage::Plymouth => self.request_plymouth(),
      _ => {}
    }
  }
  fn request_default(&mut self) {
    let action = match self.page {
      BootPage::Kernel => self
        .snapshot
        .kernels
        .get(self.selected.index)
        .and_then(|kernel| self.entry_for_kernel(kernel))
        .map(|entry| BootAction::SystemdDefault(entry.id.clone())),
      BootPage::KernelDetail(index) => self
        .snapshot
        .kernels
        .get(index)
        .and_then(|kernel| self.entry_for_kernel(kernel))
        .map(|entry| BootAction::SystemdDefault(entry.id.clone())),
      BootPage::Bootloader => self
        .snapshot
        .bootloader_info
        .entries
        .get(self.selected.index)
        .filter(|_| self.snapshot.bootloader == BootloaderKind::SystemdBoot)
        .map(|entry| BootAction::SystemdDefault(entry.id.clone())),
      BootPage::BootloaderDetail(index) => self
        .snapshot
        .bootloader_info
        .entries
        .get(index)
        .filter(|_| self.snapshot.bootloader == BootloaderKind::SystemdBoot)
        .map(|entry| BootAction::SystemdDefault(entry.id.clone())),
      _ => None,
    };
    if let Some(action) = action {
      self.pending = Some(Pending::Action(action));
    } else {
      self.error(tr(
        self.lang,
        "Não foi possível mapear uma entrada de boot com segurança.",
        "A boot entry could not be mapped safely.",
      ));
    }
  }
  fn entry_for_kernel(
    &self,
    kernel: &crate::model::KernelInfo,
  ) -> Option<&crate::model::BootEntry> {
    (self.snapshot.bootloader == BootloaderKind::SystemdBoot)
      .then_some(())
      .and_then(|_| {
        self.snapshot.bootloader_info.entries.iter().find(|entry| {
          entry
            .linux
            .as_deref()
            .is_some_and(|linux| linux.ends_with(&format!("vmlinuz-{}", kernel.package)))
        })
      })
  }
  fn apply_timeout_input(&mut self) {
    let value = self.timeout_input.take().unwrap_or_default();
    let Ok(seconds) = value.parse::<u32>() else {
      self.error(tr(self.lang, "Timeout inválido.", "Invalid timeout."));
      return;
    };
    if seconds > 60 {
      self.error(tr(
        self.lang,
        "O timeout deve estar entre 0 e 60 segundos.",
        "Timeout must be between 0 and 60 seconds.",
      ));
      return;
    }
    let action = match self.snapshot.bootloader {
      BootloaderKind::SystemdBoot => BootAction::SystemdTimeout(seconds),
      BootloaderKind::Grub => BootAction::GrubTimeout(seconds),
      BootloaderKind::Unknown => {
        self.error(tr(
          self.lang,
          "Bootloader indeterminado; nenhuma alteração foi feita.",
          "Bootloader is undetermined; no change was made.",
        ));
        return;
      }
    };
    self.pending = Some(Pending::Action(action));
  }
  fn apply_input(&mut self) {
    if self.input_mode == Some(InputMode::GrubCmdline) {
      let value = self.timeout_input.take().unwrap_or_default();
      self.input_mode = None;
      if value.len() > 2048
        || value
          .chars()
          .any(|character| character.is_control() || matches!(character, '"' | '\\'))
      {
        self.error(tr(
          self.lang,
          "Linha de kernel inválida.",
          "Invalid kernel command line.",
        ));
      } else {
        self.pending = Some(Pending::Action(BootAction::GrubCmdline(value)));
      }
    } else {
      self.apply_timeout_input();
      self.input_mode = None;
    }
  }
  fn grub_value(&self, key: &str) -> String {
    self
      .snapshot
      .bootloader_info
      .grub_values
      .iter()
      .find_map(|(name, value)| (name == key).then(|| value.clone()))
      .unwrap_or_default()
  }
  fn request_plymouth(&mut self) {
    if let Some(theme) = self.snapshot.plymouth.themes.get(self.selected.index) {
      self.pending = Some(Pending::Action(BootAction::Plymouth(theme.clone())));
    }
  }
  fn start_pending(&mut self) {
    let Some(Pending::Action(action)) = self.pending.take() else {
      return;
    };
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(
        self.lang,
        "Aplicando alteração de boot...",
        "Applying boot change...",
      )
      .into(),
    });
    let live = LiveProcess::new();
    self.transaction_live = Some(live.clone());
    self.transaction_open = true;
    self.transaction_scroll = 0;
    self.transaction_follow = true;
    self.action = Some(self.jobs.spawn(move |_| run_action(action, live)));
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
      ActionButton::Timeout => {
        self.timeout_input = Some(String::new());
        self.input_mode = Some(InputMode::Timeout);
      }
      ActionButton::GrubCmdline => {
        self.timeout_input = Some(self.grub_value("GRUB_CMDLINE_LINUX_DEFAULT"));
        self.input_mode = Some(InputMode::GrubCmdline);
      }
      ActionButton::Regenerate => {
        self.pending = Some(
          if self.page == BootPage::Bootloader || matches!(self.page, BootPage::BootloaderDetail(_))
          {
            Pending::Action(BootAction::GrubRegenerate)
          } else {
            Pending::Action(BootAction::Initramfs)
          },
        );
      }
      ActionButton::ApplyTheme => self.request_plymouth(),
    }
  }
  fn buttons(&self) -> Vec<(ActionButton, Button)> {
    match self.page {
      BootPage::Kernel if !self.snapshot.kernels.is_empty() => vec![(
        ActionButton::SetDefault,
        Button::new(tr(self.lang, "Padrão", "Default"), ButtonKind::Primary),
      )],
      BootPage::KernelDetail(_) => vec![(
        ActionButton::SetDefault,
        Button::new(tr(self.lang, "Padrão", "Default"), ButtonKind::Primary),
      )],
      BootPage::Bootloader | BootPage::BootloaderDetail(_) => {
        let mut buttons = vec![
          (
            ActionButton::SetDefault,
            Button::new(tr(self.lang, "Padrão", "Default"), ButtonKind::Primary),
          ),
          (
            ActionButton::Timeout,
            Button::new(tr(self.lang, "Timeout", "Timeout"), ButtonKind::Secondary),
          ),
        ];
        if self.snapshot.bootloader == BootloaderKind::Grub {
          buttons.push((
            ActionButton::GrubCmdline,
            Button::new(
              tr(self.lang, "Linha do kernel", "Kernel command line"),
              ButtonKind::Secondary,
            ),
          ));
          buttons.push((
            ActionButton::Regenerate,
            Button::new(
              tr(self.lang, "Regenerar", "Regenerate"),
              ButtonKind::Secondary,
            ),
          ));
        }
        buttons
      }
      BootPage::Initramfs | BootPage::InitramfsDetail(_) => vec![(
        ActionButton::Regenerate,
        Button::new(
          tr(self.lang, "Regenerar", "Regenerate"),
          ButtonKind::Primary,
        ),
      )],
      BootPage::Plymouth if !self.snapshot.plymouth.themes.is_empty() => vec![(
        ActionButton::ApplyTheme,
        Button::new(
          tr(self.lang, "Aplicar tema", "Apply theme"),
          ButtonKind::Primary,
        ),
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
    if self.page == BootPage::Home {
      home
    } else if !self.buttons().is_empty() {
      action
    } else {
      readonly
    }
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
    if self.page == BootPage::Summary {
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
    if let Some(input) = &self.timeout_input {
      let popup = argvus_tui::chrome::centered(area, 48, 7);
      frame.render_widget(Clear, popup);
      frame.render_widget(
        Paragraph::new(vec![
          Line::from(if self.input_mode == Some(InputMode::GrubCmdline) {
            tr(
              self.lang,
              "Nova linha de kernel:",
              "New kernel command line:",
            )
          } else {
            tr(
              self.lang,
              "Novo timeout em segundos:",
              "New timeout in seconds:",
            )
          }),
          Line::from(format!("{input}_")),
          Line::from(tr(
            self.lang,
            "Enter aplicar   Esc cancelar",
            "Enter apply   Esc cancel",
          )),
        ])
        .block(Block::bordered().title(
          if self.input_mode == Some(InputMode::GrubCmdline) {
            tr(self.lang, "Linha do kernel", "Kernel command line")
          } else {
            tr(self.lang, "Timeout", "Timeout")
          },
        )),
        popup,
      );
    }
    if let Some(Pending::Action(action)) = &self.pending {
      let message = action_message(self.lang, action);
      draw_confirmation(
        frame,
        area,
        &self.theme,
        ConfirmationDialog {
          title: tr(
            self.lang,
            "Confirmar operação de boot",
            "Confirm boot operation",
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
    if self.transaction_open
      && let Some(output) = self.transaction_live.as_ref().map(|live| live.output())
    {
      let popup = chrome::centered(
        frame.area(),
        TRANSACTION_POPUP_WIDTH,
        TRANSACTION_POPUP_HEIGHT,
      );
      let total = transaction_wrapped_lines(&output);
      let shown_total = total.max(1);
      let current = (self.transaction_scroll as usize + 1).min(shown_total);
      frame.render_widget(Clear, popup);
      frame.render_widget(
        Paragraph::new(output.as_str())
          .block(
            Block::new()
              .borders(Borders::ALL)
              .title(Line::styled(
                format!(
                  " {} · {} {} {} {} ",
                  tr(self.lang, "Processo", "Process"),
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
  fn rows(&self) -> Vec<String> {
    match self.page {
      BootPage::Home => self.home_rows(),
      BootPage::Summary => {
        let timeout_str = self
          .snapshot
          .bootloader_info
          .timeout
          .map(|t| format!("{} s", t))
          .unwrap_or_else(|| tr(self.lang, "Padrão", "Default").into());
        let secure_boot = self
          .snapshot
          .secure_boot
          .map(|v| yes_no(self.lang, v))
          .unwrap_or_else(|| tr(self.lang, "Indisponível", "Unavailable").into());
        let default_entry = self
          .snapshot
          .bootloader_info
          .entries
          .iter()
          .find(|entry| entry.is_default)
          .map(|entry| entry.title.clone())
          .or_else(|| self.snapshot.bootloader_info.default_entry.clone())
          .unwrap_or_else(|| "—".into());
        let plymouth_theme = self
          .snapshot
          .plymouth
          .current_theme
          .clone()
          .unwrap_or_else(|| "—".into());

        vec![
          format!(
            " {} {}",
            AppConfig::icon("💻"),
            tr(self.lang, "SISTEMA BASE", "BASE SYSTEM")
          ),
          format!("   Firmware:    {}", self.snapshot.firmware),
          format!("   Secure Boot: {}", secure_boot),
          format!("   Kernel:      ★ {}", self.snapshot.current_kernel),
          "".into(),
          format!(
            " {} {}",
            AppConfig::icon("💿"),
            tr(self.lang, "BOOTLOADER", "BOOTLOADER")
          ),
          format!("   Gerenciador: {}", self.loader_label()),
          format!("   Padrão:      {}", default_entry),
          format!(
            "   Entradas:    {}",
            self.snapshot.bootloader_info.entries.len()
          ),
          format!(
            "   ESP Path:    {}",
            self.snapshot.esp.as_deref().unwrap_or("—")
          ),
          format!("   Timeout:     {}", timeout_str),
          "".into(),
          format!(
            " {} {}",
            AppConfig::icon("📦"),
            tr(self.lang, "COMPONENTES", "COMPONENTS")
          ),
          format!("   Kernels:     {}", self.snapshot.kernels.len()),
          format!(
            "   mkinitcpio:  {}",
            yes_no(self.lang, self.snapshot.initramfs.available)
          ),
          format!(
            "   Plymouth:    {} (Tema: {})",
            yes_no(self.lang, self.snapshot.plymouth.installed),
            plymouth_theme
          ),
        ]
      }
      BootPage::Kernel => self
        .snapshot
        .kernels
        .iter()
        .map(|k| {
          let badges = match (k.current, k.default) {
            (true, true) => format!(
              "   ★ {} · ● {}",
              tr(self.lang, "Atual", "Current"),
              tr(self.lang, "Padrão", "Default")
            ),
            (true, false) => format!("   ★ {}", tr(self.lang, "Atual", "Current")),
            (false, true) => format!("   ● {}", tr(self.lang, "Padrão", "Default")),
            (false, false) => String::new(),
          };
          if badges.is_empty() {
            format!("{} {}", k.package, k.version.trim())
          } else {
            format!("{} {}{}", k.package, k.version.trim(), badges)
          }
        })
        .collect(),
      BootPage::KernelDetail(index) => self.kernel_detail(index),
      BootPage::Bootloader => self
        .snapshot
        .bootloader_info
        .entries
        .iter()
        .map(|e| {
          let badge = if e.is_default {
            format!("   ● {}", tr(self.lang, "Padrão", "Default"))
          } else {
            String::new()
          };
          format!("{}{}", e.title, badge)
        })
        .collect(),
      BootPage::BootloaderDetail(index) => self.bootloader_detail(index),
      BootPage::Initramfs => {
        let mut rows = self.snapshot.initramfs.presets.clone();
        rows.push(
          tr(
            self.lang,
            "Regenerar todos os initramfs",
            "Regenerate all initramfs images",
          )
          .into(),
        );
        rows
      }
      BootPage::InitramfsDetail(index) => self.initramfs_detail(index),
      BootPage::Plymouth => self
        .snapshot
        .plymouth
        .themes
        .iter()
        .map(|theme| {
          let current = if self.snapshot.plymouth.current_theme.as_deref() == Some(theme) {
            format!("   ★ {}", tr(self.lang, "Atual", "Current"))
          } else {
            String::new()
          };
          format!("{theme}{current}")
        })
        .collect(),
    }
  }
  fn home_rows(&self) -> Vec<String> {
    let secure = match self.snapshot.secure_boot {
      Some(value) => format!(
        "{}: {}",
        tr(self.lang, "Secure Boot", "Secure Boot"),
        yes_no(self.lang, value)
      ),
      None => tr(self.lang, "Secure Boot: N/D", "Secure Boot: N/A").into(),
    };
    let timeout = self
      .snapshot
      .bootloader_info
      .timeout
      .map(|t| format!("{}: {} s", tr(self.lang, "Timeout", "Timeout"), t))
      .unwrap_or_else(|| tr(self.lang, "Timeout: Padrão", "Timeout: Default").into());
    let loader = match self.snapshot.bootloader {
      BootloaderKind::SystemdBoot => "systemd-boot",
      BootloaderKind::Grub => "GRUB",
      BootloaderKind::Unknown => tr(self.lang, "Indeterminado", "Unknown"),
    };
    let initramfs = if self.snapshot.initramfs.available {
      format!(
        "{}: {}",
        yes_no(self.lang, true),
        self.snapshot.initramfs.presets.len()
      )
    } else {
      tr(self.lang, "Não instalado", "Not installed").into()
    };
    let plymouth = match &self.snapshot.plymouth.current_theme {
      Some(theme) => theme.clone(),
      None if self.snapshot.plymouth.installed => tr(self.lang, "Instalado", "Installed").into(),
      None => tr(self.lang, "Não instalado", "Not installed").into(),
    };
    let summary = tr(self.lang, "Resumo", "Summary");
    let kernel_label = tr(self.lang, "Kernels", "Kernels");
    let bootloader_label = tr(self.lang, "Bootloader", "Bootloader");
    let initramfs_label = tr(self.lang, "Initramfs", "Initramfs");
    let plymouth_label = tr(self.lang, "Plymouth", "Plymouth");
    vec![
      format!(
        "{} {}  ·  {} · {}",
        AppConfig::icon("💻"),
        summary,
        self.snapshot.firmware,
        secure
      ),
      format!(
        "{} {}  ·  {} · {}",
        AppConfig::icon("🧠"),
        kernel_label,
        self.snapshot.kernels.len(),
        self.snapshot.current_kernel
      ),
      format!(
        "{} {}  ·  {} · {}",
        AppConfig::icon("💿"),
        bootloader_label,
        loader,
        timeout
      ),
      format!(
        "{} {}  ·  {}",
        AppConfig::icon("📦"),
        initramfs_label,
        initramfs
      ),
      format!(
        "{} {}  ·  {}",
        AppConfig::icon("🎨"),
        plymouth_label,
        plymouth
      ),
    ]
  }
  fn kernel_detail(&self, index: usize) -> Vec<String> {
    let Some(k) = self.snapshot.kernels.get(index) else {
      return vec![tr(self.lang, "Kernel não encontrado", "Kernel not found").into()];
    };
    let status = match (k.current, k.default) {
      (true, true) => format!(
        "★ {}  ·  ● {}",
        tr(self.lang, "Atual", "Current"),
        tr(self.lang, "Padrão", "Default")
      ),
      (true, false) => format!("★ {}", tr(self.lang, "Atual", "Current")),
      (false, true) => format!("● {}", tr(self.lang, "Padrão", "Default")),
      (false, false) => "—".into(),
    };
    vec![
      format!(
        " {} {}",
        AppConfig::icon("🧠"),
        tr(self.lang, "KERNEL", "KERNEL")
      ),
      format!(
        "   {:<12} {}",
        tr(self.lang, "Pacote:", "Package:"),
        k.package
      ),
      format!(
        "   {:<12} {}",
        tr(self.lang, "Versão:", "Version:"),
        k.version
      ),
      format!("   {:<12} {}", tr(self.lang, "Status:", "Status:"), status),
      "".into(),
      format!(
        "   {:<12} {}",
        tr(self.lang, "Imagem:", "Image:"),
        k.image.as_deref().unwrap_or("—")
      ),
      format!(
        "   {:<12} {}",
        tr(self.lang, "Initramfs:", "Initramfs:"),
        k.initramfs.as_deref().unwrap_or("—")
      ),
      format!(
        "   {:<12} {}",
        tr(self.lang, "Fallback:", "Fallback:"),
        k.fallback.as_deref().unwrap_or("—")
      ),
      format!(
        "   {:<12} {}",
        tr(self.lang, "Headers:", "Headers:"),
        yes_no(self.lang, k.headers)
      ),
      format!(
        "   {:<12} {}",
        tr(self.lang, "Preset:", "Preset:"),
        k.preset.as_deref().unwrap_or("—")
      ),
      format!(
        "   {:<12} {}",
        tr(self.lang, "UKI:", "UKI:"),
        k.uki.as_deref().unwrap_or("—")
      ),
      "".into(),
      tr(
        self.lang,
        "   [ d ] Definir como padrão no próximo boot",
        "   [ d ] Set as default for the next boot",
      )
      .into(),
    ]
  }
  fn bootloader_detail(&self, index: usize) -> Vec<String> {
    let Some(e) = self.snapshot.bootloader_info.entries.get(index) else {
      return vec![tr(self.lang, "Entrada não encontrada", "Entry not found").into()];
    };
    vec![
      format!(
        " 💿 {}",
        tr(self.lang, "ENTRADA DO BOOTLOADER", "BOOTLOADER ENTRY")
      ),
      format!("   {:<12} {}", tr(self.lang, "Título:", "Title:"), e.title),
      format!("   {:<12} {}", tr(self.lang, "ID:", "ID:"), e.id),
      format!(
        "   {:<12} {}",
        tr(self.lang, "Status:", "Status:"),
        if e.is_default {
          format!("● {}", tr(self.lang, "Padrão", "Default"))
        } else {
          "—".into()
        }
      ),
      "".into(),
      format!(
        "   {:<12} {}",
        tr(self.lang, "Linux/EFI:", "Linux/EFI:"),
        e.linux.as_deref().unwrap_or("—")
      ),
      format!(
        "   {:<12} {}",
        tr(self.lang, "Initrd:", "Initrd:"),
        e.initrd.join(" ")
      ),
      format!(
        "   {:<12} {}",
        tr(self.lang, "Options:", "Options:"),
        e.options.as_deref().unwrap_or("—")
      ),
      "".into(),
      tr(
        self.lang,
        "   [ d ] Definir padrão     [ t ] Alterar timeout",
        "   [ d ] Set default        [ t ] Change timeout",
      )
      .into(),
    ]
  }
  fn initramfs_detail(&self, index: usize) -> Vec<String> {
    let preset = self
      .snapshot
      .initramfs
      .presets
      .get(index)
      .cloned()
      .unwrap_or_else(|| "—".into());
    let config = self
      .snapshot
      .initramfs
      .config_path
      .clone()
      .unwrap_or_else(|| "—".into());
    vec![
      format!(
        " {} {}",
        AppConfig::icon("📦"),
        tr(self.lang, "INITRAMFS", "INITRAMFS")
      ),
      format!("   {:<12} {}", tr(self.lang, "Preset:", "Preset:"), preset),
      format!("   {:<12} {}", tr(self.lang, "Config:", "Config:"), config),
      "".into(),
      format!(
        "   {:<12} {}",
        tr(self.lang, "MODULES:", "MODULES:"),
        self.snapshot.initramfs.modules.join(" ")
      ),
      format!(
        "   {:<12} {}",
        tr(self.lang, "BINARIES:", "BINARIES:"),
        self.snapshot.initramfs.binaries.join(" ")
      ),
      format!(
        "   {:<12} {}",
        tr(self.lang, "FILES:", "FILES:"),
        self.snapshot.initramfs.files.join(" ")
      ),
      format!(
        "   {:<12} {}",
        tr(self.lang, "HOOKS:", "HOOKS:"),
        self.snapshot.initramfs.hooks.join(" ")
      ),
      "".into(),
      tr(
        self.lang,
        "   [ g ] Regenerar todos os initramfs",
        "   [ g ] Regenerate all initramfs images",
      )
      .into(),
    ]
  }
  fn loader_label(&self) -> &'static str {
    match self.snapshot.bootloader {
      BootloaderKind::SystemdBoot => "systemd-boot",
      BootloaderKind::Grub => "GRUB",
      BootloaderKind::Unknown => "Indeterminado",
    }
  }
  fn breadcrumb(&self) -> String {
    let root = tr(self.lang, "Boot", "Boot");
    if self.page == BootPage::Home {
      root.into()
    } else {
      format!("{root} > {}", self.page_label())
    }
  }
  fn page_label(&self) -> &'static str {
    match self.page {
      BootPage::Summary => tr(self.lang, "Resumo", "Summary"),
      BootPage::Kernel | BootPage::KernelDetail(_) => tr(self.lang, "Kernel", "Kernel"),
      BootPage::Bootloader | BootPage::BootloaderDetail(_) => {
        tr(self.lang, "Bootloader", "Bootloader")
      }
      BootPage::Initramfs | BootPage::InitramfsDetail(_) => tr(self.lang, "Initramfs", "Initramfs"),
      BootPage::Plymouth => tr(self.lang, "Plymouth", "Plymouth"),
      BootPage::Home => tr(self.lang, "Boot", "Boot"),
    }
  }
}

fn yes_no(lang: Lang, value: bool) -> String {
  tr(
    lang,
    if value { "Sim" } else { "Não" },
    if value { "Yes" } else { "No" },
  )
  .into()
}
fn action_message(lang: Lang, action: &BootAction) -> String {
  match action {
    BootAction::SystemdDefault(id) => format!(
      "{} '{}' {}",
      tr(lang, "Definir entrada", "Set entry"),
      id,
      tr(
        lang,
        "como padrão no próximo boot?",
        "as default for the next boot?"
      )
    ),
    BootAction::SystemdTimeout(v) | BootAction::GrubTimeout(v) => format!(
      "{} {} s?",
      tr(lang, "Alterar timeout para", "Change timeout to"),
      v
    ),
    BootAction::GrubCmdline(value) => format!(
      "{} '{}' ?",
      tr(lang, "Aplicar linha de kernel", "Apply kernel command line"),
      value
    ),
    BootAction::GrubRegenerate => tr(
      lang,
      "Regenerar a configuração do GRUB?",
      "Regenerate GRUB configuration?",
    )
    .into(),
    BootAction::Initramfs => tr(
      lang,
      "Regenerar todos os initramfs? Uma configuração inválida pode afetar o próximo boot.",
      "Regenerate all initramfs images? An invalid configuration may affect the next boot.",
    )
    .into(),
    BootAction::Plymouth(theme) => format!(
      "{} '{}' {}",
      tr(lang, "Aplicar tema Plymouth", "Apply Plymouth theme"),
      theme,
      tr(
        lang,
        "e regenerar o initramfs?",
        "and regenerate initramfs?"
      )
    ),
  }
}
fn run_action(action: BootAction, live: LiveProcess) -> Result<(ActionResult, String), String> {
  let executable = std::env::current_exe()
    .map_err(|error| error.to_string())?
    .to_string_lossy()
    .into_owned();
  let operation = SystemSettingsOperation::new(SystemProcessRunner, executable);
  let run = |name: &str, args: Vec<String>| -> Result<String, String> {
    let request = PrivilegedRequest::new("boot", name, args)?;
    let output = operation.execute_live(&request, &live)?;
    if output.status == Some(0) {
      let stdout = terminal_text(&String::from_utf8_lossy(&output.stdout))
        .trim()
        .to_owned();
      Ok(if stdout.is_empty() {
        "ok".to_owned()
      } else {
        stdout
      })
    } else {
      Err(
        terminal_text(&String::from_utf8_lossy(&output.stderr))
          .trim()
          .into(),
      )
    }
  };
  match action {
    BootAction::SystemdDefault(id) => {
      live.push_line(&format!("$ bootctl set-default {id}"));
      run("systemd-default", vec![id]).map(|output| (ActionResult::Success, output))
    }
    BootAction::SystemdTimeout(seconds) => {
      live.push_line(&format!("$ boot timeout = {seconds}s"));
      run("systemd-timeout", vec![seconds.to_string()])
        .map(|output| (ActionResult::Success, output))
    }
    BootAction::GrubTimeout(seconds) => {
      live.push_line(&format!("$ GRUB_TIMEOUT = {seconds}s"));
      run("grub-timeout", vec![seconds.to_string()]).map(|output| (ActionResult::Success, output))
    }
    BootAction::GrubCmdline(value) => {
      live.push_line("$ GRUB_CMDLINE_LINUX_DEFAULT");
      run("grub-cmdline", vec![value]).map(|output| (ActionResult::Success, output))
    }
    BootAction::GrubRegenerate => {
      live.push_line("$ grub-mkconfig");
      run("grub-regenerate", vec![]).map(|output| (ActionResult::Success, output))
    }
    BootAction::Initramfs => {
      live.push_line("$ mkinitcpio -P");
      run("initramfs-regenerate", vec![]).map(|output| (ActionResult::Success, output))
    }
    BootAction::Plymouth(theme) => {
      live.push_line(&format!("$ plymouth-set-default-theme -R {theme}"));
      run("plymouth-theme", vec![theme]).map(|output| (ActionResult::Success, output))
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::{BootEntry, KernelInfo};
  use ratatui::{Terminal, backend::TestBackend};

  #[test]
  fn boot_home_rows_act_as_a_status_dashboard() {
    let mut app = BootApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.snapshot.firmware = "UEFI".into();
    app.snapshot.current_kernel = "linux-lts 6.18".into();
    app.snapshot.secure_boot = Some(false);
    app.snapshot.bootloader = BootloaderKind::SystemdBoot;
    app.snapshot.bootloader_info.timeout = Some(3);
    app.snapshot.initramfs.available = true;
    app.snapshot.initramfs.presets = vec!["linux-lts.preset".into()];
    let rows = app.rows();
    assert_eq!(rows.len(), 5);
    assert!(
      rows[0].contains("UEFI") && (rows[0].contains("Resumo") || rows[0].contains("Summary"))
    );
    assert!(rows[1].contains("linux-lts 6.18") && rows[1].contains("Kernels"));
    assert!(rows[2].contains("systemd-boot") && rows[2].contains("3 s"));
    assert!(rows[3].contains("1"));
    assert!(rows[4].contains("Not installed") || rows[4].contains("Não instalado"));
  }

  #[test]
  fn boot_kernel_rows_render_clean_current_and_default_badges() {
    let mut app = BootApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.snapshot.kernels = vec![
      KernelInfo {
        package: "linux".into(),
        version: "6.1".into(),
        current: true,
        ..Default::default()
      },
      KernelInfo {
        package: "linux-lts".into(),
        version: "6.18".into(),
        default: true,
        ..Default::default()
      },
      KernelInfo {
        package: "linux-zen".into(),
        version: "6.19".into(),
        ..Default::default()
      },
    ];
    app.page = BootPage::Kernel;
    let rows = app.rows();
    assert_eq!(rows.len(), 3);
    assert!(rows[0].starts_with("linux 6.1"));
    assert!(rows[0].contains("Current"));
    assert!(rows[1].contains("Default"));
    assert!(!rows[2].contains("Current") && !rows[2].contains("Default"));
    assert!(rows[2].starts_with("linux-zen 6.19"));
  }

  #[test]
  fn boot_detail_pages_render_section_headers_and_aligned_rows() {
    let mut app = BootApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.snapshot.kernels = vec![KernelInfo {
      package: "linux".into(),
      version: "6.1".into(),
      current: true,
      uki: Some("/boot/EFI/Linux/linux.efi".into()),
      ..Default::default()
    }];
    app.page = BootPage::KernelDetail(0);
    let rows = app.rows();
    assert!(rows[0].contains("KERNEL"));
    assert!(rows.iter().any(|r| r.contains("/boot/EFI/Linux/linux.efi")));
    assert!(rows.iter().any(|r| r.contains("Set as default")));
    assert!(rows.len() > 3);
  }

  #[test]
  fn boot_home_and_details_are_keyboard_navigable() {
    let mut app = BootApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.snapshot.kernels = vec![KernelInfo {
      package: "linux".into(),
      version: "6.1".into(),
      ..Default::default()
    }];
    app.handle(KeyCode::Down);
    app.handle(KeyCode::Enter);
    assert_eq!(app.page, BootPage::Kernel);
    app.job = None;
    app.handle(KeyCode::Enter);
    assert_eq!(app.page, BootPage::KernelDetail(0));
    app.handle(KeyCode::Esc);
    assert_eq!(app.page, BootPage::Kernel);
    app.handle(KeyCode::Esc);
    assert_eq!(app.page, BootPage::Home);
  }

  #[test]
  fn systemd_entry_default_requires_a_real_entry() {
    let mut app = BootApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.snapshot.bootloader = BootloaderKind::SystemdBoot;
    app.snapshot.bootloader_info.entries = vec![BootEntry {
      id: "arch.conf".into(),
      title: "Arch".into(),
      ..Default::default()
    }];
    app.page = BootPage::Bootloader;
    app.handle(KeyCode::Enter);
    assert_eq!(app.page, BootPage::BootloaderDetail(0));
    app.handle(KeyCode::Tab);
    assert!(app.on_buttons);
    app.handle(KeyCode::Enter);
    assert!(app.pending.is_some());
    app.handle(KeyCode::Tab);
    assert!(app.confirmation.confirm_selected);
    app.handle(KeyCode::BackTab);
    assert!(!app.confirmation.confirm_selected);
    app.handle(KeyCode::Enter);
    assert!(app.pending.is_none());
    assert!(app.action.is_none());
    app.handle(KeyCode::Enter);
    assert!(app.pending.is_some());
    app.handle(KeyCode::Esc);
    assert!(app.pending.is_none());
  }

  #[test]
  fn timeout_input_is_bounded_and_cancelable() {
    let mut app = BootApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.snapshot.bootloader = BootloaderKind::SystemdBoot;
    app.page = BootPage::Bootloader;
    app.handle(KeyCode::Tab);
    assert!(app.on_buttons);
    app.handle(KeyCode::Right);
    app.handle(KeyCode::Enter);
    assert!(app.timeout_input.is_some());
    app.handle(KeyCode::Char('6'));
    app.handle(KeyCode::Char('1'));
    app.handle(KeyCode::Enter);
    assert!(app.pending.is_none());
    assert!(
      app
        .status
        .as_ref()
        .is_some_and(|status| status.kind == StatusKind::Error)
    );
  }

  #[test]
  fn boot_uses_shared_chrome_and_contextual_footer() {
    let app = BootApp::new(Lang::En, Theme::load(), Capabilities::default());
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
    assert!(text.contains("Boot"));
    assert!(text.contains(app.theme.name.as_str()));
    assert!(text.contains("Enter") && text.contains("Back"));
    assert!(text.contains("Summary") || text.contains("Resumo"));
  }

  #[test]
  fn boot_detail_and_info_pages_expose_buttons() {
    let app = BootApp::new(Lang::En, Theme::load(), Capabilities::default());
    let mut detail = BootApp::new(Lang::En, Theme::load(), Capabilities::default());
    detail.snapshot.bootloader = BootloaderKind::Grub;
    detail.page = BootPage::BootloaderDetail(0);
    assert_eq!(app.buttons().len(), 0);
    assert_eq!(detail.buttons().len(), 4);
  }

  #[test]
  fn boot_tab_cycles_between_list_and_buttons() {
    let mut app = BootApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.snapshot.bootloader = BootloaderKind::Grub;
    app.page = BootPage::Bootloader;
    app.selected.index = 2;
    app.handle(KeyCode::Tab);
    assert!(app.on_buttons);
    app.handle(KeyCode::Tab);
    assert!(!app.on_buttons);
    assert_eq!(app.selected.index, 2);
    app.handle(KeyCode::BackTab);
    assert!(app.on_buttons);
    assert_eq!(app.button_selected, 3);
    app.handle(KeyCode::Esc);
    assert!(!app.on_buttons);
    assert_eq!(app.page, BootPage::Home);
  }

  #[test]
  fn boot_left_right_move_buttons_while_focused_and_no_back_out() {
    let mut app = BootApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.snapshot.bootloader = BootloaderKind::Grub;
    app.page = BootPage::Bootloader;
    app.selected.index = 2;
    app.handle(KeyCode::Tab);
    assert!(app.on_buttons);
    assert_eq!(app.button_selected, 0);
    assert_eq!(app.page, BootPage::Bootloader);
    app.handle(KeyCode::Left);
    assert_eq!(app.button_selected, 3);
    assert_eq!(app.page, BootPage::Bootloader);
    app.handle(KeyCode::Right);
    assert_eq!(app.button_selected, 0);
    app.handle(KeyCode::Down);
    app.handle(KeyCode::Up);
    assert_eq!(app.selected.index, 2);
    app.handle(KeyCode::Tab);
    assert!(!app.on_buttons);
    assert_eq!(app.selected.index, 2);
  }

  #[test]
  fn boot_renders_button_bar_only_on_action_pages() {
    let mut list = BootApp::new(Lang::En, Theme::load(), Capabilities::default());
    list.snapshot.bootloader = BootloaderKind::Grub;
    list.page = BootPage::Bootloader;
    let mut terminal = Terminal::new(TestBackend::new(90, 25)).unwrap();
    terminal.draw(|frame| list.draw(frame)).unwrap();
    let text = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(text.contains("[ Default ]") || text.contains("[ Padrão ]"));
    assert!(text.contains("Actions") || text.contains("Ações"));

    let mut summary = BootApp::new(Lang::En, Theme::load(), Capabilities::default());
    summary.page = BootPage::Summary;
    terminal.draw(|frame| summary.draw(frame)).unwrap();
    let info_text = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(!info_text.contains("[ Default ]") && !info_text.contains("[ Padrão ]"));
  }

  #[test]
  fn boot_start_pending_opens_the_process_window_immediately() {
    let mut app = BootApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.pending = Some(Pending::Action(BootAction::Initramfs));
    app.start_pending();
    assert!(
      app.transaction_open,
      "window must appear when the process starts"
    );
    assert!(app.transaction_live.is_some(), "live sink must be shared");
    assert!(app.transaction_follow);
    assert!(app.action.is_some());
    app.handle(KeyCode::Esc);
    assert!(!app.transaction_open);
  }

  #[test]
  fn boot_transaction_window_scrolls_and_closes() {
    let mut app = BootApp::new(Lang::En, Theme::load(), Capabilities::default());
    let live = LiveProcess::new();
    for _ in 0..40 {
      live.push_line("line");
    }
    app.transaction_live = Some(live.clone());
    app.transaction_open = true;
    assert!(app.transaction_open);
    app.handle(KeyCode::Down);
    assert_eq!(app.transaction_scroll, 1);
    app.handle(KeyCode::Up);
    assert_eq!(app.transaction_scroll, 0);
    app.handle(KeyCode::PageDown);
    assert_eq!(app.transaction_scroll, 10);
    app.handle(KeyCode::PageUp);
    assert_eq!(app.transaction_scroll, 0);
    app.handle(KeyCode::End);
    assert_eq!(
      app.transaction_scroll as usize,
      transaction_bottom_offset(&live.output()) as usize
    );
    app.handle(KeyCode::Esc);
    assert!(!app.transaction_open);
    assert_eq!(app.transaction_scroll, 0);
  }

  #[test]
  fn boot_transaction_window_renders_process_title_and_output() {
    let mut app = BootApp::new(Lang::En, Theme::load(), Capabilities::default());
    let live = LiveProcess::new();
    live.push_line("$ plymouth-set-default-theme -R argvus");
    live.push_line("==> Building image");
    app.transaction_live = Some(live);
    app.transaction_open = true;
    let mut terminal = Terminal::new(TestBackend::new(110, 25)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    let text = terminal
      .backend()
      .buffer()
      .content
      .iter()
      .map(|cell| cell.symbol())
      .collect::<String>();
    assert!(text.contains("Process") || text.contains("Processo"));
    assert!(text.contains("$ plymouth-set-default-theme -R argvus"));
    assert!(text.contains("scr") || text.contains("rol"));
  }

  #[test]
  fn boot_home_dashboard_uses_theme_and_falls_back_to_installed_flag() {
    let mut app = BootApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.snapshot.plymouth.installed = true;
    app.snapshot.plymouth.current_theme = None;
    let rows = app.rows();
    assert!(rows[4].contains("Installed") || rows[4].contains("Instalado"));
    app.snapshot.plymouth.current_theme = Some("argvus".into());
    let rows = app.rows();
    assert!(rows[4].contains("argvus"));
  }
}
