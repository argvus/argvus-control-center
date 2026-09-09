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
      | Page::User
      | Page::CreateUser
      | Page::UserGroups
      | Page::UserPassword
      | Page::UserShell
      | Page::UserPrimaryGroup
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
  accounts: Value,
  config: Value,
  user: Value,
  passwords: [String; 3],
  show_system: bool,
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
      passwords: Default::default(),
      show_system: false,
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
        self.accounts = value;
        if self.operation == "delete" {
          self.user = Value::Null;
        }
      }
    }))
  }

  pub fn submit(&mut self) {
    if let Some((firewall, value)) = self.pending.take() {
      self.start(firewall, value, true);
    }
  }

  fn users(&self) -> Vec<Value> {
    self.accounts["users"]
      .as_array()
      .map(|users| {
        users
          .iter()
          .filter(|user| {
            let uid = user["uid"].as_u64().unwrap_or(0);
            self.show_system
              || (1000..65534).contains(&uid)
              || user["uid"] == self.accounts["actor_uid"]
          })
          .cloned()
          .collect()
      })
      .unwrap_or_default()
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
        rows.push(row(
          tr(lang, "Salvar configuração", "Save configuration"),
          "",
        ));
        rows.push(row(tr(lang, "Editar rules.fw", "Edit rules.fw"), ""));
        rows.push(row(
          tr(lang, "Aplicar regras salvas", "Apply saved rules"),
          "",
        ));
        rows.push(row(
          tr(
            lang,
            "Recarregar / descartar alterações",
            "Reload / discard changes",
          ),
          "",
        ));
        rows
      }
      Page::Users => {
        let mut rows = vec![
          row(tr(lang, "Criar usuário", "Create user"), ""),
          row(tr(lang, "Criar grupo", "Create group"), ""),
          row(
            tr(lang, "Mostrar contas de sistema", "Show system accounts"),
            if self.show_system { "[x]" } else { "[ ]" },
          ),
          row(tr(lang, "Recarregar", "Reload"), ""),
        ];
        rows.extend(
          self
            .users()
            .iter()
            .map(|user| row(&text(user, "user"), text(user, "name"))),
        );
        rows
      }
      Page::User | Page::CreateUser => {
        let mut rows = vec![
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
        ];
        if page == Page::CreateUser {
          rows.push(row(
            tr(
              lang,
              "Criar conta (senha bloqueada)",
              "Create account (password locked)",
            ),
            "",
          ));
        } else {
          rows.extend([
            row(
              tr(lang, "Grupo primário", "Primary group"),
              text(&self.user, "primary_group"),
            ),
            row(tr(lang, "Salvar alterações", "Save changes"), ""),
            row(tr(lang, "Alterar senha", "Change password"), ""),
            row(tr(lang, "Bloquear senha", "Lock password"), ""),
            row(tr(lang, "Desbloquear senha", "Unlock password"), ""),
            row(
              tr(
                lang,
                "Exigir nova senha no login",
                "Require password change at login",
              ),
              "",
            ),
            row(tr(lang, "Imagem do avatar", "Avatar image"), ""),
            row(tr(lang, "Remover avatar", "Remove avatar"), ""),
            row(
              tr(
                lang,
                "Excluir usuário (preservar home)",
                "Delete user (keep home)",
              ),
              "",
            ),
            row(
              "UID / GID",
              format!("{} / {}", self.user["uid"], self.user["gid"]),
            ),
            row("Home", text(&self.user, "home")),
          ]);
        }
        rows
      }
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
    let page = self.page();
    let title = self
      .admin
      .rows(page, self.lang)
      .get(selected)
      .map(|row| row.label.clone())
      .unwrap_or_default();
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
          17 => {
            json!({"action":"save-config", "original":self.admin.firewall["config_text"], "config":self.admin.config})
          }
          18 => {
            self.admin.editor = Some(Editor::new(
              title,
              text(&self.admin.firewall, "rules"),
              EditTarget::Rules,
              true,
            ));
            return;
          }
          19 => json!({"action":"restart"}),
          20 => {
            self.admin.load(true);
            return;
          }
          _ => return,
        };
        self.admin_confirm(true, value, tr(self.lang, "Alterar o firewall? A conexão de rede pode ser interrompida. Salvar não aplica as regras.", "Change the firewall? Network connectivity may be interrupted. Saving does not apply rules.").into());
      }
      Page::Users => match selected {
        0 => {
          self.admin.user = json!({"user":"", "name":"", "shell":strings(&self.admin.accounts["shells"]).first().cloned().unwrap_or("/bin/bash".into()), "groups":[]});
          self.navigation.push(Page::CreateUser);
        }
        1 => self.admin.editor = Some(Editor::new(title, String::new(), EditTarget::Group, false)),
        2 => self.admin.show_system = !self.admin.show_system,
        3 => self.admin.load(false),
        _ => {
          if let Some(user) = self.admin.users().get(selected - 4) {
            self.admin.user = user.clone();
            self.navigation.push(Page::User);
          }
        }
      },
      Page::CreateUser | Page::User => {
        let user = text(&self.admin.user, "user");
        match selected {
          0 if page == Page::CreateUser => {
            self.admin.editor = Some(Editor::new(
              title,
              user,
              EditTarget::User("user".into()),
              false,
            ))
          }
          1 => {
            self.admin.editor = Some(Editor::new(
              title,
              text(&self.admin.user, "name"),
              EditTarget::User("name".into()),
              false,
            ))
          }
          2 => self.navigation.push(Page::UserShell),
          3 => self.navigation.push(Page::UserGroups),
          4 if page == Page::User => self.navigation.push(Page::UserPrimaryGroup),
          4 | 5 => {
            let mut value = json!({"action":if page == Page::CreateUser { "create" } else { "edit" }, "user":user, "name":self.admin.user["name"], "shell":self.admin.user["shell"], "groups":self.admin.user["groups"]});
            if page == Page::User {
              value["primary_group"] = self.admin.user["primary_group"].clone();
            }
            self.admin_confirm(false, value, format!("{title}: {user}?"));
          }
          6 => {
            self.admin.passwords = Default::default();
            self.navigation.push(Page::UserPassword);
          }
          7..=9 | 11 | 12 => {
            let value = match selected {
              7 | 8 => json!({"action":"lock", "user":user, "locked":selected == 7}),
              9 => json!({"action":"expire-password", "user":user}),
              11 => json!({"action":"avatar", "user":user, "path":""}),
              _ => json!({"action":"delete", "user":user}),
            };
            self.admin_confirm(false, value, format!("{title}: {user}?"));
          }
          10 => {
            self.admin.editor = Some(Editor::new(title, String::new(), EditTarget::Avatar, false))
          }
          _ => {}
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
        EditTarget::Group => self.admin_confirm(
          false,
          json!({"action":"create-group", "group":value}),
          format!("{}: {value}?", tr(self.lang, "Criar grupo", "Create group")),
        ),
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

enum EditTarget {
  Config(String),
  User(String),
  Password(usize),
  Rules,
  Group,
  Avatar,
}

pub struct Editor {
  title: String,
  value: Vec<char>,
  cursor: usize,
  target: EditTarget,
  multiline: bool,
}

impl Editor {
  fn new(title: String, value: String, target: EditTarget, multiline: bool) -> Self {
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
    app.admin.accounts = json!({"actor_uid":1000, "shells":["/bin/bash", "/bin/zsh"], "groups":["users", "wheel", "audio"], "users":[
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
    app.navigation.current_mut().selected = 12;
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
    let mut app = app(Page::Users);
    assert_eq!(app.admin.users().len(), 1);
    app.admin.show_system = true;
    assert_eq!(app.admin.users().len(), 2);
  }

  #[test]
  fn pages_and_password_editor_render_at_minimum_and_large_sizes() {
    use ratatui::{Terminal, backend::TestBackend};
    for (width, height) in [(60, 15), (120, 40)] {
      for page in [
        Page::Users,
        Page::User,
        Page::CreateUser,
        Page::UserGroups,
        Page::UserShell,
        Page::UserPrimaryGroup,
        Page::UserPassword,
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
