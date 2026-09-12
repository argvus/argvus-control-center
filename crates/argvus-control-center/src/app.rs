use crate::config_app::ConfigApp;
#[cfg(feature = "about")]
use argvus_control_center_about::{App as AboutState, Tab};
#[cfg(feature = "audio")]
use argvus_control_center_audio::{AudioApp, AudioPage};
#[cfg(feature = "bluetooth")]
use argvus_control_center_bluetooth::{BluetoothApp, BluetoothPage};
#[cfg(feature = "boot")]
use argvus_control_center_boot::{BootApp, BootPage};
use argvus_control_center_core::{
  capabilities::Capabilities,
  jobs::JobManager,
  search::{SearchEntry, SearchRegistry},
};
#[cfg(feature = "diagnostics")]
use argvus_control_center_diagnostics::{DiagnosticPage, DiagnosticsApp};
#[cfg(feature = "displays")]
use argvus_control_center_displays::{DisplayPage, DisplaysApp};
#[cfg(feature = "hardware")]
use argvus_control_center_hardware::{HardwareApp, HardwarePage};
#[cfg(feature = "network")]
use argvus_control_center_network::{NetworkApp, NetworkPage};
#[cfg(feature = "packages")]
use argvus_control_center_packages::{PackagesApp, PackagesPage};
#[cfg(feature = "power")]
use argvus_control_center_power::PowerApp;
#[cfg(feature = "services")]
use argvus_control_center_services::{ServicePage, ServicesApp};
#[cfg(feature = "session")]
use argvus_control_center_session::{SessionApp, SessionPage};
use argvus_control_center_settings::{App as SettingsState, Page};
#[cfg(feature = "storage")]
use argvus_control_center_storage::{StorageApp, StoragePage};
use argvus_i18n::{Lang, tr};
use argvus_theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
  Home,
  Settings,
  Config,
  #[cfg(feature = "about")]
  About,
  #[cfg(feature = "hardware")]
  Hardware,
  #[cfg(feature = "services")]
  Services,
  #[cfg(feature = "network")]
  Network,
  #[cfg(feature = "audio")]
  Audio,
  #[cfg(feature = "bluetooth")]
  Bluetooth,
  #[cfg(feature = "boot")]
  Boot,
  #[cfg(feature = "packages")]
  Packages,
  #[cfg(feature = "storage")]
  Storage,
  #[cfg(feature = "diagnostics")]
  Diagnostics,
  #[cfg(feature = "power")]
  Power,
  #[cfg(feature = "session")]
  Session,
  #[cfg(feature = "displays")]
  Displays,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitialRoute {
  Home,
  Config,
  Settings(Page),
  #[cfg(feature = "apps")]
  Apps,
  #[cfg(feature = "fonts")]
  Fonts,
  #[cfg(feature = "locale")]
  LocaleRegion,
  #[cfg(feature = "language")]
  Language,
  #[cfg(feature = "system")]
  System,
  #[cfg(feature = "about")]
  About(Tab),
  #[cfg(feature = "hardware")]
  Hardware(HardwarePage),
  #[cfg(feature = "services")]
  Services(ServicePage),
  #[cfg(feature = "network")]
  Network(NetworkPage),
  #[cfg(feature = "audio")]
  Audio(AudioPage),
  #[cfg(feature = "bluetooth")]
  Bluetooth(BluetoothPage),
  #[cfg(feature = "boot")]
  Boot(BootPage),
  #[cfg(feature = "packages")]
  Packages(PackagesPage),
  #[cfg(feature = "storage")]
  Storage(StoragePage),
  #[cfg(feature = "diagnostics")]
  Diagnostics(DiagnosticPage),
  #[cfg(feature = "power")]
  Power,
  #[cfg(feature = "session")]
  Session(SessionPage),
  #[cfg(feature = "displays")]
  Displays(DisplayPage),
}

pub struct App {
  pub lang: Lang,
  pub theme: Theme,
  pub route: Route,
  pub home_selected: usize,
  pub settings: SettingsState,
  pub config: ConfigApp,
  #[cfg(feature = "about")]
  pub about: AboutState,
  pub help: bool,
  pub quit: bool,
  pub width: u16,
  pub height: u16,
  #[cfg(feature = "hardware")]
  pub hardware: HardwareApp,
  #[cfg(feature = "services")]
  pub services: ServicesApp,
  #[cfg(feature = "network")]
  pub network: NetworkApp,
  #[cfg(feature = "audio")]
  pub audio: AudioApp,
  #[cfg(feature = "bluetooth")]
  pub bluetooth: BluetoothApp,
  #[cfg(feature = "boot")]
  pub boot: BootApp,
  #[cfg(feature = "packages")]
  pub packages: PackagesApp,
  #[cfg(feature = "storage")]
  pub storage: StorageApp,
  #[cfg(feature = "diagnostics")]
  pub diagnostics: DiagnosticsApp,
  #[cfg(feature = "power")]
  pub power: PowerApp,
  #[cfg(feature = "session")]
  pub session: SessionApp,
  #[cfg(feature = "displays")]
  pub displays: DisplaysApp,
  pub capabilities: Capabilities,
  pub search_registry: SearchRegistry,
  pub jobs: JobManager,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeRow {
  Header(&'static str),
  Item { label: &'static str, action: usize },
}

impl App {
  pub fn new(initial: InitialRoute) -> Self {
    let lang = Lang::detect();
    let theme = Theme::load();
    let route = match initial {
      InitialRoute::Home => Route::Home,
      InitialRoute::Config => Route::Config,
      InitialRoute::Settings(_) => Route::Settings,
      #[cfg(feature = "apps")]
      InitialRoute::Apps => Route::Settings,
      #[cfg(feature = "fonts")]
      InitialRoute::Fonts => Route::Settings,
      #[cfg(feature = "locale")]
      InitialRoute::LocaleRegion => Route::Settings,
      #[cfg(feature = "language")]
      InitialRoute::Language => Route::Settings,
      #[cfg(feature = "system")]
      InitialRoute::System => Route::Settings,
      #[cfg(feature = "about")]
      InitialRoute::About(_) => Route::About,
      #[cfg(feature = "hardware")]
      InitialRoute::Hardware(_) => Route::Hardware,
      #[cfg(feature = "services")]
      InitialRoute::Services(_) => Route::Services,
      #[cfg(feature = "network")]
      InitialRoute::Network(_) => Route::Network,
      #[cfg(feature = "audio")]
      InitialRoute::Audio(_) => Route::Audio,
      #[cfg(feature = "bluetooth")]
      InitialRoute::Bluetooth(_) => Route::Bluetooth,
      #[cfg(feature = "boot")]
      InitialRoute::Boot(_) => Route::Boot,
      #[cfg(feature = "packages")]
      InitialRoute::Packages(_) => Route::Packages,
      #[cfg(feature = "storage")]
      InitialRoute::Storage(_) => Route::Storage,
      #[cfg(feature = "diagnostics")]
      InitialRoute::Diagnostics(_) => Route::Diagnostics,
      #[cfg(feature = "power")]
      InitialRoute::Power => Route::Power,
      #[cfg(feature = "session")]
      InitialRoute::Session(_) => Route::Session,
      #[cfg(feature = "displays")]
      InitialRoute::Displays(_) => Route::Displays,
    };
    let page = match initial {
      InitialRoute::Settings(page) => page,
      #[cfg(feature = "apps")]
      InitialRoute::Apps => Page::DefaultApps,
      #[cfg(feature = "fonts")]
      InitialRoute::Fonts => Page::Fonts,
      #[cfg(feature = "locale")]
      InitialRoute::LocaleRegion => Page::LocaleRegion,
      #[cfg(feature = "language")]
      InitialRoute::Language => Page::Language,
      #[cfg(feature = "system")]
      InitialRoute::System => Page::System,
      _ => Page::Main,
    };
    #[cfg(feature = "about")]
    let tab = match initial {
      InitialRoute::About(tab) => tab,
      _ => Tab::System,
    };
    let capabilities = Capabilities::detect();
    #[cfg(feature = "hardware")]
    let mut hardware = HardwareApp::new(lang, theme.clone(), capabilities.clone());
    #[cfg(feature = "services")]
    let mut services = ServicesApp::new(lang, theme.clone());
    #[cfg(feature = "network")]
    let network = NetworkApp::new(lang, theme.clone(), capabilities.clone());
    #[cfg(feature = "audio")]
    let audio = AudioApp::new(lang, theme.clone(), capabilities.clone());
    #[cfg(feature = "bluetooth")]
    let bluetooth = BluetoothApp::new(lang, theme.clone(), capabilities.clone());
    #[cfg(feature = "boot")]
    let boot = BootApp::new(lang, theme.clone(), capabilities.clone());
    #[cfg(feature = "packages")]
    let packages = PackagesApp::new(lang, theme.clone(), capabilities.clone());
    #[cfg(feature = "storage")]
    let storage = StorageApp::new(lang, theme.clone(), capabilities.clone());
    #[cfg(feature = "diagnostics")]
    let diagnostics = DiagnosticsApp::new(lang, theme.clone(), capabilities.clone());
    #[cfg(feature = "power")]
    let power = PowerApp::new(lang, theme.clone());
    #[cfg(feature = "session")]
    let mut session = SessionApp::new(lang, theme.clone());
    #[cfg(feature = "displays")]
    let mut displays = DisplaysApp::new(lang, theme.clone());
    #[cfg(feature = "hardware")]
    if let InitialRoute::Hardware(page) = initial {
      hardware.page = page;
    }
    #[cfg(feature = "services")]
    if let InitialRoute::Services(page) = initial {
      services.page = page;
      services.reload();
    }
    #[cfg(feature = "network")]
    let mut network = network;
    #[cfg(feature = "audio")]
    let mut audio = audio;
    #[cfg(feature = "bluetooth")]
    let mut bluetooth = bluetooth;
    #[cfg(feature = "network")]
    if let InitialRoute::Network(page) = initial {
      network.page = page;
      network.reload();
    }
    #[cfg(feature = "audio")]
    if let InitialRoute::Audio(page) = initial {
      audio.page = page;
      audio.reload();
    }
    #[cfg(feature = "bluetooth")]
    if let InitialRoute::Bluetooth(page) = initial {
      bluetooth.page = page;
      bluetooth.reload();
    }
    #[cfg(feature = "boot")]
    let mut boot = boot;
    #[cfg(feature = "boot")]
    if let InitialRoute::Boot(page) = initial {
      boot.page = page;
      boot.reload();
    }
    #[cfg(feature = "packages")]
    let mut packages = packages;
    #[cfg(feature = "packages")]
    if let InitialRoute::Packages(page) = initial {
      packages.page = page;
      packages.reload();
    }
    #[cfg(feature = "storage")]
    let mut storage = storage;
    #[cfg(feature = "storage")]
    if let InitialRoute::Storage(page) = initial {
      storage.page = page;
      storage.reload();
    }
    #[cfg(feature = "diagnostics")]
    let mut diagnostics = diagnostics;
    #[cfg(feature = "diagnostics")]
    if let InitialRoute::Diagnostics(page) = initial {
      diagnostics.page = page;
      diagnostics.reload();
    }
    #[cfg(feature = "session")]
    if let InitialRoute::Session(page) = initial {
      session.page = page;
      session.reload();
    }
    #[cfg(feature = "displays")]
    if let InitialRoute::Displays(page) = initial {
      displays.page = page;
      displays.reload();
    }
    let mut search_registry = SearchRegistry::default();
    let entries: [(&str, &str, &str, &str, &str); _] = [
      #[cfg(feature = "hardware")]
      (
        "hardware.summary",
        "hardware",
        "Summary",
        "summary hardware overview",
        "hardware/summary",
      ),
      #[cfg(feature = "hardware")]
      (
        "hardware.cpu",
        "hardware",
        "CPU",
        "cpu processor",
        "hardware/cpu",
      ),
      #[cfg(feature = "hardware")]
      (
        "hardware.gpu",
        "hardware",
        "GPU",
        "gpu graphics video driver",
        "hardware/gpu",
      ),
      #[cfg(feature = "hardware")]
      (
        "hardware.memory",
        "hardware",
        "Memory",
        "memory ram",
        "hardware/memory",
      ),
      #[cfg(feature = "hardware")]
      (
        "hardware.power",
        "hardware",
        "Power",
        "power energy battery",
        "hardware/power",
      ),
      #[cfg(feature = "hardware")]
      (
        "hardware.devices",
        "hardware",
        "Devices",
        "devices pci usb drm input",
        "hardware/devices",
      ),
      #[cfg(feature = "services")]
      (
        "services.system",
        "services",
        "System",
        "systemd services service daemon",
        "services/system",
      ),
      #[cfg(feature = "services")]
      (
        "services.user",
        "services",
        "User",
        "systemd user service",
        "services/user",
      ),
      #[cfg(feature = "services")]
      (
        "services.failed",
        "services",
        "Failed",
        "failed failures",
        "services/failed",
      ),
      #[cfg(feature = "services")]
      (
        "services.logs",
        "services",
        "Logs",
        "logs journal journalctl",
        "services/logs",
      ),
      #[cfg(feature = "network")]
      (
        "network.status",
        "network",
        "Status",
        "network rede internet",
        "network/status",
      ),
      #[cfg(feature = "network")]
      (
        "network.interfaces",
        "network",
        "Interfaces",
        "interfaces interface rede",
        "network/interfaces",
      ),
      #[cfg(feature = "network")]
      (
        "network.wifi",
        "network",
        "Wi-Fi",
        "wifi wireless wlan sem fio",
        "network/wifi",
      ),
      #[cfg(feature = "network")]
      (
        "network.ethernet",
        "network",
        "Ethernet",
        "ethernet lan",
        "network/ethernet",
      ),
      #[cfg(feature = "network")]
      (
        "network.vpn",
        "network",
        "VPN",
        "vpn wireguard openvpn tunnel",
        "network/vpn",
      ),
      #[cfg(feature = "network")]
      (
        "network.dns",
        "network",
        "DNS",
        "dns resolved nameserver",
        "network/dns",
      ),
      #[cfg(feature = "network")]
      (
        "network.proxy",
        "network",
        "Proxy",
        "proxy http https",
        "network/proxy",
      ),
      #[cfg(feature = "audio")]
      (
        "audio.output",
        "audio",
        "Output",
        "audio sound som output saída",
        "audio/output",
      ),
      #[cfg(feature = "audio")]
      (
        "audio.input",
        "audio",
        "Input",
        "audio microphone microfone input entrada",
        "audio/input",
      ),
      #[cfg(feature = "audio")]
      (
        "audio.devices",
        "audio",
        "Devices",
        "audio devices dispositivos",
        "audio/devices",
      ),
      #[cfg(feature = "bluetooth")]
      (
        "bluetooth.state",
        "bluetooth",
        "State",
        "bluetooth bt estado",
        "bluetooth/state",
      ),
      #[cfg(feature = "bluetooth")]
      (
        "bluetooth.devices",
        "bluetooth",
        "Devices",
        "bluetooth dispositivos",
        "bluetooth/devices",
      ),
      #[cfg(feature = "bluetooth")]
      (
        "bluetooth.pair",
        "bluetooth",
        "Pair",
        "bluetooth pair parear",
        "bluetooth/pair",
      ),
      #[cfg(feature = "boot")]
      (
        "boot.summary",
        "boot",
        "Summary",
        "boot startup inicializacao kernel",
        "boot/summary",
      ),
      #[cfg(feature = "boot")]
      (
        "boot.kernel",
        "boot",
        "Kernel",
        "kernel linux",
        "boot/kernel",
      ),
      #[cfg(feature = "boot")]
      (
        "boot.bootloader",
        "boot",
        "Bootloader",
        "bootloader grub systemd-boot loader",
        "boot/bootloader",
      ),
      #[cfg(feature = "boot")]
      (
        "boot.initramfs",
        "boot",
        "Initramfs",
        "initramfs mkinitcpio",
        "boot/initramfs",
      ),
      #[cfg(feature = "boot")]
      (
        "boot.plymouth",
        "boot",
        "Plymouth",
        "plymouth splash",
        "boot/plymouth",
      ),
      #[cfg(feature = "packages")]
      (
        "packages.search",
        "packages",
        "Search",
        "pacman package pacote",
        "packages/search",
      ),
      #[cfg(feature = "packages")]
      (
        "packages.installed",
        "packages",
        "Installed",
        "installed instalados",
        "packages/installed",
      ),
      #[cfg(feature = "packages")]
      (
        "packages.updates",
        "packages",
        "Updates",
        "update upgrade atualizar",
        "packages/updates",
      ),
      #[cfg(feature = "packages")]
      (
        "packages.orphans",
        "packages",
        "Orphans",
        "orphan órfão orfao",
        "packages/orphans",
      ),
      #[cfg(feature = "packages")]
      (
        "packages.cache",
        "packages",
        "Cache",
        "cache paccache",
        "packages/cache",
      ),
      #[cfg(feature = "packages")]
      (
        "packages.aur",
        "packages",
        "AUR",
        "aur paru yay",
        "packages/aur",
      ),
      #[cfg(feature = "packages")]
      (
        "packages.history",
        "packages",
        "History",
        "history histórico pacman.log",
        "packages/history",
      ),
      #[cfg(feature = "packages")]
      (
        "packages.downgrade",
        "packages",
        "Downgrade",
        "downgrade",
        "packages/downgrade",
      ),
      #[cfg(feature = "packages")]
      (
        "packages.mirrors",
        "packages",
        "Mirrors",
        "mirror reflector",
        "packages/mirrors",
      ),
      #[cfg(feature = "storage")]
      (
        "storage.summary",
        "storage",
        "Summary",
        "storage armazenamento disk disco",
        "storage/summary",
      ),
      #[cfg(feature = "storage")]
      (
        "storage.disks",
        "storage",
        "Disks",
        "disk disco nvme ssd hdd",
        "storage/disks",
      ),
      #[cfg(feature = "storage")]
      (
        "storage.partitions",
        "storage",
        "Partitions",
        "partition partição",
        "storage/partitions",
      ),
      #[cfg(feature = "storage")]
      (
        "storage.filesystems",
        "storage",
        "Filesystems",
        "filesystem sistema arquivos",
        "storage/filesystems",
      ),
      #[cfg(feature = "storage")]
      (
        "storage.mounts",
        "storage",
        "Mount points",
        "mount montagem",
        "storage/mounts",
      ),
      #[cfg(feature = "storage")]
      (
        "storage.smart",
        "storage",
        "SMART",
        "smart health saúde",
        "storage/smart",
      ),
      #[cfg(feature = "storage")]
      (
        "storage.usage",
        "storage",
        "Disk usage",
        "space espaço uso disco",
        "storage/usage",
      ),
      #[cfg(feature = "diagnostics")]
      (
        "diagnostics.summary",
        "diagnostics",
        "Summary",
        "diagnostic diagnóstico health status",
        "diagnostics/summary",
      ),
      #[cfg(feature = "diagnostics")]
      (
        "diagnostics.services",
        "diagnostics",
        "Services",
        "services serviços failed falhos",
        "diagnostics/services",
      ),
      #[cfg(feature = "diagnostics")]
      (
        "diagnostics.boot",
        "diagnostics",
        "Kernel and Boot",
        "boot kernel initramfs",
        "diagnostics/boot",
      ),
      #[cfg(feature = "diagnostics")]
      (
        "diagnostics.graphics",
        "diagnostics",
        "Graphics",
        "graphics gráficos gpu vmwgfx",
        "diagnostics/graphics",
      ),
      #[cfg(feature = "diagnostics")]
      (
        "diagnostics.network",
        "diagnostics",
        "Network",
        "network rede dns",
        "diagnostics/network",
      ),
      #[cfg(feature = "diagnostics")]
      (
        "diagnostics.audio",
        "diagnostics",
        "Audio",
        "audio áudio pipewire",
        "diagnostics/audio",
      ),
      #[cfg(feature = "diagnostics")]
      (
        "diagnostics.bluetooth",
        "diagnostics",
        "Bluetooth",
        "bluetooth",
        "diagnostics/bluetooth",
      ),
      #[cfg(feature = "diagnostics")]
      (
        "diagnostics.storage",
        "diagnostics",
        "Storage",
        "storage armazenamento smart",
        "diagnostics/storage",
      ),
      #[cfg(feature = "diagnostics")]
      (
        "diagnostics.packages",
        "diagnostics",
        "Packages",
        "packages pacotes updates",
        "diagnostics/packages",
      ),
      #[cfg(feature = "diagnostics")]
      (
        "diagnostics.argvus",
        "diagnostics",
        "ARGVUS",
        "argvus session theme tema",
        "diagnostics/argvus",
      ),
      #[cfg(feature = "power")]
      (
        "power.summary",
        "power",
        "Power",
        "power energia energy battery bateria",
        "power/summary",
      ),
      #[cfg(feature = "session")]
      (
        "session.summary",
        "session",
        "Session",
        "session sessão inicio inicializacao",
        "session/summary",
      ),
      #[cfg(feature = "session")]
      (
        "session.components",
        "session",
        "Components",
        "components componentes desktop wayland hyprland",
        "session/components",
      ),
      #[cfg(feature = "session")]
      (
        "session.autostart",
        "session",
        "Autostart",
        "autostart login inicialização iniciar",
        "session/autostart",
      ),
      #[cfg(feature = "session")]
      (
        "session.diagnostics",
        "session",
        "Diagnostics",
        "diagnostic diagnóstico",
        "session/diagnostics",
      ),
      #[cfg(feature = "session")]
      (
        "session.logs",
        "session",
        "Logs",
        "logs journal journalctl",
        "session/logs",
      ),
      #[cfg(feature = "displays")]
      (
        "displays.summary",
        "displays",
        "Displays",
        "displays monitores monitor screens telas",
        "displays/summary",
      ),
      #[cfg(feature = "displays")]
      (
        "displays.mode",
        "displays",
        "Res",
        "resolution resolucao modo mode refresh",
        "displays/mode",
      ),
      #[cfg(feature = "displays")]
      (
        "displays.scale",
        "displays",
        "Scale",
        "scale escala zoom",
        "displays/scale",
      ),
      #[cfg(feature = "displays")]
      (
        "displays.position",
        "displays",
        "Position",
        "position posicao layout arrangement",
        "displays/position",
      ),
      #[cfg(feature = "displays")]
      (
        "displays.vrr",
        "displays",
        "VRR",
        "vrr variable refresh rate freesync",
        "displays/vrr",
      ),
      #[cfg(feature = "displays")]
      (
        "displays.hdr",
        "displays",
        "HDR",
        "hdr high dynamic range",
        "displays/hdr",
      ),
      #[cfg(feature = "displays")]
      (
        "displays.orientation",
        "displays",
        "Orientation",
        "orientation orientacao rotate rotacionar",
        "displays/orientation",
      ),
      #[cfg(feature = "displays")]
      (
        "displays.primary",
        "displays",
        "Primary",
        "primary principal",
        "displays/primary",
      ),
    ];
    for (id, category, title, keywords, route) in entries {
      let _ = search_registry.register(SearchEntry {
        id: id.into(),
        category: category.into(),
        title: title.into(),
        keywords: keywords.split_whitespace().map(str::to_owned).collect(),
        route: route.into(),
      });
    }
    Self {
      lang,
      theme: theme.clone(),
      route,
      home_selected: 0,
      settings: SettingsState::with_context(page, lang, theme.clone()),
      config: ConfigApp::new(lang, theme.clone()),
      #[cfg(feature = "about")]
      about: AboutState::with_context(tab, lang, theme),
      help: false,
      quit: false,
      width: 80,
      height: 24,
      #[cfg(feature = "hardware")]
      hardware,
      #[cfg(feature = "services")]
      services,
      #[cfg(feature = "network")]
      network,
      #[cfg(feature = "audio")]
      audio,
      #[cfg(feature = "bluetooth")]
      bluetooth,
      #[cfg(feature = "boot")]
      boot,
      #[cfg(feature = "packages")]
      packages,
      #[cfg(feature = "storage")]
      storage,
      #[cfg(feature = "diagnostics")]
      diagnostics,
      #[cfg(feature = "power")]
      power,
      #[cfg(feature = "session")]
      session,
      #[cfg(feature = "displays")]
      displays,
      capabilities,
      search_registry,
      jobs: JobManager::default(),
    }
  }

  #[allow(clippy::vec_init_then_push)]
  pub fn home_rows(&self) -> Vec<HomeRow> {
    let mut rows = Vec::new();
    #[cfg(feature = "locale")]
    rows.push(HomeRow::Header(tr(
      self.lang,
      "Idioma e Região",
      "Language & Region",
    )));
    #[cfg(feature = "locale")]
    rows.push(HomeRow::Item {
      label: tr(self.lang, "Locale e Região", "Locale & Region"),
      action: 2,
    });
    rows.push(HomeRow::Header(tr(self.lang, "Aparência", "Appearance")));
    #[cfg(feature = "fonts")]
    rows.push(HomeRow::Item {
      label: tr(self.lang, "Fontes", "Fonts"),
      action: 1,
    });
    rows.push(HomeRow::Header(tr(
      self.lang,
      "Aplicativos",
      "Applications",
    )));
    #[cfg(feature = "apps")]
    rows.push(HomeRow::Item {
      label: tr(self.lang, "Apps Padrão", "Default Apps"),
      action: 0,
    });
    #[cfg(any(feature = "hardware", feature = "displays"))]
    rows.push(HomeRow::Header(tr(self.lang, "Hardware", "Hardware")));
    #[cfg(feature = "hardware")]
    rows.push(HomeRow::Item {
      label: tr(self.lang, "Hardware", "Hardware"),
      action: 4,
    });
    #[cfg(feature = "displays")]
    if self.capabilities.has_hyprctl {
      rows.push(HomeRow::Item {
        label: tr(self.lang, "Monitores", "Displays"),
        action: 18,
      });
    }
    #[cfg(any(feature = "power", feature = "session"))]
    rows.push(HomeRow::Header(tr(
      self.lang,
      "Energia e Sessão",
      "Power & Session",
    )));
    #[cfg(feature = "power")]
    if self.capabilities.has_loginctl {
      rows.push(HomeRow::Item {
        label: tr(self.lang, "Energia", "Power"),
        action: 16,
      });
    }
    #[cfg(feature = "session")]
    if self.capabilities.has_sessionctl {
      rows.push(HomeRow::Item {
        label: tr(self.lang, "Sessão", "Session"),
        action: 17,
      });
    }
    #[cfg(any(feature = "network", feature = "bluetooth"))]
    rows.push(HomeRow::Header(tr(
      self.lang,
      "Conectividade",
      "Connectivity",
    )));
    #[cfg(feature = "network")]
    rows.push(HomeRow::Item {
      label: tr(self.lang, "Rede", "Network"),
      action: 5,
    });
    #[cfg(feature = "bluetooth")]
    if self.capabilities.has_bluetooth {
      rows.push(HomeRow::Item {
        label: tr(self.lang, "Bluetooth", "Bluetooth"),
        action: 7,
      });
    }
    #[cfg(feature = "audio")]
    {
      rows.push(HomeRow::Header(tr(self.lang, "Áudio", "Audio")));
      rows.push(HomeRow::Item {
        label: tr(self.lang, "Áudio", "Audio"),
        action: 6,
      });
    }
    #[cfg(any(
      feature = "boot",
      feature = "packages",
      feature = "services",
      feature = "storage",
      feature = "diagnostics",
      feature = "system"
    ))]
    rows.push(HomeRow::Header(tr(self.lang, "Sistema", "System")));
    #[cfg(feature = "boot")]
    rows.push(HomeRow::Item {
      label: tr(self.lang, "Boot", "Boot"),
      action: 8,
    });
    #[cfg(feature = "packages")]
    rows.push(HomeRow::Item {
      label: tr(self.lang, "Pacotes", "Packages"),
      action: 9,
    });
    #[cfg(feature = "services")]
    rows.push(HomeRow::Item {
      label: tr(self.lang, "Serviços", "Services"),
      action: 10,
    });
    #[cfg(feature = "storage")]
    rows.push(HomeRow::Item {
      label: tr(self.lang, "Armazenamento", "Storage"),
      action: 12,
    });
    #[cfg(feature = "diagnostics")]
    rows.push(HomeRow::Item {
      label: tr(self.lang, "Diagnóstico", "Diagnostics"),
      action: 13,
    });
    #[cfg(feature = "system")]
    rows.push(HomeRow::Item {
      label: tr(self.lang, "Sistema", "System"),
      action: 11,
    });
    #[cfg(feature = "about")]
    rows.push(HomeRow::Item {
      label: tr(self.lang, "About", "About"),
      action: 14,
    });
    rows.push(HomeRow::Header(tr(
      self.lang,
      "Preferências",
      "Preferences",
    )));
    rows.push(HomeRow::Item {
      label: tr(self.lang, "Configuração", "Configuration"),
      action: 15,
    });
    #[cfg(feature = "language")]
    rows.push(HomeRow::Item {
      label: tr(self.lang, "Idioma", "Language"),
      action: 3,
    });
    rows
  }

  pub fn home_item_count(&self) -> usize {
    self
      .home_rows()
      .iter()
      .filter(|row| matches!(row, HomeRow::Item { .. }))
      .count()
  }

  pub fn move_home(&mut self, delta: isize) {
    let last = self.home_item_count().saturating_sub(1);
    self.home_selected = (self.home_selected as isize + delta).clamp(0, last as isize) as usize;
  }

  pub fn open_home(&mut self) {
    let Some(action) = self
      .home_rows()
      .into_iter()
      .filter_map(|row| match row {
        HomeRow::Item { action, .. } => Some(action),
        HomeRow::Header(_) => None,
      })
      .nth(self.home_selected)
    else {
      return;
    };
    match action {
      #[cfg(feature = "apps")]
      0 => self.open_settings(Page::DefaultApps),
      #[cfg(feature = "fonts")]
      1 => self.open_settings(Page::Fonts),
      #[cfg(feature = "locale")]
      2 => self.open_settings(Page::LocaleRegion),
      #[cfg(feature = "language")]
      3 => self.open_settings(Page::Language),
      #[cfg(feature = "hardware")]
      4 => self.route = Route::Hardware,
      #[cfg(feature = "network")]
      5 => {
        self.route = Route::Network;
        self.network.reload();
      }
      #[cfg(feature = "audio")]
      6 => {
        self.route = Route::Audio;
        self.audio.reload();
      }
      #[cfg(feature = "bluetooth")]
      7 => {
        self.route = Route::Bluetooth;
        self.bluetooth.reload();
      }
      #[cfg(feature = "boot")]
      8 => {
        self.route = Route::Boot;
        self.boot.reload();
      }
      #[cfg(feature = "packages")]
      9 => {
        self.route = Route::Packages;
        self.packages.reload();
      }
      #[cfg(feature = "services")]
      10 => {
        self.route = Route::Services;
        self.services.reload();
      }
      #[cfg(feature = "system")]
      11 => self.open_settings(Page::System),
      #[cfg(feature = "storage")]
      12 => {
        self.route = Route::Storage;
        self.storage.reload();
      }
      #[cfg(feature = "diagnostics")]
      13 => {
        self.route = Route::Diagnostics;
        self.diagnostics.reload();
      }
      #[cfg(feature = "about")]
      14 => self.route = Route::About,
      15 => self.route = Route::Config,
      #[cfg(feature = "power")]
      16 => {
        self.route = Route::Power;
        self.power.reload();
      }
      #[cfg(feature = "session")]
      17 => {
        self.route = Route::Session;
        self.session.reload();
      }
      #[cfg(feature = "displays")]
      18 => {
        self.route = Route::Displays;
        self.displays.reload();
      }
      _ => {}
    }
  }

  pub fn open_settings(&mut self, page: Page) {
    self.settings.navigation = argvus_control_center_settings::navigation::Navigation::new(page);
    self.settings.select_current();
    self.route = Route::Settings;
  }

  pub fn back(&mut self) {
    match self.route {
      Route::Home => {}
      Route::Config => self.route = Route::Home,
      #[cfg(feature = "about")]
      Route::About => self.route = Route::Home,
      #[cfg(feature = "hardware")]
      Route::Hardware => {
        if self.hardware.handle(crossterm::event::KeyCode::Esc) {
          self.route = Route::Home;
        }
      }
      #[cfg(feature = "services")]
      Route::Services => {
        if self.services.handle(crossterm::event::KeyCode::Esc) {
          self.route = Route::Home;
        }
      }
      #[cfg(feature = "network")]
      Route::Network => {
        if self.network.handle(crossterm::event::KeyCode::Esc) {
          self.route = Route::Home;
        }
      }
      #[cfg(feature = "audio")]
      Route::Audio => {
        if self.audio.handle(crossterm::event::KeyCode::Esc) {
          self.route = Route::Home;
        }
      }
      #[cfg(feature = "bluetooth")]
      Route::Bluetooth => {
        if self.bluetooth.handle(crossterm::event::KeyCode::Esc) {
          self.route = Route::Home;
        }
      }
      #[cfg(feature = "boot")]
      Route::Boot => {
        if self.boot.handle(crossterm::event::KeyCode::Esc) {
          self.route = Route::Home;
        }
      }
      #[cfg(feature = "packages")]
      Route::Packages => {
        if self.packages.handle(crossterm::event::KeyCode::Esc) {
          self.route = Route::Home;
        }
      }
      #[cfg(feature = "storage")]
      Route::Storage => {
        if self.storage.handle(crossterm::event::KeyCode::Esc) {
          self.route = Route::Home;
        }
      }
      #[cfg(feature = "diagnostics")]
      Route::Diagnostics => {
        if self.diagnostics.handle(crossterm::event::KeyCode::Esc) {
          self.route = Route::Home;
        }
      }
      #[cfg(feature = "power")]
      Route::Power => {
        if self.power.handle(crossterm::event::KeyCode::Esc) {
          self.route = Route::Home;
        }
      }
      #[cfg(feature = "session")]
      Route::Session => {
        if self.session.handle(crossterm::event::KeyCode::Esc) {
          self.route = Route::Home;
        }
      }
      #[cfg(feature = "displays")]
      Route::Displays => {
        if self.displays.handle(crossterm::event::KeyCode::Esc) {
          self.route = Route::Home;
        }
      }
      Route::Settings => {
        self.settings.back();
        if self.settings.page() == Page::Main {
          self.route = Route::Home;
        }
      }
    }
  }

  pub fn resize(&mut self, width: u16, height: u16) {
    self.width = width;
    self.height = height;
    self.settings.resize(width, height);
    #[cfg(feature = "about")]
    self.about.resize(width, height);
  }

  pub fn help_lines(&self) -> Vec<String> {
    let mut lines: Vec<&str> = vec![
      tr(self.lang, "Global", "Global"),
      tr(self.lang, "q             Sair", "q             Quit"),
      tr(self.lang, "Esc           Voltar", "Esc           Back"),
      tr(
        self.lang,
        "?             Fechar ajuda",
        "?             Close help",
      ),
      "",
    ];
    if self.route == Route::Home {
      lines.extend([
        tr(self.lang, "Menu", "Menu"),
        tr(self.lang, "↑/↓           Navegar", "↑/↓           Navigate"),
        tr(self.lang, "→/Enter       Abrir", "→/Enter       Open"),
        tr(
          self.lang,
          "s             Configuração",
          "s             Configuration",
        ),
      ]);
    }
    if self.route == Route::Config {
      lines.extend([
        tr(self.lang, "Configuração", "Configuration"),
        tr(
          self.lang,
          "Enter/Space    Alternar",
          "Enter/Space    Toggle",
        ),
        tr(self.lang, "←/Esc          Voltar", "←/Esc          Back"),
      ]);
    }
    if self.route == Route::Settings {
      lines.extend([
        tr(self.lang, "Listas", "Lists"),
        tr(self.lang, "↑/↓           Navegar", "↑/↓           Navigate"),
        tr(
          self.lang,
          "Enter         Abrir/aplicar",
          "Enter         Open/apply",
        ),
        tr(self.lang, "/             Buscar", "/             Search"),
      ]);
    }
    #[cfg(feature = "about")]
    if self.route == Route::About {
      lines.extend([
        tr(self.lang, "About", "About"),
        tr(self.lang, "←/→           Abas", "←/→           Tabs"),
        tr(
          self.lang,
          "↑/↓           Navegar/rolar",
          "↑/↓           Navigate/scroll",
        ),
        tr(self.lang, "PgUp/PgDn    Rolar", "PgUp/PgDn    Scroll"),
        tr(
          self.lang,
          "Enter         Abrir ação",
          "Enter         Open action",
        ),
      ]);
    }
    #[cfg(any(
      feature = "hardware",
      feature = "services",
      feature = "network",
      feature = "audio",
      feature = "bluetooth",
      feature = "boot",
      feature = "packages",
      feature = "storage",
      feature = "diagnostics",
      feature = "power",
      feature = "session",
      feature = "displays"
    ))]
    {
      let mut domain = false;
      #[cfg(feature = "hardware")]
      {
        domain |= self.route == Route::Hardware;
      }
      #[cfg(feature = "services")]
      {
        domain |= self.route == Route::Services;
      }
      #[cfg(feature = "network")]
      {
        domain |= self.route == Route::Network;
      }
      #[cfg(feature = "audio")]
      {
        domain |= self.route == Route::Audio;
      }
      #[cfg(feature = "bluetooth")]
      {
        domain |= self.route == Route::Bluetooth;
      }
      #[cfg(feature = "boot")]
      {
        domain |= self.route == Route::Boot;
      }
      #[cfg(feature = "packages")]
      {
        domain |= self.route == Route::Packages;
      }
      #[cfg(feature = "storage")]
      {
        domain |= self.route == Route::Storage;
      }
      #[cfg(feature = "diagnostics")]
      {
        domain |= self.route == Route::Diagnostics;
      }
      #[cfg(feature = "power")]
      {
        domain |= self.route == Route::Power;
      }
      #[cfg(feature = "session")]
      {
        domain |= self.route == Route::Session;
      }
      #[cfg(feature = "displays")]
      {
        domain |= self.route == Route::Displays;
      }
      if domain {
        lines.extend([
          tr(self.lang, "↑/↓           Navegar", "↑/↓           Navigate"),
          tr(self.lang, "→/Enter       Abrir", "→/Enter       Open"),
          tr(self.lang, "←/Esc         Voltar", "←/Esc         Back"),
          tr(
            self.lang,
            "r             Atualizar",
            "r             Refresh",
          ),
        ]);
      }
    }
    lines.into_iter().map(str::to_string).collect()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn home_navigation_is_bounded() {
    let mut app = App::new(InitialRoute::Home);
    app.move_home(-1);
    assert_eq!(app.home_selected, 0);
    app.move_home(20);
    assert_eq!(app.home_selected, app.home_item_count().saturating_sub(1));
  }

  #[test]
  fn home_navigation_never_selects_a_header() {
    let mut app = App::new(InitialRoute::Home);
    let rows = app.home_rows();
    for index in 0..app.home_item_count() {
      app.home_selected = index;
      assert!(matches!(
        rows
          .iter()
          .filter(|row| matches!(row, HomeRow::Item { .. }))
          .nth(app.home_selected),
        Some(HomeRow::Item { .. })
      ));
    }
  }

  #[cfg(feature = "about")]
  #[test]
  fn entering_and_leaving_about_preserves_tab() {
    let mut app = App::new(InitialRoute::About(Tab::Credits));
    app.back();
    assert_eq!(app.route, Route::Home);
    app.home_selected = app
      .home_rows()
      .iter()
      .filter_map(|row| match row {
        HomeRow::Item { action, .. } => Some(*action),
        HomeRow::Header(_) => None,
      })
      .position(|action| action == 14)
      .expect("About row must be listed");
    app.open_home();
    assert_eq!(app.route, Route::About);
    assert_eq!(app.about.active_tab, Tab::Credits);
  }

  #[test]
  fn home_lists_and_opens_the_configuration_screen() {
    let mut app = App::new(InitialRoute::Home);
    app.home_selected = app
      .home_rows()
      .iter()
      .filter_map(|row| match row {
        HomeRow::Item { action, .. } => Some(*action),
        HomeRow::Header(_) => None,
      })
      .position(|action| action == 15)
      .expect("Configuration row must be listed");
    app.open_home();
    assert_eq!(app.route, Route::Config);
    app.back();
    assert_eq!(app.route, Route::Home);
  }
}
