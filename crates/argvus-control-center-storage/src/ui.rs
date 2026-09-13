use crate::{backend, model::*};
use argvus_control_center_core::{
  capabilities::Capabilities,
  config::AppConfig,
  jobs::{JobHandle, JobManager, JobState},
};
use argvus_i18n::{Lang, tr};
use argvus_theme::Theme;
use argvus_tui::{
  components::{StatusKind, StatusMessage, draw_status},
  page::{Selection, list, readonly, shell},
};
use crossterm::event::KeyCode;
use ratatui::{Frame, layout::Rect, text::Line};

pub struct StorageApp {
  pub page: StoragePage,
  pub snapshot: StorageSnapshot,
  selection: Selection,
  job: Option<JobHandle<StorageSnapshot>>,
  jobs: JobManager,
  cap: Capabilities,
  pub lang: Lang,
  pub theme: Theme,
  pub status: Option<StatusMessage>,
}
impl StorageApp {
  pub fn new(lang: Lang, theme: Theme, cap: Capabilities) -> Self {
    let mut s = Self {
      page: StoragePage::Home,
      snapshot: Default::default(),
      selection: Selection::default(),
      job: None,
      jobs: JobManager::default(),
      cap,
      lang,
      theme,
      status: None,
    };
    s.refresh();
    s
  }
  pub fn reload(&mut self) {
    self.refresh();
  }
  pub fn refresh(&mut self) {
    if self.job.is_some() {
      return;
    }
    let cap = self.cap.clone();
    self.job = Some(
      self
        .jobs
        .spawn(move |_| backend::collect(&cap).map_err(|e| e.to_string())),
    );
    self.status = Some(StatusMessage {
      kind: StatusKind::Info,
      text: tr(
        self.lang,
        "Carregando armazenamento...",
        "Loading storage...",
      )
      .into(),
    });
  }
  pub fn poll(&mut self) -> bool {
    let Some(job) = &self.job else {
      return false;
    };
    if let JobState::Finished(result) = job.try_state() {
      self.job = None;
      match result {
        Ok(snapshot) => {
          self.snapshot = snapshot;
          self.status = Some(StatusMessage {
            kind: StatusKind::Success,
            text: tr(self.lang, "Armazenamento atualizado", "Storage refreshed").into(),
          });
        }
        Err(e) => {
          self.status = Some(StatusMessage {
            kind: StatusKind::Error,
            text: e,
          })
        }
      };
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
      StoragePage::Home => 7,
      StoragePage::Disks => self.snapshot.devices.len(),
      StoragePage::Partitions => self.partitions().len(),
      StoragePage::Filesystems => self.snapshot.filesystems.len(),
      StoragePage::Mounts => self.snapshot.mounts.len(),
      StoragePage::Smart => {
        if self.snapshot.smart.is_empty() {
          1
        } else {
          self.snapshot.smart.len()
        }
      }
      StoragePage::Usage => {
        self.snapshot.filesystems.len() + usize::from(!self.snapshot.swap.is_empty())
      }
      _ => 1,
    }
  }
  fn partitions(&self) -> Vec<&StorageDevice> {
    self
      .snapshot
      .devices
      .iter()
      .flat_map(|d| d.children.iter())
      .collect()
  }
  pub fn handle(&mut self, key: KeyCode) -> bool {
    if matches!(key, KeyCode::Esc | KeyCode::Left) {
      if self.page == StoragePage::Home {
        return true;
      }
      self.page = match self.page {
        StoragePage::DiskDetails(_) => StoragePage::Disks,
        StoragePage::PartitionDetails(_) => StoragePage::Partitions,
        StoragePage::FilesystemDetails(_) => StoragePage::Filesystems,
        StoragePage::MountDetails(_) => StoragePage::Mounts,
        StoragePage::SmartDetails(_) => StoragePage::Smart,
        _ => StoragePage::Home,
      };
      self.selection.index = 0;
      return false;
    }
    if self.busy() {
      return false;
    }
    if key == KeyCode::Char('r') {
      self.refresh();
      return false;
    }
    self.selection.handle(key, self.len(), 8);
    if matches!(key, KeyCode::Enter | KeyCode::Right) {
      self.page = match self.page {
        StoragePage::Home => [
          StoragePage::Summary,
          StoragePage::Disks,
          StoragePage::Partitions,
          StoragePage::Filesystems,
          StoragePage::Mounts,
          StoragePage::Smart,
          StoragePage::Usage,
        ][self.selection.index.min(6)],
        StoragePage::Disks => StoragePage::DiskDetails(self.selection.index),
        StoragePage::Partitions => StoragePage::PartitionDetails(self.selection.index),
        StoragePage::Filesystems => StoragePage::FilesystemDetails(self.selection.index),
        StoragePage::Mounts => StoragePage::MountDetails(self.selection.index),
        StoragePage::Smart => StoragePage::SmartDetails(self.selection.index),
        _ => self.page,
      };
      self.selection.index = 0;
    }
    false
  }
  pub fn breadcrumb(&self) -> String {
    let root = tr(self.lang, "Armazenamento", "Storage");
    if self.page == StoragePage::Home {
      root.into()
    } else {
      format!("{root} > {}", self.label())
    }
  }
  fn label(&self) -> &'static str {
    match self.page {
      StoragePage::Summary => tr(self.lang, "Resumo", "Summary"),
      StoragePage::Disks | StoragePage::DiskDetails(_) => tr(self.lang, "Discos", "Disks"),
      StoragePage::Partitions | StoragePage::PartitionDetails(_) => {
        tr(self.lang, "Partições", "Partitions")
      }
      StoragePage::Filesystems | StoragePage::FilesystemDetails(_) => {
        tr(self.lang, "Sistemas de arquivos", "Filesystems")
      }
      StoragePage::Mounts | StoragePage::MountDetails(_) => {
        tr(self.lang, "Pontos de montagem", "Mount points")
      }
      StoragePage::Smart | StoragePage::SmartDetails(_) => "SMART",
      StoragePage::Usage => tr(self.lang, "Uso de disco", "Disk usage"),
      _ => tr(self.lang, "Resumo", "Summary"),
    }
  }
  fn rows(&self) -> Vec<String> {
    match self.page {
      StoragePage::Home => self.home_rows(),
      StoragePage::Summary => self.summary_rows(),
      StoragePage::Disks => self
        .snapshot
        .devices
        .iter()
        .map(|d| disk_row(self.lang, d))
        .collect(),
      StoragePage::Partitions => self.partitions().iter().map(|p| partition_row(p)).collect(),
      StoragePage::Filesystems => self
        .snapshot
        .filesystems
        .iter()
        .map(filesystem_row)
        .collect(),
      StoragePage::Mounts => self
        .snapshot
        .mounts
        .iter()
        .map(|m| {
          format!(
            "{:<18} {:<9}  {}{}",
            m.target,
            m.fstype,
            m.source,
            if m.readonly {
              format!("  [{}]", tr(self.lang, "ro", "ro"))
            } else {
              String::new()
            }
          )
        })
        .collect(),
      StoragePage::Smart => self.smart_rows(),
      StoragePage::Usage => self.usage_rows(),
      StoragePage::DiskDetails(index) => self.disk_detail(index),
      StoragePage::PartitionDetails(index) => self.partition_detail(index),
      StoragePage::FilesystemDetails(index) => self.filesystem_detail(index),
      StoragePage::MountDetails(index) => self.mount_detail(index),
      StoragePage::SmartDetails(index) => self.smart_detail(index),
    }
  }
  fn home_rows(&self) -> Vec<String> {
    let disks = physical_devices(&self.snapshot.devices);
    let total: u64 = disks.iter().map(|d| d.size_bytes.unwrap_or(0)).sum();
    let root_pct = self
      .snapshot
      .filesystems
      .iter()
      .find(|f| f.mountpoint.as_deref() == Some("/"))
      .and_then(|f| f.total_bytes.zip(f.available_bytes))
      .map(|(total, available)| percentage(total, available))
      .unwrap_or(0);
    let smart = self.smart_rows().first().cloned().unwrap_or_default();
    let summary = tr(self.lang, "Resumo", "Summary");
    let disks_label = tr(self.lang, "Discos", "Disks");
    let partitions_label = tr(self.lang, "Partições", "Partitions");
    let filesystems_label = tr(self.lang, "Sistemas de arquivos", "Filesystems");
    let mounts_label = tr(self.lang, "Pontos de montagem", "Mount points");
    let usage_label = tr(self.lang, "Uso de disco", "Disk usage");
    vec![
      format!(
        "{} {}  ·  {} {} · {}",
        AppConfig::icon("📊"),
        summary,
        disks.len(),
        tr(self.lang, "discos", "disks"),
        human_bytes(total)
      ),
      format!(
        "{} {}  ·  {} · {}",
        AppConfig::icon("💽"),
        disks_label,
        disks.len(),
        human_bytes(total)
      ),
      format!(
        "{} {}  ·  {}",
        AppConfig::icon("🔖"),
        partitions_label,
        self.partitions().len()
      ),
      format!(
        "{} {}  ·  {} · / {}%",
        AppConfig::icon("📁"),
        filesystems_label,
        self.snapshot.filesystems.len(),
        root_pct
      ),
      format!(
        "{} {}  ·  {}",
        AppConfig::icon("📌"),
        mounts_label,
        self.snapshot.mounts.len()
      ),
      format!(
        "{} {}  ·  {}",
        AppConfig::icon("🛡️"),
        "SMART",
        smart.trim_start()
      ),
      format!(
        "{} {}  ·  / {}%",
        AppConfig::icon("📈"),
        usage_label,
        root_pct
      ),
    ]
  }
  fn summary_rows(&self) -> Vec<String> {
    let disks = physical_devices(&self.snapshot.devices);
    let total: u64 = disks.iter().map(|d| d.size_bytes.unwrap_or(0)).sum();
    let smart_available = if self.snapshot.smart.is_empty() {
      if self.snapshot.smart_available {
        tr(self.lang, "Nenhum dado coletado", "No data collected").to_string()
      } else {
        tr(self.lang, "Indisponível", "Unavailable").to_string()
      }
    } else {
      tr(self.lang, "Disponível", "Available").to_string()
    };
    let swap = self
      .snapshot
      .swap
      .first()
      .map(|s| {
        format!(
          "{} · {} ({} {})",
          s.source,
          human_bytes(s.total_bytes),
          human_bytes(s.used_bytes),
          tr(self.lang, "usado", "used")
        )
      })
      .unwrap_or_else(|| "—".into());
    let mut rows = vec![
      format!(
        " {} {}",
        AppConfig::icon("📊"),
        tr(self.lang, "RESUMO DO ARMAZENAMENTO", "STORAGE SUMMARY")
      ),
      format!(
        "   {:<16} {}",
        tr(self.lang, "Discos:", "Disks:"),
        disks.len()
      ),
      format!(
        "   {:<16} {}",
        tr(self.lang, "Tamanho total:", "Total size:"),
        human_bytes(total)
      ),
      format!(
        "   {:<16} {}",
        tr(self.lang, "Partições:", "Partitions:"),
        self.partitions().len()
      ),
      format!(
        "   {:<16} {}",
        tr(self.lang, "Sistemas de arquivos:", "Filesystems:"),
        self.snapshot.filesystems.len()
      ),
      format!(
        "   {:<16} {}",
        tr(self.lang, "Pontos de montagem:", "Mount points:"),
        self.snapshot.mounts.len()
      ),
      format!("   {:<16} {}", tr(self.lang, "Swap:", "Swap:"), swap),
      format!(
        "   {:<16} {}",
        tr(self.lang, "SMART:", "SMART:"),
        smart_available
      ),
      "".into(),
      format!(
        " {} {}",
        AppConfig::icon("💾"),
        tr(
          self.lang,
          "USO POR SISTEMA DE ARQUIVOS",
          "USAGE BY FILESYSTEM"
        )
      ),
    ];
    if self.snapshot.filesystems.is_empty() {
      rows.push(format!(
        "   {}",
        tr(
          self.lang,
          "Nenhum sistema de arquivos encontrado",
          "No filesystems found"
        )
      ));
    }
    for f in &self.snapshot.filesystems {
      rows.push(filesystem_row(f));
    }
    rows
  }
  fn smart_rows(&self) -> Vec<String> {
    if self.snapshot.smart.is_empty() {
      return vec![if self.snapshot.smart_available {
        tr(
          self.lang,
          "Nenhum dado SMART disponível",
          "No SMART data available",
        )
        .into()
      } else {
        tr(
          self.lang,
          "smartctl não está instalado",
          "smartctl is not installed",
        )
        .into()
      }];
    }
    self
      .snapshot
      .smart
      .iter()
      .map(|s| {
        let temp = s
          .temperature_c
          .map(|t| format!(" · {}°C", t))
          .unwrap_or_default();
        let level = match s.health {
          SmartHealth::Failed => {
            format!("  [{}]", tr(self.lang, "Falhou", "Failed"))
          }
          _ => String::new(),
        };
        format!(
          "{}  ·  {}{}{}",
          s.device,
          smart_health_label(self.lang, s.health),
          level,
          temp
        )
      })
      .collect()
  }
  fn usage_rows(&self) -> Vec<String> {
    let mut rows = self
      .snapshot
      .filesystems
      .iter()
      .map(|f| usage_row(self.lang, f))
      .collect::<Vec<_>>();
    for swap in &self.snapshot.swap {
      rows.push(format!(
        "{} {} {}  ·  {} / {}",
        AppConfig::icon("🔄"),
        tr(self.lang, "Swap", "Swap"),
        swap.source,
        human_bytes(swap.used_bytes),
        human_bytes(swap.total_bytes)
      ));
    }
    rows
  }
  fn disk_detail(&self, index: usize) -> Vec<String> {
    let Some(d) = self.snapshot.devices.get(index) else {
      return vec![tr(self.lang, "Disco não encontrado", "Disk not found").into()];
    };
    vec![
      format!(
        " {} {}",
        AppConfig::icon("💽"),
        tr(self.lang, "DISCO", "DISK")
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Dispositivo:", "Device:"),
        d.name
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Modelo:", "Model:"),
        d.model.as_deref().unwrap_or("—")
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Fabricante:", "Vendor:"),
        d.vendor.as_deref().unwrap_or("—")
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Serial:", "Serial:"),
        d.serial.as_deref().unwrap_or("—")
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Tamanho:", "Size:"),
        human_bytes(d.size_bytes.unwrap_or(0))
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Transporte:", "Transport:"),
        d.transport.as_deref().unwrap_or("—")
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Rotacional:", "Rotational:"),
        bool_label(self.lang, d.rotational)
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Removível:", "Removable:"),
        bool_label(self.lang, d.removable)
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "UUID:", "UUID:"),
        d.uuid.as_deref().unwrap_or("—")
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Partições:", "Partitions:"),
        d.children.len()
      ),
    ]
  }
  fn partition_detail(&self, index: usize) -> Vec<String> {
    let Some(p) = self.partitions().get(index).copied() else {
      return vec![tr(self.lang, "Partição não encontrada", "Partition not found").into()];
    };
    let parent = p.parent.as_deref().unwrap_or("—");
    vec![
      format!(
        " {} {}",
        AppConfig::icon("🔖"),
        tr(self.lang, "PARTIÇÃO", "PARTITION")
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Dispositivo:", "Device:"),
        p.name
      ),
      format!("   {:<14} {}", tr(self.lang, "Disco:", "Disk:"), parent),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Sistema:", "Filesystem:"),
        p.fstype.as_deref().unwrap_or("—")
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Tamanho:", "Size:"),
        human_bytes(p.size_bytes.unwrap_or(0))
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Montado em:", "Mounted at:"),
        p.mountpoint.as_deref().unwrap_or("—")
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Rótulo:", "Label:"),
        p.label.as_deref().unwrap_or("—")
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "UUID:", "UUID:"),
        p.uuid.as_deref().unwrap_or("—")
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Somente leitura:", "Read-only:"),
        bool_label(self.lang, p.readonly)
      ),
    ]
  }
  fn filesystem_detail(&self, index: usize) -> Vec<String> {
    let Some(f) = self.snapshot.filesystems.get(index) else {
      return vec![
        tr(
          self.lang,
          "Sistema de arquivos não encontrado",
          "Filesystem not found",
        )
        .into(),
      ];
    };
    let (used, total, available, pct) = match (f.total_bytes, f.used_bytes) {
      (Some(total), Some(used)) => {
        let available = f.available_bytes.unwrap_or(total.saturating_sub(used));
        (
          Some(used),
          Some(total),
          Some(available),
          percentage(total, available),
        )
      }
      _ => (None, None, None, 0),
    };
    let level = usage_level(used.unwrap_or(0), available.unwrap_or(0));
    let level_label = match level {
      UsageLevel::Ok => tr(self.lang, "OK", "OK"),
      UsageLevel::Warning => tr(self.lang, "Atenção", "Warning"),
      UsageLevel::Critical => tr(self.lang, "Crítico", "Critical"),
    };
    let mut rows = vec![
      format!(
        " {} {}",
        AppConfig::icon("📁"),
        tr(self.lang, "SISTEMA DE ARQUIVOS", "FILESYSTEM")
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Origem:", "Source:"),
        f.source
      ),
      format!("   {:<14} {}", tr(self.lang, "Tipo:", "Type:"), f.fstype),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Montado em:", "Mounted at:"),
        f.mountpoint.as_deref().unwrap_or("—")
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Somente leitura:", "Read-only:"),
        yes_no(self.lang, f.readonly)
      ),
      "".into(),
    ];
    if let (Some(total), Some(used), Some(available)) = (total, used, available) {
      rows.push(format!(
        "   {:<14} {}",
        tr(self.lang, "Tamanho:", "Size:"),
        human_bytes(total)
      ));
      rows.push(format!(
        "   {:<14} {}",
        tr(self.lang, "Usado:", "Used:"),
        human_bytes(used)
      ));
      rows.push(format!(
        "   {:<14} {}",
        tr(self.lang, "Disponível:", "Available:"),
        human_bytes(available)
      ));
      rows.push(format!(
        "   {:<14} {} · {}  {}",
        tr(self.lang, "Uso:", "Usage:"),
        pct,
        "%",
        level_label
      ));
      rows.push(format!("   {}", usage_bar(pct, 20)));
    }
    rows
  }
  fn mount_detail(&self, index: usize) -> Vec<String> {
    let Some(m) = self.snapshot.mounts.get(index) else {
      return vec![
        tr(
          self.lang,
          "Ponto de montagem não encontrado",
          "Mount point not found",
        )
        .into(),
      ];
    };
    vec![
      format!(
        " {} {}",
        AppConfig::icon("📌"),
        tr(self.lang, "PONTO DE MONTAGEM", "MOUNT POINT")
      ),
      format!("   {:<14} {}", tr(self.lang, "Alvo:", "Target:"), m.target),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Origem:", "Source:"),
        m.source
      ),
      format!("   {:<14} {}", tr(self.lang, "Tipo:", "Type:"), m.fstype),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Opções:", "Options:"),
        m.options.join(" ")
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Somente leitura:", "Read-only:"),
        yes_no(self.lang, m.readonly)
      ),
    ]
  }
  fn smart_detail(&self, index: usize) -> Vec<String> {
    let Some(s) = self.snapshot.smart.get(index) else {
      return vec![tr(self.lang, "SMART indisponível", "SMART unavailable").into()];
    };
    let health = smart_health_label(self.lang, s.health);
    let temp = s
      .temperature_c
      .map(|t| format!("{t}°C"))
      .unwrap_or_else(|| "—".into());
    let mut rows = vec![
      format!(
        " {} {}",
        AppConfig::icon("🛡️"),
        tr(self.lang, "SMART", "SMART")
      ),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Dispositivo:", "Device:"),
        s.device
      ),
      format!("   {:<14} {}", tr(self.lang, "Saúde:", "Health:"), health),
      format!(
        "   {:<14} {}",
        tr(self.lang, "Temperatura:", "Temperature:"),
        temp
      ),
    ];
    if let Some(hours) = s.power_on_hours {
      rows.push(format!(
        "   {:<14} {} h",
        tr(self.lang, "Tempo ligado:", "Power on:"),
        hours
      ));
    }
    if let Some(cycles) = s.power_cycles {
      rows.push(format!(
        "   {:<14} {}",
        tr(self.lang, "Ciclos:", "Cycles:"),
        cycles
      ));
    }
    if let Some(percent) = s.percentage_used {
      rows.push(format!(
        "   {:<14} {}%",
        tr(self.lang, "Uso:", "Used:"),
        percent
      ));
    }
    if let Some(warning) = s.critical_warning {
      rows.push(format!(
        "   {:<14} {}",
        tr(self.lang, "Aviso crítico:", "Critical warning:"),
        warning
      ));
    }
    rows
  }
  pub fn draw(&mut self, frame: &mut Frame) {
    let body = shell(
      frame,
      frame.area(),
      &self.theme,
      &self.breadcrumb(),
      if self.len() > 0 {
        tr(
          self.lang,
          "↑/↓ Rolar   r Atualizar   ←/Esc Voltar   ? Ajuda",
          "↑/↓ Scroll   r Refresh   ←/Esc Back   ? Help",
        )
      } else {
        tr(
          self.lang,
          "r Atualizar   ←/Esc Voltar   ? Ajuda",
          "r Refresh   ←/Esc Back   ? Help",
        )
      },
    );
    let rows = self.rows();
    if matches!(
      self.page,
      StoragePage::Summary
        | StoragePage::DiskDetails(_)
        | StoragePage::PartitionDetails(_)
        | StoragePage::FilesystemDetails(_)
        | StoragePage::MountDetails(_)
        | StoragePage::SmartDetails(_)
    ) {
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
        self.selection.index.min(rows.len().saturating_sub(1)),
      );
    }
    if let Some(status) = &self.status {
      draw_status(
        frame,
        Rect::new(body.x, body.bottom().saturating_sub(1), body.width, 1),
        &self.theme,
        status,
      );
    }
  }
}

fn physical_devices(devices: &[StorageDevice]) -> Vec<&StorageDevice> {
  devices
    .iter()
    .filter(|d| d.kind == "disk" && !d.name.starts_with("zram") && !d.name.starts_with("loop"))
    .collect()
}
fn percentage(total: u64, available: u64) -> u64 {
  total
    .saturating_sub(available)
    .checked_mul(100)
    .and_then(|used| used.checked_div(total))
    .unwrap_or(0)
}
fn disk_icon(device: &StorageDevice) -> &'static str {
  if device.name.starts_with("zram") || device.name.starts_with("loop") {
    AppConfig::icon("🌀")
  } else {
    AppConfig::icon("💽")
  }
}
fn partition_icon(partition: &StorageDevice) -> &'static str {
  match partition.fstype.as_deref() {
    Some("crypto_LUKS") | Some("crypt") => AppConfig::icon("🔐"),
    Some("swap") => AppConfig::icon("🔄"),
    _ => AppConfig::icon("🔖"),
  }
}
fn disk_row(lang: Lang, device: &StorageDevice) -> String {
  let label = if device.name.starts_with("zram") || device.name.starts_with("loop") {
    device.kind.clone()
  } else {
    device
      .model
      .clone()
      .or_else(|| device.transport.clone())
      .unwrap_or(device.kind.clone())
  };
  let parts = if device.children.is_empty() {
    String::new()
  } else {
    format!(
      "  ·  {} {}",
      device.children.len(),
      tr(lang, "partições", "partitions")
    )
  };
  format!(
    "{} {:<8} {:>9}  ·  {}{}",
    disk_icon(device),
    device.name,
    human_bytes(device.size_bytes.unwrap_or(0)),
    label,
    parts
  )
}
fn partition_row(partition: &StorageDevice) -> String {
  format!(
    "{} {:<12} {:>9}  {:<11}  {}",
    partition_icon(partition),
    partition.name,
    human_bytes(partition.size_bytes.unwrap_or(0)),
    partition.fstype.as_deref().unwrap_or("—"),
    partition.mountpoint.as_deref().unwrap_or("—")
  )
}
fn filesystem_row(filesystem: &Filesystem) -> String {
  let mountpoint = filesystem
    .mountpoint
    .as_deref()
    .unwrap_or(filesystem.source.as_str());
  if filesystem.used_bytes.is_none() || filesystem.total_bytes.is_none() {
    return format!("{:<18} {:<9}", mountpoint, filesystem.fstype);
  }
  let total = filesystem.total_bytes.unwrap_or(0);
  let used = filesystem.used_bytes.unwrap_or(0);
  let available = filesystem
    .available_bytes
    .unwrap_or(total.saturating_sub(used));
  let pct = percentage(total, available);
  format!(
    "{:<18} {:<9} {:>9} / {:<9} {:>3}%",
    mountpoint,
    filesystem.fstype,
    human_bytes(used),
    human_bytes(total),
    pct
  )
}
fn usage_row(lang: Lang, filesystem: &Filesystem) -> String {
  let mountpoint = filesystem
    .mountpoint
    .as_deref()
    .unwrap_or(filesystem.source.as_str());
  if let (Some(total), Some(available)) = (filesystem.total_bytes, filesystem.available_bytes)
    && total > 0
  {
    let pct = percentage(total, available);
    return format!(
      "{:<18} {:<9} {:>3}%  {}{}",
      mountpoint,
      filesystem.fstype,
      pct,
      usage_bar(pct, 10),
      if pct >= 80 {
        format!("  [{}]", usage_label(lang, usage_level(total, available)))
      } else {
        String::new()
      }
    );
  }
  format!("{:<18} {:<9}", mountpoint, filesystem.fstype)
}
fn usage_bar(percent: u64, width: usize) -> String {
  let filled = (percent as usize * width / 100).min(width);
  let mut bar = String::with_capacity(width);
  for index in 0..width {
    bar.push(if index < filled { '█' } else { '░' });
  }
  bar
}
fn usage_label(lang: Lang, level: UsageLevel) -> String {
  match level {
    UsageLevel::Ok => tr(lang, "OK", "OK").into(),
    UsageLevel::Warning => tr(lang, "Atenção", "Warning").into(),
    UsageLevel::Critical => tr(lang, "Crítico", "Critical").into(),
  }
}
fn smart_health_label(lang: Lang, health: SmartHealth) -> String {
  match health {
    SmartHealth::Passed => tr(lang, "Aprovado", "Passed").into(),
    SmartHealth::Warning => tr(lang, "Atenção", "Warning").into(),
    SmartHealth::Failed => tr(lang, "Falhou", "Failed").into(),
    SmartHealth::Unsupported => tr(lang, "Indisponível", "Unavailable").into(),
    SmartHealth::Unknown => tr(lang, "Desconhecido", "Unknown").into(),
  }
}
fn bool_label(lang: Lang, value: Option<bool>) -> String {
  match value {
    Some(value) => yes_no(lang, value),
    None => "—".into(),
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

#[cfg(test)]
mod tests {
  use super::*;
  use argvus_i18n::Lang;
  use argvus_theme::Theme;

  fn test_app(snapshot: StorageSnapshot) -> StorageApp {
    let mut app = StorageApp::new(Lang::En, Theme::load(), Capabilities::default());
    app.snapshot = snapshot;
    app.job = None;
    app
  }

  fn sample_snapshot() -> StorageSnapshot {
    StorageSnapshot {
      devices: vec![
        StorageDevice {
          name: "sda".into(),
          kind: "disk".into(),
          model: Some("KINGSTON SA400S37240G".into()),
          size_bytes: Some(240_057_409_536),
          children: vec![StorageDevice {
            name: "sda1".into(),
            kind: "part".into(),
            fstype: Some("vfat".into()),
            mountpoint: Some("/boot".into()),
            parent: Some("sda".into()),
            size_bytes: Some(536_870_912),
            ..Default::default()
          }],
          ..Default::default()
        },
        StorageDevice {
          name: "zram0".into(),
          kind: "disk".into(),
          size_bytes: Some(4_294_967_296),
          ..Default::default()
        },
      ],
      filesystems: vec![Filesystem {
        source: "/dev/sda2".into(),
        fstype: "ext4".into(),
        mountpoint: Some("/".into()),
        total_bytes: Some(100_000_000_000),
        used_bytes: Some(60_000_000_000),
        available_bytes: Some(40_000_000_000),
        readonly: false,
        ..Default::default()
      }],
      mounts: vec![
        Mount {
          target: "/".into(),
          source: "/dev/sda2".into(),
          fstype: "ext4".into(),
          options: vec!["rw".into()],
          readonly: false,
        },
        Mount {
          target: "/proc".into(),
          source: "proc".into(),
          fstype: "proc".into(),
          options: vec!["rw".into()],
          readonly: false,
        },
      ],
      swap: vec![SwapDevice {
        source: "/dev/zram0".into(),
        kind: "zram".into(),
        total_bytes: 4_294_967_296,
        used_bytes: 0,
      }],
      smart_available: true,
      smart: vec![],
    }
  }

  #[test]
  fn storage_home_rows_act_as_a_status_dashboard() {
    let app = test_app(sample_snapshot());
    let rows = app.home_rows();
    assert_eq!(rows.len(), 7);
    assert!(rows[0].contains("1 disks"), "{}", rows[0]);
    assert!(rows[0].contains("223.6 GiB"), "{}", rows[0]);
    assert!(rows[2].contains("1"));
    assert!(rows[3].contains("%"));
    assert!(rows[4].contains("2"));
    assert!(rows[5].contains("No SMART data"));
  }

  #[test]
  fn storage_summary_page_shows_counts_and_usage() {
    let mut app = test_app(sample_snapshot());
    app.page = StoragePage::Summary;
    let rows = app.rows();
    assert!(rows[0].contains("SUMMARY"), "{}", rows[0]);
    assert!(rows.iter().any(|r| r.contains("Disks:")));
    assert!(rows.iter().any(|r| r.contains("/")));
  }

  #[test]
  fn storage_smart_page_falls_back_when_no_data() {
    let mut app = test_app(sample_snapshot());
    app.page = StoragePage::Smart;
    assert_eq!(app.len(), 1);
    let rows = app.rows();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].contains("No SMART data"));
    assert!(app.len() >= 1);
  }

  #[test]
  fn storage_usage_rows_append_swap() {
    let mut app = test_app(sample_snapshot());
    app.page = StoragePage::Usage;
    assert_eq!(app.len(), 2);
    let rows = app.rows();
    assert_eq!(rows.len(), 2);
    assert!(rows[1].contains("Swap"));
    assert!(rows[0].contains("%"));
  }

  #[test]
  fn storage_detail_pages_render_section_headers_and_aligned_rows() {
    let mut app = test_app(sample_snapshot());
    app.page = StoragePage::DiskDetails(0);
    let rows = app.rows();
    assert!(rows[0].contains("DISK"));
    assert!(rows.iter().any(|r| r.contains("KINGSTON")));
    assert!(
      rows
        .iter()
        .any(|r| r.contains("223.6 GiB") || r.contains("GiB"))
    );
    assert!(rows.len() > 5);

    app.page = StoragePage::FilesystemDetails(0);
    let rows = app.rows();
    assert!(rows[0].contains("FILESYSTEM"));
    assert!(rows.iter().any(|r| r.contains("60 ") && r.contains('%')));
    assert!(rows.iter().any(|r| r.contains('█')));
  }

  #[test]
  fn storage_human_bytes_and_percentage_helpers() {
    assert_eq!(human_bytes(0), "0 B");
    assert_eq!(human_bytes(1024), "1 KiB");
    assert_eq!(human_bytes(240_057_409_536), "223.6 GiB");
    assert_eq!(percentage(100, 40), 60);
    assert_eq!(percentage(0, 0), 0);
    assert_eq!(usage_bar(50, 10), "█████░░░░░");
  }
}
