use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
  Frame,
  layout::Rect,
  style::Style,
  widgets::{Block, Clear, Paragraph},
};
use serde_json::{Value, json};

use crate::{
  Page,
  app::{App, PendingAction, Row},
  i18n::{Lang, tr},
};

const FIREWALL_FIELDS: [(&str, &str, &str); 15] = [
  ("INTERFACE_WAN", "Interface WAN", "WAN interface"),
  ("INTERFACE_LAN", "Interface LAN", "LAN interface"),
  ("MASQUERADE_ENABLE", "Mascaramento NAT", "NAT masquerading"),
  ("PROTECTION_LEVEL", "Nível de proteção", "Protection level"),
  ("ALLOW_SSH", "Permitir SSH", "Allow SSH"),
  ("SSH_CLIENTS_IP", "Redes IPv4 SSH", "SSH IPv4 networks"),
  ("SSH_PORT", "Porta SSH", "SSH port"),
  ("ALLOW_SAMBA", "Permitir Samba", "Allow Samba"),
  (
    "SAMBA_CLIENTS_IP",
    "Redes IPv4 Samba",
    "Samba IPv4 networks",
  ),
  ("ALLOW_ICMP", "Permitir ICMP", "Allow ICMP"),
  ("OPEN_PORTS_UDP", "Portas UDP", "UDP ports"),
  (
    "SYN_FLOOD_PROTECTION",
    "Proteção SYN flood",
    "SYN flood protection",
  ),
  ("DDOS_PROTECTION", "Proteção DDoS", "DDoS protection"),
  (
    "PORT_SCAN_PROTECTION",
    "Proteção contra varredura",
    "Port scan protection",
  ),
  ("ANTI_SPOOFING", "Antifalsificação de IP", "Anti-spoofing"),
];

pub fn is_page(page: Page) -> bool {
  matches!(
    page,
    Page::Firewall
      | Page::Users
      | Page::UserList
      | Page::SystemUsers
      | Page::User
      | Page::CreateUser
      | Page::UserGroups
      | Page::UserPassword
      | Page::UserShell
      | Page::UserPrimaryGroup
      | Page::Groups
      | Page::GroupList
      | Page::SystemGroups
      | Page::Group
      | Page::GroupMembers
      | Page::CreateGroup
  )
}

fn text(value: &Value, key: &str) -> String {
  value[key].as_str().unwrap_or_default().into()
}
fn strings(value: &Value) -> Vec<String> {
  value
    .as_array()
    .map(|items| {
      items
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
    })
    .unwrap_or_default()
}
fn row(label: &str, detail: impl Into<String>) -> Row {
  Row {
    label: label.into(),
    detail: Some(detail.into()),
    current: false,
  }
}
fn plain(label: &str) -> Row {
  Row {
    label: label.into(),
    detail: None,
    current: false,
  }
}
fn section(label: &str) -> Row {
  Row {
    label: format!("-- {label}"),
    detail: None,
    current: false,
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
  Primary,
  Secondary,
  Danger,
}

#[derive(Debug, Clone)]
pub struct Button {
  pub label: String,
  pub kind: ButtonKind,
}

impl Button {
  fn new(label: impl Into<String>, kind: ButtonKind) -> Self {
    Self {
      label: label.into(),
      kind,
    }
  }
}

fn request(firewall: bool, value: Value, privileged: bool) -> Result<Value, String> {
  let binary = if firewall {
    "/usr/bin/argvus-firewall"
  } else {
    "/usr/bin/argvus-accounts"
  };
  let mut command = Command::new(if privileged {
    "/usr/bin/pkexec"
  } else {
    binary
  });
  if privileged {
    command
      .arg("--disable-internal-agent")
      .arg(binary)
      .env_clear();
    for name in ["DBUS_SESSION_BUS_ADDRESS", "XDG_RUNTIME_DIR"] {
      if let Some(value) = std::env::var_os(name) {
        command.env(name, value);
      }
    }
  }
  let mut child = command
    .arg("manage")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|error| format!("{binary}: {error}"))?;
  let payload = serde_json::to_vec(&value).map_err(|error| error.to_string())?;
  let written = child
    .stdin
    .take()
    .ok_or("missing backend input")?
    .write_all(&payload);
  let output = child
    .wait_with_output()
    .map_err(|error| error.to_string())?;
  if !output.status.success() {
    let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    return Err(if message.is_empty() {
      format!("{binary}: {}", output.status)
    } else {
      message
    });
  }
  written.map_err(|error| error.to_string())?;
  serde_json::from_slice(&output.stdout).map_err(|error| format!("{binary}: {error}"))
}

pub struct Administration {
  firewall: Value,
  pub(crate) accounts: Value,
  config: Value,
  pub(crate) user: Value,
  group: Value,
  passwords: [String; 3],
  pub editor: Option<Editor>,
  pending: Option<(bool, Value)>,
  worker: Option<Receiver<Result<(bool, Value), String>>>,
  operation: String,
  pub completed: Option<String>,
}

impl Administration {
  pub fn new(page: Page) -> Self {
    let mut state = Self {
      firewall: Value::Null,
      accounts: Value::Null,
      config: Value::Null,
      user: Value::Null,
      group: Value::Null,
      passwords: Default::default(),
      editor: None,
      pending: None,
      worker: None,
      operation: String::new(),
      completed: None,
    };
    if is_page(page) {
      state.load(page == Page::Firewall);
    }
    state
  }

  pub fn busy(&self) -> bool {
    self.worker.is_some()
  }
  pub fn cancel_pending(&mut self) {
    self.pending = None;
  }

  pub fn load(&mut self, firewall: bool) {
    if self.busy() {
      return;
    }
    self.start(firewall, json!({"action":"snapshot"}), false);
  }

  fn start(&mut self, firewall: bool, value: Value, privileged: bool) {
    self.operation = text(&value, "action");
    let (sender, receiver) = mpsc::channel();
    self.worker = Some(receiver);
    std::thread::spawn(move || {
      let result = request(firewall, value, privileged)
        .and_then(|value| {
          if privileged {
            request(firewall, json!({"action":"snapshot"}), false)
          } else {
            Ok(value)
          }
        })
        .map(|value| (firewall, value));
      let _ = sender.send(result);
    });
  }

  pub fn poll(&mut self) -> Option<Result<(), String>> {
    let result = match self.worker.as_ref()?.try_recv() {
      Ok(result) => result,
      Err(mpsc::TryRecvError::Empty) => return None,
      Err(mpsc::TryRecvError::Disconnected) => Err("backend worker disconnected".into()),
    };
    self.worker = None;
    Some(result.map(|(firewall, value)| {
      self.completed = Some(self.operation.clone());
      if firewall {
        if matches!(self.operation.as_str(), "snapshot" | "save-config") {
          self.config = value["config"].clone();
        }
        self.firewall = value;
      } else {
        if let Some(users) = value["users"].as_array()
          && let Some(user) = users.iter().find(|user| user["user"] == self.user["user"])
          && matches!(self.operation.as_str(), "snapshot" | "edit" | "create")
        {
          self.user = user.clone();
        }
        if let Some(groups) = value["group_details"].as_array()
          && let Some(group) = groups.iter().find(|group| {
            group["name"] == self.group["original"] || group["gid"] == self.group["gid"]
          })
        {
          self.group = group.clone();
          self.group["original"] = self.group["name"].clone();
        }
        self.accounts = value;
        if self.operation == "delete" {
          self.user = Value::Null;
        }
        if self.operation == "delete-group" {
          self.group = Value::Null;
        }
      }
    }))
  }

  pub fn submit(&mut self) {
    if let Some((firewall, value)) = self.pending.take() {
      self.start(firewall, value, true);
    }
  }

  fn users(&self, include_system: bool) -> Vec<Value> {
    self.accounts["users"]
      .as_array()
      .map(|users| {
        users
          .iter()
          .filter(|user| {
            let uid = user["uid"].as_u64().unwrap_or(0);
            include_system
              || (1000..65534).contains(&uid)
              || user["uid"] == self.accounts["actor_uid"]
          })
          .cloned()
          .collect()
      })
      .unwrap_or_default()
  }

  fn groups(&self, include_system: bool) -> Vec<Value> {
    self.accounts["group_details"]
      .as_array()
      .map(|groups| {
        groups
          .iter()
          .filter(|group| include_system || group["gid"].as_u64().unwrap_or(0) >= 1000)
          .cloned()
          .collect()
      })
      .unwrap_or_else(|| {
        strings(&self.accounts["groups"])
          .into_iter()
          .filter(|name| include_system || !matches!(name.as_str(), "root" | "bin" | "daemon"))
          .map(|name| json!({"name":name,"gid":"","members":[]}))
          .collect()
      })
  }

  pub fn buttons(&self, page: Page, lang: Lang) -> Vec<Button> {
    if self.busy() {
      return Vec::new();
    }
    match page {
      Page::User => vec![
        Button::new(
          tr(lang, "Salvar alterações", "Save changes"),
          ButtonKind::Primary,
        ),
        Button::new(
          tr(lang, "Alterar senha", "Change password"),
          ButtonKind::Secondary,
        ),
        Button::new(
          tr(lang, "Bloquear senha", "Lock password"),
          ButtonKind::Secondary,
        ),
        Button::new(
          tr(lang, "Desbloquear senha", "Unlock password"),
          ButtonKind::Secondary,
        ),
        Button::new(
          tr(
            lang,
            "Exigir nova senha no login",
            "Require password change at login",
          ),
          ButtonKind::Secondary,
        ),
        Button::new(
          tr(lang, "Imagem do avatar", "Avatar image"),
          ButtonKind::Secondary,
        ),
        Button::new(
          tr(lang, "Remover avatar", "Remove avatar"),
          ButtonKind::Danger,
        ),
        Button::new(
          tr(
            lang,
            "Excluir usuário (manter home)",
            "Delete user (keep home)",
          ),
          ButtonKind::Danger,
        ),
        Button::new(
          tr(lang, "Excluir usuário e home", "Delete user and home"),
          ButtonKind::Danger,
        ),
      ],
      Page::CreateUser => vec![Button::new(
        tr(
          lang,
          "Criar conta (senha bloqueada)",
          "Create account (password locked)",
        ),
        ButtonKind::Primary,
      )],
      Page::CreateGroup => vec![Button::new(
        tr(lang, "Criar grupo", "Create group"),
        ButtonKind::Primary,
      )],
      Page::Group => vec![
        Button::new(
          tr(lang, "Salvar alterações", "Save changes"),
          ButtonKind::Primary,
        ),
        Button::new(
          tr(lang, "Alterar membros", "Edit members"),
          ButtonKind::Secondary,
        ),
        Button::new(
          tr(lang, "Excluir grupo", "Delete group"),
          ButtonKind::Danger,
        ),
      ],
      Page::Firewall => vec![
        Button::new(
          tr(lang, "Salvar Configuração", "Save configuration"),
          ButtonKind::Primary,
        ),
        Button::new(
          tr(lang, "Adicionar Regras IPTables", "Add IPTables rules"),
          ButtonKind::Secondary,
        ),
        Button::new(
          tr(lang, "Aplicar regras salvas", "Apply saved rules"),
          ButtonKind::Secondary,
        ),
        Button::new(tr(lang, "Cancelar", "Cancel"), ButtonKind::Danger),
      ],
      _ => Vec::new(),
    }
  }

  pub fn rows(&self, page: Page, lang: Lang) -> Vec<Row> {
    if self.busy() {
      return vec![row(
        tr(
          lang,
          "Aguardando backend / autenticação",
          "Waiting for backend / authentication",
        ),
        "",
      )];
    }
    match page {
      Page::Firewall => {
        if self.firewall.is_null() {
          return vec![row(tr(lang, "Recarregar", "Reload"), "")];
        }
        let mut rows = vec![
          row(
            tr(lang, "Serviço", "Service"),
            if self.firewall["active"] == true {
              tr(lang, "Ativo", "Active")
            } else {
              tr(lang, "Parado", "Stopped")
            },
          ),
          row(
            tr(lang, "Iniciar no boot", "Start at boot"),
            if self.firewall["enabled"] == true {
              "[x]"
            } else {
              "[ ]"
            },
          ),
        ];
        rows.extend(FIREWALL_FIELDS.iter().map(|(key, pt, en)| {
          let value = text(&self.config, key);
          row(
            tr(lang, pt, en),
            match value.as_str() {
              "y" => "[x]".into(),
              "n" => "[ ]".into(),
              _ => value,
            },
          )
        }));
        rows
      }
      Page::Users => {
        vec![
          plain(tr(lang, "Criar", "Create")),
          plain(tr(lang, "Listar", "List")),
          plain(tr(lang, "Contas do sistema", "System accounts")),
        ]
      }
      Page::UserList | Page::SystemUsers => {
        let include_system = page == Page::SystemUsers;
        let mut rows = vec![plain(tr(lang, "Recarregar", "Reload"))];
        rows.extend(
          self
            .users(include_system)
            .iter()
            .map(|user| row(&text(user, "user"), text(user, "name"))),
        );
        rows
      }
      Page::CreateUser => vec![
        section(tr(lang, "Conta", "Account")),
        row(tr(lang, "Usuário", "Username"), text(&self.user, "user")),
        row(
          tr(lang, "Nome completo", "Full name"),
          text(&self.user, "name"),
        ),
        row("Shell", text(&self.user, "shell")),
        row(
          tr(lang, "Grupos suplementares", "Supplementary groups"),
          strings(&self.user["groups"]).join(", "),
        ),
      ],
      Page::User => vec![
        section(tr(lang, "Conta", "Account")),
        row(tr(lang, "Usuário", "Username"), text(&self.user, "user")),
        row(
          tr(lang, "Nome completo", "Full name"),
          text(&self.user, "name"),
        ),
        row("Shell", text(&self.user, "shell")),
        row(
          tr(lang, "Grupos suplementares", "Supplementary groups"),
          strings(&self.user["groups"]).join(", "),
        ),
        section(tr(lang, "Identidade", "Identity")),
        row(
          tr(lang, "Grupo primário", "Primary group"),
          text(&self.user, "primary_group"),
        ),
        section(tr(lang, "Informações", "Information")),
        row(
          "UID / GID",
          format!("{} / {}", self.user["uid"], self.user["gid"]),
        ),
        row("Home", text(&self.user, "home")),
      ],
      Page::Groups => vec![
        plain(tr(lang, "Criar", "Create")),
        plain(tr(lang, "Listar", "List")),
      ],
      Page::GroupList | Page::SystemGroups => {
        let mut rows = vec![plain(tr(lang, "Recarregar", "Reload"))];
        rows.extend(self.groups(true).iter().map(|group| {
          let members = strings(&group["members"]);
          row(
            &text(group, "name"),
            if members.is_empty() {
              format!("GID {}", group["gid"])
            } else {
              format!("GID {} | {}", group["gid"], members.join(", "))
            },
          )
        }));
        rows
      }
      Page::CreateGroup => vec![
        section(tr(lang, "Grupo", "Group")),
        row(
          tr(lang, "Nome do grupo", "Group name"),
          text(&self.group, "name"),
        ),
      ],
      Page::Group => vec![
        section(tr(lang, "Grupo", "Group")),
        row(tr(lang, "Nome", "Name"), text(&self.group, "name")),
        row("GID", self.group["gid"].to_string()),
        section(tr(lang, "Membros", "Members")),
        row(
          tr(lang, "Usuários", "Users"),
          strings(&self.group["members"]).join(", "),
        ),
      ],
      Page::GroupMembers => self
        .users(true)
        .iter()
        .map(|user| {
          let name = text(user, "user");
          row(
            &name,
            if strings(&self.group["members"]).contains(&name) {
              "[x]"
            } else {
              "[ ]"
            },
          )
        })
        .collect(),
      Page::UserShell | Page::UserPrimaryGroup => {
        let (source, field) = if page == Page::UserShell {
          ("shells", "shell")
        } else {
          ("groups", "primary_group")
        };
        strings(&self.accounts[source])
          .iter()
          .map(|value| {
            row(
              value,
              if *value == text(&self.user, field) {
                "[x]"
              } else {
                "[ ]"
              },
            )
          })
          .collect()
      }
      Page::UserGroups => strings(&self.accounts["groups"])
        .iter()
        .map(|group| {
          row(
            group,
            if strings(&self.user["groups"]).contains(group)
              || text(&self.user, "primary_group") == *group
            {
              "[x]"
            } else {
              "[ ]"
            },
          )
        })
        .collect(),
      Page::UserPassword => vec![
        row(
          tr(
            lang,
            "Senha atual (própria conta)",
            "Current password (own account)",
          ),
          "*".repeat(self.passwords[0].chars().count()),
        ),
        row(
          tr(lang, "Nova senha", "New password"),
          "*".repeat(self.passwords[1].chars().count()),
        ),
        row(
          tr(lang, "Confirmar senha", "Confirm password"),
          "*".repeat(self.passwords[2].chars().count()),
        ),
        row(tr(lang, "Salvar senha", "Save password"), ""),
      ],
      _ => Vec::new(),
    }
  }

  pub fn rows_filtered(&self, page: Page, lang: Lang, query: &str) -> Vec<Row> {
    let rows = self.rows(page, lang);
    if query.trim().is_empty()
      || !matches!(
        page,
        Page::UserList | Page::SystemUsers | Page::GroupList | Page::SystemGroups
      )
    {
      return rows;
    }
    let query = query.to_lowercase();
    rows
      .into_iter()
      .enumerate()
      .filter(|(index, row)| {
        *index == 0
          || row.label.to_lowercase().contains(&query)
          || row
            .detail
            .as_deref()
            .is_some_and(|detail| detail.to_lowercase().contains(&query))
      })
      .map(|(_, row)| row)
      .collect()
  }

  pub fn row_selectable(&self, page: Page, index: usize) -> bool {
    if self.busy() {
      return false;
    }
    match page {
      Page::User => matches!(index, 2 | 3 | 4 | 6),
      Page::CreateUser => matches!(index, 1..=4),
      Page::UserGroups => strings(&self.accounts["groups"])
        .get(index)
        .is_some_and(|group| *group != text(&self.user, "primary_group")),
      Page::Group => matches!(index, 1),
      Page::GroupMembers => true,
      Page::CreateGroup => matches!(index, 1),
      Page::Firewall
      | Page::Users
      | Page::UserList
      | Page::SystemUsers
      | Page::UserPassword
      | Page::UserShell
      | Page::UserPrimaryGroup
      | Page::Groups
      | Page::GroupList
      | Page::SystemGroups => true,
      _ => true,
    }
  }
}

impl App {
  pub fn admin_paste(&mut self, text: &str) {
    if let Some(editor) = self.admin.editor.as_mut() {
      for character in text.replace("\r\n", "\n").chars() {
        if editor.value.len() >= 262144 {
          break;
        }
        if !character.is_control() || (editor.multiline && matches!(character, '\n' | '\t')) {
          editor.value.insert(editor.cursor, character);
          editor.cursor += 1;
        }
      }
    }
  }
  fn admin_confirm(&mut self, firewall: bool, value: Value, message: String) {
    self.admin.pending = Some((firewall, value));
    self.confirm = Some(PendingAction::Administration(message));
    self.confirm_apply_selected = false;
  }

  pub fn admin_open(&mut self) {
    if self.admin.busy() {
      return;
    }
    let selected = self.navigation.current().selected;
    if !self.row_selectable(selected) {
      return;
    }
    let page = self.page();
    let rows = self.admin.rows_filtered(page, self.lang, &self.search);
    let title = if selected >= rows.len() {
      self
        .admin
        .buttons(page, self.lang)
        .get(selected - rows.len())
        .map(|button| button.label.clone())
        .unwrap_or_default()
    } else {
      rows
        .get(selected)
        .map(|row| row.label.clone())
        .unwrap_or_default()
    };
    if selected >= rows.len() {
      self.admin_button(selected - rows.len());
      return;
    }
    match page {
      Page::Firewall => {
        if self.admin.firewall.is_null() {
          self.admin.load(true);
          return;
        }
        if (2..17).contains(&selected) {
          let key = FIREWALL_FIELDS[selected - 2].0;
          let value = text(&self.admin.config, key);
          if matches!(value.as_str(), "y" | "n") {
            self.admin.config[key] = json!(if value == "y" { "n" } else { "y" });
          } else if key == "PROTECTION_LEVEL" {
            let levels = ["low", "medium", "high", "paranoid"];
            let index = levels.iter().position(|level| *level == value).unwrap_or(0);
            self.admin.config[key] = json!(levels[(index + 1) % levels.len()]);
          } else {
            self.admin.editor = Some(Editor::new(
              title,
              value,
              EditTarget::Config(key.into()),
              false,
            ));
          }
          return;
        }
        let value = match selected {
          0 => {
            json!({"action": if self.admin.firewall["active"] == true { "stop" } else { "start" }})
          }
          1 => {
            json!({"action": if self.admin.firewall["enabled"] == true { "disable" } else { "enable" }})
          }
          _ => return,
        };
        self.admin_confirm(true, value, tr(self.lang, "Alterar o firewall? A conexão de rede pode ser interrompida. Salvar não aplica as regras.", "Change the firewall? Network connectivity may be interrupted. Saving does not apply rules.").into());
      }
      Page::Users => match selected {
        0 => {
          self.admin.user = json!({"user":"", "name":"", "shell":strings(&self.admin.accounts["shells"]).first().cloned().unwrap_or("/bin/bash".into()), "groups":[]});
          self.navigation.push(Page::CreateUser);
          self.normalize_selection();
        }
        1 => {
          self.navigation.push(Page::UserList);
          self.normalize_selection();
        }
        2 => {
          self.navigation.push(Page::SystemUsers);
          self.normalize_selection();
        }
        _ => {}
      },
      Page::UserList | Page::SystemUsers => match selected {
        0 => self.admin.load(false),
        _ => {
          let include_system = page == Page::SystemUsers;
          let user = self
            .admin
            .users(include_system)
            .into_iter()
            .find(|user| text(user, "user") == title);
          if let Some(user) = user {
            self.admin.user = user.clone();
            self.navigation.push(Page::User);
            self.normalize_selection();
          }
        }
      },
      Page::CreateUser | Page::User => {
        let user = text(&self.admin.user, "user");
        match selected {
          1 if page == Page::CreateUser => {
            self.admin.editor = Some(Editor::new(
              title,
              user,
              EditTarget::User("user".into()),
              false,
            ))
          }
          2 => {
            self.admin.editor = Some(Editor::new(
              title,
              text(&self.admin.user, "name"),
              EditTarget::User("name".into()),
              false,
            ))
          }
          3 => {
            self.navigation.push(Page::UserShell);
            self.normalize_selection();
          }
          4 => {
            self.navigation.push(Page::UserGroups);
            self.normalize_selection();
          }
          6 if page == Page::User => {
            self.navigation.push(Page::UserPrimaryGroup);
            self.normalize_selection();
          }
          _ => {}
        }
      }
      Page::Groups => match selected {
        0 => {
          self.admin.group = json!({"name":""});
          self.navigation.push(Page::CreateGroup);
          self.normalize_selection();
        }
        1 => {
          self.navigation.push(Page::GroupList);
          self.normalize_selection();
        }
        _ => {}
      },
      Page::GroupList | Page::SystemGroups => match selected {
        0 => self.admin.load(false),
        _ => {
          let group = self
            .admin
            .groups(true)
            .into_iter()
            .find(|group| text(group, "name") == title);
          if let Some(group) = group {
            self.admin.group = group.clone();
            self.admin.group["original"] = self.admin.group["name"].clone();
            self.navigation.push(Page::Group);
            self.normalize_selection();
          }
        }
      },
      Page::CreateGroup if selected == 1 => {
        self.admin.editor = Some(Editor::new(
          title,
          text(&self.admin.group, "name"),
          EditTarget::Group,
          false,
        ));
      }
      Page::Group if selected == 1 => {
        self.admin.editor = Some(Editor::new(
          title,
          text(&self.admin.group, "name"),
          EditTarget::Group,
          false,
        ));
      }
      Page::GroupMembers => {
        if let Some(user) = self.admin.users(true).get(selected)
          && let Some(member) = user["user"].as_str()
        {
          let mut members = strings(&self.admin.group["members"]);
          if members.iter().any(|value| value == member) {
            members.retain(|value| value != member);
          } else {
            members.push(member.to_string());
          }
          self.admin.group["members"] = json!(members);
        }
      }
      Page::UserShell | Page::UserPrimaryGroup => {
        let (source, field) = if page == Page::UserShell {
          ("shells", "shell")
        } else {
          ("groups", "primary_group")
        };
        if let Some(value) = strings(&self.admin.accounts[source]).get(selected) {
          self.admin.user[field] = json!(value);
          self.navigation.back();
        }
      }
      Page::UserGroups => {
        if let Some(group) = strings(&self.admin.accounts["groups"]).get(selected) {
          if *group == text(&self.admin.user, "primary_group") {
            return;
          }
          let mut groups = strings(&self.admin.user["groups"]);
          if groups.contains(group) {
            groups.retain(|value| value != group);
          } else {
            groups.push(group.clone());
          }
          self.admin.user["groups"] = json!(groups);
        }
      }
      Page::UserPassword => {
        if selected < 3 {
          self.admin.editor = Some(Editor::new(
            title,
            self.admin.passwords[selected].clone(),
            EditTarget::Password(selected),
            false,
          ));
        } else {
          if self.admin.passwords[1].is_empty()
            || self.admin.passwords[1] != self.admin.passwords[2]
          {
            self.error_modal = Some(
              tr(
                self.lang,
                "A nova senha deve ser preenchida e coincidir com a confirmação.",
                "The new password must be nonempty and match its confirmation.",
              )
              .into(),
            );
            return;
          }
          let value = json!({"action":"password", "user":self.admin.user["user"], "old":self.admin.passwords[0], "new":self.admin.passwords[1], "confirm":self.admin.passwords[2]});
          self.admin.passwords = Default::default();
          self.admin_confirm(
            false,
            value,
            format!("{}: {}?", title, text(&self.admin.user, "user")),
          );
        }
      }
      _ => {}
    }
  }

  pub fn admin_button(&mut self, button: usize) {
    let page = self.page();
    let user = text(&self.admin.user, "user");
    let title = self
      .admin
      .buttons(page, self.lang)
      .get(button)
      .map(|button| button.label.clone())
      .unwrap_or_default();
    match page {
      Page::User => match button {
        0 => {
          let value = json!({"action":"edit", "user":user, "name":self.admin.user["name"], "shell":self.admin.user["shell"], "groups":self.admin.user["groups"], "primary_group":self.admin.user["primary_group"]});
          self.admin_confirm(false, value, format!("{title}: {user}?"));
        }
        1 => {
          self.admin.passwords = Default::default();
          self.navigation.push(Page::UserPassword);
          self.normalize_selection();
        }
        2 | 3 => {
          self.admin_confirm(
            false,
            json!({"action":"lock", "user":user, "locked":button == 2}),
            format!("{title}: {user}?"),
          );
        }
        4 => {
          self.admin_confirm(
            false,
            json!({"action":"expire-password", "user":user}),
            format!("{title}: {user}?"),
          );
        }
        5 => {
          self.admin.editor = Some(Editor::new(title, String::new(), EditTarget::Avatar, false));
        }
        6 => {
          self.admin_confirm(
            false,
            json!({"action":"avatar", "user":user, "path":""}),
            format!("{title}: {user}?"),
          );
        }
        7 | 8 => {
          let remove_home = button == 8;
          self.admin_confirm(
            false,
            json!({"action":"delete", "user":user, "remove_home":remove_home}),
            format!("{title}: {user}?"),
          );
        }
        _ => {}
      },
      Page::CreateUser if button == 0 => {
        let value = json!({"action":"create", "user":user, "name":self.admin.user["name"], "shell":self.admin.user["shell"], "groups":self.admin.user["groups"]});
        self.admin_confirm(false, value, format!("{title}: {user}?"));
      }
      Page::CreateGroup if button == 0 => {
        let group = text(&self.admin.group, "name");
        self.admin_confirm(
          false,
          json!({"action":"create-group", "group":group}),
          format!("{title}: {group}?"),
        );
      }
      Page::Group => match button {
        0 => {
          let group = text(&self.admin.group, "name");
          self.admin_confirm(
            false,
            json!({
              "action":"edit-group",
              "group": text(&self.admin.group, "original"),
              "name": group,
              "members": self.admin.group["members"]}),
            format!("{title}: {}?", text(&self.admin.group, "original")),
          );
        }
        1 => {
          self.navigation.push(Page::GroupMembers);
          self.normalize_selection();
        }
        2 => {
          let group = text(&self.admin.group, "original");
          self.admin_confirm(
            false,
            json!({"action":"delete-group", "group":group}),
            format!("{title}: {group}?"),
          );
        }
        _ => {}
      },
      Page::Firewall => match button {
        0 => {
          let value = json!({"action":"save-config", "original":self.admin.firewall["config_text"], "config":self.admin.config});
          self.admin_confirm(true, value, format!("{title}?"));
        }
        1 => {
          self.admin.editor = Some(Editor::new(
            title,
            text(&self.admin.firewall, "rules"),
            EditTarget::Rules,
            true,
          ));
        }
        2 => {
          self.admin_confirm(true, json!({"action":"restart"}), format!("{title}?"));
        }
        3 => {
          self.admin.load(true);
        }
        _ => {}
      },
      _ => {}
    }
  }

  pub fn admin_input(&mut self, key: KeyEvent) {
    let Some(mut editor) = self.admin.editor.take() else {
      return;
    };
    if key.code == KeyCode::Esc {
      return;
    }
    let save = (key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL))
      || (key.code == KeyCode::Enter && !editor.multiline);
    if save {
      let value: String = editor.value.iter().collect();
      match editor.target {
        EditTarget::Config(key) => self.admin.config[&key] = json!(value),
        EditTarget::User(key) => self.admin.user[&key] = json!(value),
        EditTarget::Password(index) => self.admin.passwords[index] = value,
        EditTarget::Rules => self.admin_confirm(
          true,
          json!({"action":"save-rules", "original":self.admin.firewall["rules"], "rules":value}),
          tr(
            self.lang,
            "Salvar rules.fw? Este script será executado como root ao aplicar o firewall.",
            "Save rules.fw? This script will run as root when applying the firewall.",
          )
          .into(),
        ),
        EditTarget::Group => self.admin.group["name"] = json!(value),
        EditTarget::DateTime => match crate::system::time::set_local_time(&value) {
          Ok(()) => self.refresh_time(),
          Err(error) => self.fail(error),
        },
        EditTarget::Avatar => self.admin_confirm(
          false,
          json!({"action":"avatar", "user":self.admin.user["user"], "path":value}),
          tr(self.lang, "Alterar avatar?", "Change avatar?").into(),
        ),
      }
    } else {
      editor.input(key);
      self.admin.editor = Some(editor);
    }
  }
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum EditTarget {
  Config(String),
  User(String),
  Password(usize),
  Rules,
  Group,
  Avatar,
  DateTime,
}

pub struct Editor {
  title: String,
  value: Vec<char>,
  cursor: usize,
  target: EditTarget,
  multiline: bool,
}

impl Editor {
  pub(crate) fn new(title: String, value: String, target: EditTarget, multiline: bool) -> Self {
    let value: Vec<char> = value.chars().collect();
    Self {
      title,
      cursor: value.len(),
      value,
      target,
      multiline,
    }
  }

  fn input(&mut self, key: KeyEvent) {
    match key.code {
      KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
      KeyCode::Right => self.cursor = (self.cursor + 1).min(self.value.len()),
      KeyCode::Home => {
        self.cursor = self.value[..self.cursor]
          .iter()
          .rposition(|c| *c == '\n')
          .map(|i| i + 1)
          .unwrap_or(0)
      }
      KeyCode::End => {
        self.cursor += self.value[self.cursor..]
          .iter()
          .position(|c| *c == '\n')
          .unwrap_or(self.value.len() - self.cursor)
      }
      KeyCode::Up | KeyCode::Down => {
        let start = self.value[..self.cursor]
          .iter()
          .rposition(|c| *c == '\n')
          .map(|i| i + 1)
          .unwrap_or(0);
        let column = self.cursor - start;
        if key.code == KeyCode::Up && start > 0 {
          let previous = self.value[..start - 1]
            .iter()
            .rposition(|c| *c == '\n')
            .map(|i| i + 1)
            .unwrap_or(0);
          self.cursor = (previous + column).min(start - 1);
        } else if key.code == KeyCode::Down
          && let Some(end) = self.value[self.cursor..].iter().position(|c| *c == '\n')
        {
          let next = self.cursor + end + 1;
          let length = self.value[next..]
            .iter()
            .position(|c| *c == '\n')
            .unwrap_or(self.value.len() - next);
          self.cursor = next + column.min(length);
        }
      }
      KeyCode::Backspace if self.cursor > 0 => {
        self.cursor -= 1;
        self.value.remove(self.cursor);
      }
      KeyCode::Delete if self.cursor < self.value.len() => {
        self.value.remove(self.cursor);
      }
      KeyCode::Enter if self.multiline => {
        self.value.insert(self.cursor, '\n');
        self.cursor += 1;
      }
      KeyCode::Char(character)
        if !key
          .modifiers
          .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
          && !character.is_control()
          && self.value.len() < 262144 =>
      {
        self.value.insert(self.cursor, character);
        self.cursor += 1;
      }
      _ => {}
    }
  }

  pub fn draw(&self, frame: &mut Frame, area: Rect, app: &App) {
    let area = crate::ui::layout::centered(
      area,
      area.width.saturating_sub(6),
      if self.multiline {
        area.height.saturating_sub(4)
      } else {
        5
      },
    );
    frame.render_widget(Clear, area);
    let block = Block::bordered()
      .title(self.title.as_str())
      .title_bottom(tr(
        app.lang,
        " Enter / Ctrl+S: salvar | Esc: cancelar ",
        " Enter / Ctrl+S: save | Esc: cancel ",
      ))
      .style(
        Style::new()
          .fg(app.theme.foreground)
          .bg(app.theme.background),
      );
    let inner = block.inner(area);
    let secret = matches!(self.target, EditTarget::Password(_));
    let text: String = self
      .value
      .iter()
      .map(|c| if secret { '*' } else { *c })
      .collect();
    let before: String = self.value[..self.cursor].iter().collect();
    let line = before.chars().filter(|c| *c == '\n').count();
    let current_line = before.rsplit('\n').next().unwrap_or("");
    let column = if secret {
      current_line.chars().count()
    } else {
      ratatui::text::Line::from(current_line).width()
    };
    let vertical = line.saturating_sub(inner.height.saturating_sub(1) as usize);
    let horizontal = column.saturating_sub(inner.width.saturating_sub(1) as usize);
    frame.render_widget(block, area);
    frame.render_widget(
      Paragraph::new(text).scroll((vertical as u16, horizontal as u16)),
      inner,
    );
    frame.set_cursor_position((
      inner.x + (column - horizontal) as u16,
      inner.y + (line - vertical) as u16,
    ));
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn app(page: Page) -> App {
    let mut app = App::new(Page::Main);
    app.error_modal = None;
    app.navigation.push(page);
    app.admin.accounts = json!({"actor_uid":1000, "shells":["/bin/bash", "/bin/zsh"], "groups":["users", "wheel", "audio"], "group_details":[
      {"name":"audio","gid":986,"members":["alice"]},
      {"name":"users","gid":1000,"members":[]},
      {"name":"wheel","gid":998,"members":["alice"]}
    ], "users":[
      {"user":"root", "uid":0, "gid":0, "name":"root", "shell":"/bin/bash", "primary_group":"root", "groups":[]},
      {"user":"alice", "uid":1000, "gid":1000, "name":"Alice", "shell":"/bin/bash", "primary_group":"users", "groups":["wheel"]}
    ]});
    app.admin.user = app.admin.accounts["users"][1].clone();
    app
  }

  #[test]
  fn every_firewall_field_is_editable_without_writing() {
    let mut app = app(Page::Firewall);
    app.admin.firewall = json!({"active":false,"enabled":false});
    app.admin.config = json!({});
    for (index, (key, _, _)) in FIREWALL_FIELDS.iter().enumerate() {
      app.admin.config[*key] = json!(if *key == "PROTECTION_LEVEL" {
        "high"
      } else {
        ""
      });
      app.navigation.current_mut().selected = index + 2;
      app.admin_open();
      if *key == "PROTECTION_LEVEL" {
        assert_eq!(app.admin.config[*key], "paranoid");
      } else {
        assert!(app.admin.editor.take().is_some());
      }
    }
    app.admin.config["ALLOW_SSH"] = json!("n");
    app.navigation.current_mut().selected = 6;
    app.admin_open();
    assert_eq!(app.admin.config["ALLOW_SSH"], "y");
    assert!(!app.admin.busy());
  }

  #[test]
  fn user_group_selection_preserves_primary_group() {
    let mut app = app(Page::UserGroups);
    app.admin_open();
    assert_eq!(strings(&app.admin.user["groups"]), vec!["wheel"]);
    app.navigation.current_mut().selected = 2;
    app.admin_open();
    assert_eq!(strings(&app.admin.user["groups"]), vec!["wheel", "audio"]);
    app.admin_open();
    assert_eq!(strings(&app.admin.user["groups"]), vec!["wheel"]);
  }

  #[test]
  fn destructive_actions_default_to_cancel_and_never_run_on_selection() {
    let mut app = app(Page::User);
    app.navigation.current_mut().selected = 17;
    app.admin_open();
    assert!(app.confirm.is_some());
    assert!(!app.confirm_apply_selected);
    assert!(!app.admin.busy());
    app.confirm_accept();
    assert!(app.admin.pending.is_none());
    assert!(!app.admin.busy());
  }

  #[test]
  fn password_mismatch_stays_local_and_passwords_are_masked() {
    let mut app = app(Page::UserPassword);
    app.admin.passwords = ["old secret".into(), "new secret".into(), "different".into()];
    app.navigation.current_mut().selected = 3;
    app.admin_open();
    assert!(app.error_modal.is_some());
    assert!(app.admin.pending.is_none());
    for row in app.rows() {
      assert!(!row.detail.unwrap_or_default().contains("secret"));
    }
  }

  #[test]
  fn multiline_editor_moves_between_lines_and_accepts_paste() {
    let mut app = app(Page::Firewall);
    app.admin.editor = Some(Editor::new(
      "Rules".into(),
      "abc\nxy".into(),
      EditTarget::Rules,
      true,
    ));
    app.admin_input(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(app.admin.editor.as_ref().unwrap().cursor, 2);
    app.admin_paste("q?\niptables -A INPUT -j ACCEPT");
    assert!(
      app
        .admin
        .editor
        .as_ref()
        .unwrap()
        .value
        .iter()
        .collect::<String>()
        .contains("q?\niptables")
    );
    app.admin_input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.admin.editor.is_none());
    assert!(app.admin.pending.is_none());
  }

  #[test]
  fn system_users_are_optional_and_own_user_is_visible() {
    let app = app(Page::Users);
    assert_eq!(app.admin.users(false).len(), 1);
    assert_eq!(app.admin.users(true).len(), 2);
  }

  #[test]
  fn users_overview_is_separate_from_account_list() {
    let mut app = app(Page::Users);
    let labels = app
      .rows()
      .into_iter()
      .map(|row| row.label)
      .collect::<Vec<_>>();
    assert_eq!(labels, vec!["Criar", "Listar", "Contas do sistema"]);
    app.navigation.current_mut().selected = 1;
    app.admin_open();
    assert_eq!(app.page(), Page::UserList);
    assert_eq!(app.rows()[0].label, "Recarregar");
    assert!(app.rows().iter().any(|row| row.label == "alice"));
  }

  #[test]
  fn readonly_user_and_group_rows_are_not_selectable() {
    let mut user_app = app(Page::User);
    user_app.normalize_selection();
    assert_eq!(user_app.navigation.current().selected, 2);
    assert!(!user_app.row_selectable(0));
    assert!(!user_app.row_selectable(1));
    assert!(!user_app.row_selectable(8));
    assert!(!user_app.row_selectable(9));
    assert_eq!(user_app.item_count(), 19);
    assert!(user_app.row_selectable(18));

    let group_app = app(Page::Group);
    assert!(!group_app.row_selectable(0));
    assert!(group_app.row_selectable(1));
    assert!(!group_app.row_selectable(2));
    assert!(!group_app.row_selectable(4));
  }

  #[test]
  fn group_actions_are_buttons_outside_the_field_list() {
    let app = app(Page::Group);
    let rows = app.admin.rows(Page::Group, Lang::Pt);
    assert!(rows.iter().all(|row| !row.label.contains("Excluir grupo")));
    let buttons = app.admin.buttons(Page::Group, Lang::Pt);
    assert_eq!(buttons.len(), 3);
    assert_eq!(buttons[0].label, "Salvar alterações");
    assert_eq!(buttons[2].kind, ButtonKind::Danger);
    assert!(app.admin.row_selectable(Page::Group, 1));
  }

  #[test]
  fn group_members_picker_toggles_users_in_list() {
    let mut app = app(Page::GroupMembers);
    app.admin.accounts = json!({
      "users": [
        {"user":"alice","uid":1000,"shell":"/bin/bash","groups":["users"],"password":"x","locked":false},
        {"user":"bob","uid":1001,"shell":"/bin/bash","groups":["users"],"password":"x","locked":false}
      ],
      "groups": [],
      "group_details": []
    });
    app.admin.group = json!({"name":"dev","gid":1000,"members":["alice"]});
    assert_eq!(strings(&app.admin.group["members"]), vec!["alice"]);
    app.navigation.current_mut().selected = 1;
    app.admin_open();
    assert_eq!(strings(&app.admin.group["members"]), vec!["alice", "bob"]);
    app.admin_open();
    assert_eq!(strings(&app.admin.group["members"]), vec!["alice"]);
  }

  #[test]
  fn group_open_keeps_original_name_for_rename_and_delete() {
    let mut app = app(Page::GroupList);
    app.admin.accounts = json!({
      "users": [],
      "groups": [{"name":"dev","gid":1000,"members":["alice"]}],
      "group_details": [{"name":"dev","gid":1000,"members":["alice"]}]
    });
    app.navigation.current_mut().selected = 1;
    app.admin_open();
    assert_eq!(app.page(), Page::Group);
    assert_eq!(text(&app.admin.group, "original"), "dev");
  }

  #[test]
  fn firewall_actions_are_buttons_outside_the_config_rows() {
    let mut app = app(Page::Firewall);
    app.admin.firewall = json!({"active":false,"enabled":false});
    app.admin.config = json!({});
    let rows = app.admin.rows(Page::Firewall, Lang::Pt);
    assert_eq!(rows.len(), 2 + FIREWALL_FIELDS.len());
    assert!(
      rows
        .iter()
        .all(|row| !row.label.contains("Editar rules.fw"))
    );
    let buttons = app.admin.buttons(Page::Firewall, Lang::Pt);
    assert_eq!(buttons.len(), 4);
    assert_eq!(buttons[0].label, "Salvar Configuração");
    assert_eq!(buttons[3].label, "Cancelar");
    assert_eq!(buttons[3].kind, ButtonKind::Danger);
  }

  #[test]
  fn firewall_buttons_route_to_confirm_editor_reload_and_cancel_like_before() {
    let mut app = app(Page::Firewall);
    app.admin.firewall = json!({
      "active":false,
      "enabled":false,
      "config_text":"",
      "rules":"iptables -A INPUT -j DROP"
    });
    app.admin.config = json!({"ALLOW_SSH":"n"});
    app.admin_button(0); // Salvar Configuração
    assert!(app.admin.pending.is_some());
    app.admin.pending = None;
    app.admin_button(1); // Adicionar Regras
    assert_eq!(app.admin.editor.take().unwrap().target, EditTarget::Rules);
    app.admin_button(2); // Aplicar regras salvas
    assert!(app.admin.pending.is_some());
    app.admin.pending = None;
    app.admin_button(3); // Cancelar
    assert!(app.admin.busy());
  }

  #[test]
  fn user_actions_are_buttons_outside_the_field_list() {
    let app = app(Page::User);
    let rows = app.admin.rows(Page::User, Lang::Pt);
    assert!(
      rows
        .iter()
        .all(|row| !row.label.contains("Excluir usuário"))
    );
    let buttons = app.admin.buttons(Page::User, Lang::Pt);
    assert_eq!(buttons.len(), 9);
    assert_eq!(buttons[0].label, "Salvar alterações");
    assert_eq!(buttons[7].label, "Excluir usuário (manter home)");
    assert_eq!(buttons[8].label, "Excluir usuário e home");
    assert_eq!(app.item_count(), rows.len() + buttons.len());
  }

  #[test]
  fn save_and_delete_buttons_confirm_without_running() {
    let mut app = app(Page::User);
    app.navigation.current_mut().selected = 10;
    app.admin_open();
    assert!(app.confirm.is_some());
    assert!(!app.admin.busy());
  }

  #[test]
  fn down_navigation_reaches_buttons_and_tab_cycles() {
    let mut app = app(Page::User);
    app.normalize_selection();
    assert_eq!(app.navigation.current().selected, 2);
    app.move_selection(1);
    app.move_selection(1);
    assert_eq!(app.navigation.current().selected, 4);
    app.move_selection(1);
    assert_eq!(app.navigation.current().selected, 6);
    app.move_selection(1);
    assert_eq!(app.navigation.current().selected, 6);
    assert!(!app.row_selectable(9));
    assert!(app.row_selectable(10));

    app.cycle_selection(1);
    assert_eq!(app.navigation.current().selected, 10);
    assert!(app.on_buttons());
    app.cycle_selection(1);
    assert_eq!(app.navigation.current().selected, 6);
    assert!(!app.on_buttons());

    app.cycle_selection(1);
    assert_eq!(app.navigation.current().selected, 10);
    app.move_button(1);
    app.move_button(1);
    assert_eq!(app.navigation.current().selected, 12);
    app.move_button(1);
    app.move_button(1);
    app.move_button(1);
    app.move_button(1);
    app.move_button(1);
    assert_eq!(app.navigation.current().selected, 17);
    app.move_button(1);
    assert_eq!(app.navigation.current().selected, 18);
    app.move_button(1);
    assert_eq!(app.navigation.current().selected, 10);
    app.move_button(-1);
    assert_eq!(app.navigation.current().selected, 18);

    let before = app.navigation.current().selected;
    app.move_selection(1);
    app.move_selection(-1);
    assert_eq!(app.navigation.current().selected, before);

    app.cycle_selection(-1);
    assert_eq!(app.navigation.current().selected, 6);
    app.cycle_selection(-1);
    assert_eq!(app.navigation.current().selected, 18);
  }

  #[test]
  fn tab_cycles_action_buttons_through_key_events() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    let mut app = app(Page::User);
    app.normalize_selection();
    assert_eq!(app.navigation.current().selected, 2);
    let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    let shift_tab = KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE);
    let right = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
    let up = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
    let down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);

    crate::event::handle(&mut app, Event::Key(tab));
    assert_eq!(app.navigation.current().selected, 10);
    crate::event::handle(&mut app, Event::Key(right));
    assert_eq!(app.navigation.current().selected, 11);
    crate::event::handle(&mut app, Event::Key(right));
    assert_eq!(app.navigation.current().selected, 12);
    crate::event::handle(&mut app, Event::Key(right));
    assert_eq!(app.navigation.current().selected, 13);
    crate::event::handle(&mut app, Event::Key(right));
    assert_eq!(app.navigation.current().selected, 14);
    crate::event::handle(&mut app, Event::Key(right));
    assert_eq!(app.navigation.current().selected, 15);
    crate::event::handle(&mut app, Event::Key(right));
    assert_eq!(app.navigation.current().selected, 16);
    crate::event::handle(&mut app, Event::Key(right));
    assert_eq!(app.navigation.current().selected, 17);
    crate::event::handle(&mut app, Event::Key(right));
    assert_eq!(app.navigation.current().selected, 18);
    crate::event::handle(&mut app, Event::Key(right));
    assert_eq!(app.navigation.current().selected, 10);

    let before = app.navigation.current().selected;
    crate::event::handle(&mut app, Event::Key(up));
    crate::event::handle(&mut app, Event::Key(down));
    assert_eq!(app.navigation.current().selected, before);

    crate::event::handle(&mut app, Event::Key(tab));
    assert_eq!(app.navigation.current().selected, 2);
    crate::event::handle(&mut app, Event::Key(shift_tab));
    assert_eq!(app.navigation.current().selected, 18);
  }

  #[test]
  fn pages_and_password_editor_render_at_minimum_and_large_sizes() {
    use ratatui::{Terminal, backend::TestBackend};
    for (width, height) in [(60, 15), (120, 40)] {
      for page in [
        Page::Users,
        Page::UserList,
        Page::SystemUsers,
        Page::User,
        Page::CreateUser,
        Page::UserGroups,
        Page::UserShell,
        Page::UserPrimaryGroup,
        Page::UserPassword,
        Page::Groups,
        Page::GroupList,
        Page::SystemGroups,
        Page::Group,
        Page::CreateGroup,
      ] {
        let mut app = app(page);
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
          .draw(|frame| crate::ui::draw(&mut app, frame))
          .unwrap();
        assert!(
          terminal
            .backend()
            .buffer()
            .content
            .iter()
            .any(|cell| cell.symbol() != " ")
        );
        if page == Page::User {
          let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
          assert!(rendered.contains("Excluir usuário (manter home)"));
          assert!(!rendered.contains("-- Segurança"));
        }
        app.admin.editor = Some(Editor::new(
          "Password".into(),
          "private-password".into(),
          EditTarget::Password(1),
          false,
        ));
        terminal
          .draw(|frame| crate::ui::draw(&mut app, frame))
          .unwrap();
        let rendered: String = terminal
          .backend()
          .buffer()
          .content
          .iter()
          .map(|cell| cell.symbol())
          .collect();
        assert!(!rendered.contains("private-password"));
        assert!(rendered.contains("****************"));
      }
    }
  }
}
