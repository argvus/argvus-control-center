//! Implements user and group administration in crate `argvus control center settings`. This separation keeps external effects from contaminating models, routes, or rendering.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
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

use argvus_tui::icons;
use argvus_tui::menu::{Row, draft_actions};

use std::collections::BTreeSet;

use crate::{
  Page,
  app::{App, PendingAction},
  i18n::{Lang, tr},
  item::Item,
};

/// Defines the constant `FIREWALL_FIELDS`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
const FIREWALL_FIELDS: [(&str, &str); 15] = [
  ("INTERFACE_WAN", "control_center.wan_interface"),
  ("INTERFACE_LAN", "control_center.lan_interface"),
  ("MASQUERADE_ENABLE", "control_center.nat_masquerading"),
  ("PROTECTION_LEVEL", "control_center.protection_level"),
  ("ALLOW_SSH", "control_center.allow_ssh"),
  ("SSH_CLIENTS_IP", "control_center.ssh_ipv4_networks"),
  ("SSH_PORT", "control_center.ssh_port"),
  ("ALLOW_SAMBA", "control_center.allow_samba"),
  ("SAMBA_CLIENTS_IP", "control_center.samba_ipv4_networks"),
  ("ALLOW_ICMP", "control_center.allow_icmp"),
  ("OPEN_PORTS_UDP", "control_center.udp_ports"),
  (
    "SYN_FLOOD_PROTECTION",
    "control_center.syn_flood_protection",
  ),
  ("DDOS_PROTECTION", "control_center.ddos_protection"),
  (
    "PORT_SCAN_PROTECTION",
    "control_center.port_scan_protection",
  ),
  ("ANTI_SPOOFING", "control_center.anti_spoofing"),
];

/// Checks the condition represented by `is_page` using only the state available to the module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

/// Executes the `text` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
fn text(value: &Value, key: &str) -> String {
  value[key].as_str().unwrap_or_default().into()
}
/// Executes the `strings` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
/// Case-insensitive search over the visible texts of a list entry.
fn matches_search(query: &str, values: &[&str]) -> bool {
  crate::app::search_matches(query.trim(), values)
}

/// Executes the `request` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
    for name in [
      "PATH",
      "DBUS_SESSION_BUS_ADDRESS",
      "DBUS_SYSTEM_BUS_ADDRESS",
      "XDG_RUNTIME_DIR",
    ] {
      if let Some(value) = std::env::var_os(name) {
        command.env(name, value);
      }
    }
    if std::env::var_os("PATH").is_none() {
      command.env(
        "PATH",
        "/usr/local/sbin:/usr/local/bin:/usr/bin:/usr/sbin:/sbin:/bin",
      );
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

/// Represents `Administration`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
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
  /// Constructs `new` with this module's expected initial state. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

  /// Executes the `busy` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn busy(&self) -> bool {
    self.worker.is_some()
  }

  /// Reports whether the snapshot needed to render `page` has already been loaded.
  fn has_loaded_data(&self, page: Page) -> bool {
    match page {
      Page::Firewall => !self.firewall.is_null(),
      _ => self.is_loaded(),
    }
  }
  /// Executes the `cancel_pending` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn cancel_pending(&mut self) {
    self.pending = None;
  }

  /// Retrieves data for `load` without mixing collection with TUI rendering. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn load(&mut self, firewall: bool) {
    if self.busy() {
      return;
    }
    self.start(firewall, json!({"action":"snapshot"}), false);
  }

  /// Executes the `start` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

  /// Executes the `poll` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
        // A reload never replaces a draft with unsaved changes (D15); a
        // completed write (edit, create...) always refreshes it.
        let reload = self.operation == "snapshot";
        let keep_user = reload && self.user_draft_changed();
        let keep_group = reload && self.group_draft_changed();
        if let Some(users) = value["users"].as_array()
          && let Some(user) = users.iter().find(|user| user["user"] == self.user["user"])
          && matches!(self.operation.as_str(), "snapshot" | "edit" | "create")
          && !keep_user
        {
          self.user = user.clone();
        }
        if !keep_group
          && let Some(groups) = value["group_details"].as_array()
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

  /// Executes the `submit` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn submit(&mut self) {
    if let Some((firewall, value)) = self.pending.take() {
      self.start(firewall, value, true);
    }
  }

  /// Checks the condition represented by `is_loaded` using only the state available to the module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn is_loaded(&self) -> bool {
    self.accounts["users"].is_array()
  }

  /// Executes the `users` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub(crate) fn users(&self, include_system: bool) -> Vec<Value> {
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

  /// Executes the `groups` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub(crate) fn groups(&self, include_system: bool) -> Vec<Value> {
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

  /// Rows of an account or firewall page. `search` narrows the user and
  /// group lists, whose items keep their index in the unfiltered list.
  pub fn rows(&self, page: Page, lang: Lang, search: &str) -> Vec<Row<Item>> {
    let label = |key: &str| tr(lang, key);
    // Keep the already-loaded rows on screen while a write is running. The
    // placeholder is only useful when there is nothing to show yet; replacing
    // loaded rows made the user's selections vanish until the snapshot returned.
    if self.busy() && !self.has_loaded_data(page) {
      return vec![Row::info(
        label("control_center.waiting_for_backend_authentication"),
        "",
      )];
    }
    match page {
      Page::Firewall => self.firewall_rows(lang),
      Page::Users => vec![
        Row::submenu(Item::CreateUser, label("control_center.create")).icon(icons::ADD),
        Row::submenu(Item::UserList, label("control_center.list")).icon(icons::USERS),
        Row::submenu(Item::SystemUsers, label("control_center.system_accounts"))
          .icon(icons::ACCOUNT_COG),
      ],
      Page::UserList | Page::SystemUsers => {
        let mut rows = vec![
          Row::action(Item::Reload, label("control_center.reload")).icon(icons::REFRESH),
          Row::separator(),
        ];
        rows.extend(
          self
            .users(page == Page::SystemUsers)
            .iter()
            .enumerate()
            .filter(|(_, user)| matches_search(search, &[&text(user, "user"), &text(user, "name")]))
            .map(|(index, user)| {
              Row::submenu(Item::UserEntry(index), text(user, "user")).detail(text(user, "name"))
            }),
        );
        rows
      }
      Page::CreateUser => {
        let mut rows = vec![
          Row::section(label("control_center.account")),
          Row::value(
            Item::Username,
            label("control_center.username"),
            text(&self.user, "user"),
            None,
          )
          .icon(icons::USER),
          Row::value(
            Item::FullName,
            label("control_center.full_name"),
            text(&self.user, "name"),
            None,
          )
          .icon(icons::ID_CARD),
          Row::submenu(Item::Shell, label("control_center.shell"))
            .icon(icons::TERMINAL)
            .detail(text(&self.user, "shell")),
          Row::submenu(
            Item::SupplementaryGroups,
            label("control_center.supplementary_groups"),
          )
          .icon(icons::GROUP)
          .detail(strings(&self.user["groups"]).join(", ")),
          self.password_row(lang, 1),
          self.password_row(lang, 2),
        ];
        rows.extend(self.draft_rows(
          Item::CreateAccount,
          self.create_account_label(lang),
          self.create_user_changed(),
          lang,
        ));
        rows
      }
      Page::User => self.user_rows(lang),
      Page::UserPassword => {
        let mut rows = vec![
          self.password_row(lang, 0),
          self.password_row(lang, 1),
          self.password_row(lang, 2),
        ];
        rows.extend(self.draft_rows(
          Item::SavePassword,
          label("control_center.save_password"),
          self.passwords_changed(),
          lang,
        ));
        rows
      }
      Page::UserShell | Page::UserPrimaryGroup => {
        let (source, field) = if page == Page::UserShell {
          ("shells", "shell")
        } else {
          ("groups", "primary_group")
        };
        let current = text(&self.user, field);
        strings(&self.accounts[source])
          .into_iter()
          .enumerate()
          .map(|(index, value)| {
            let item = if page == Page::UserShell {
              Item::ShellOption(index)
            } else {
              Item::PrimaryGroupOption(index)
            };
            let is_current = value == current;
            Row::choice(item, value, is_current)
          })
          .collect()
      }
      Page::UserGroups => {
        let primary = text(&self.user, "primary_group");
        let groups = strings(&self.user["groups"]);
        strings(&self.accounts["groups"])
          .into_iter()
          .enumerate()
          .map(|(index, group)| {
            let is_primary = group == primary;
            let on = is_primary || groups.contains(&group);
            // The primary group always stays a member; it cannot be removed here.
            Row::toggle(Item::GroupOption(index), group, on).enabled(!is_primary)
          })
          .collect()
      }
      Page::Groups => vec![
        Row::submenu(Item::CreateGroup, label("control_center.create")).icon(icons::ADD),
        Row::submenu(Item::GroupList, label("control_center.list")).icon(icons::GROUP),
      ],
      Page::GroupList | Page::SystemGroups => {
        let mut rows = vec![
          Row::action(Item::Reload, label("control_center.reload")).icon(icons::REFRESH),
          Row::separator(),
        ];
        rows.extend(
          self
            .groups(true)
            .iter()
            .enumerate()
            .map(|(index, group)| {
              let members = strings(&group["members"]);
              let detail = if members.is_empty() {
                format!("GID {}", group["gid"])
              } else {
                format!("GID {} | {}", group["gid"], members.join(", "))
              };
              (index, text(group, "name"), detail)
            })
            .filter(|(_, name, detail)| matches_search(search, &[name, detail]))
            .map(|(index, name, detail)| {
              Row::submenu(Item::GroupEntry(index), name).detail(detail)
            }),
        );
        rows
      }
      Page::CreateGroup => {
        let mut rows = vec![
          Row::section(label("control_center.group")),
          Row::value(
            Item::GroupName,
            label("control_center.group_name"),
            text(&self.group, "name"),
            None,
          )
          .icon(icons::EDIT),
        ];
        rows.extend(self.draft_rows(
          Item::SubmitGroup,
          label("control_center.create_group"),
          self.create_group_changed(),
          lang,
        ));
        rows
      }
      Page::Group => {
        let mut rows = vec![
          Row::section(label("control_center.group")),
          Row::value(
            Item::GroupName,
            label("control_center.name_8d6abd"),
            text(&self.group, "name"),
            None,
          )
          .icon(icons::EDIT),
          Row::info(label("control_center.gid"), self.group["gid"].to_string()),
          Row::submenu(Item::Members, label("control_center.members"))
            .icon(icons::USERS)
            .detail(strings(&self.group["members"]).join(", ")),
        ];
        rows.extend(self.draft_rows(
          Item::SaveGroup,
          label("control_center.save_changes"),
          self.group_draft_changed(),
          lang,
        ));
        rows.push(Row::section(label("control_center.danger_zone")));
        rows.push(
          Row::destructive(Item::DeleteGroup, label("control_center.delete_group"))
            .icon(icons::DELETE),
        );
        rows
      }
      Page::GroupMembers => {
        let members = strings(&self.group["members"]);
        self
          .users(true)
          .iter()
          .enumerate()
          .map(|(index, user)| {
            let name = text(user, "user");
            let on = members.contains(&name);
            Row::toggle(Item::Member(index), name, on)
          })
          .collect()
      }
      _ => Vec::new(),
    }
  }

  /// System > Users > (user): account fields and their Save row, then the
  /// password and avatar actions, with the deletions in the Danger zone.
  fn user_rows(&self, lang: Lang) -> Vec<Row<Item>> {
    let label = |key: &str| tr(lang, key);
    let mut rows = vec![
      Row::section(label("control_center.account")),
      Row::info(label("control_center.username"), text(&self.user, "user")),
      Row::info(
        label("control_center.uid_gid"),
        format!("{} / {}", self.user["uid"], self.user["gid"]),
      ),
      Row::info(
        label("control_center.home_directory"),
        text(&self.user, "home"),
      ),
      Row::value(
        Item::FullName,
        label("control_center.full_name"),
        text(&self.user, "name"),
        None,
      )
      .icon(icons::ID_CARD),
      Row::submenu(Item::Shell, label("control_center.shell"))
        .icon(icons::TERMINAL)
        .detail(text(&self.user, "shell")),
      Row::submenu(Item::PrimaryGroup, label("control_center.primary_group"))
        .icon(icons::ACCOUNT_STAR)
        .detail(text(&self.user, "primary_group")),
      Row::submenu(
        Item::SupplementaryGroups,
        label("control_center.supplementary_groups"),
      )
      .icon(icons::GROUP)
      .detail(strings(&self.user["groups"]).join(", ")),
    ];
    rows.extend(self.draft_rows(
      Item::SaveUser,
      label("control_center.save_changes"),
      self.user_draft_changed(),
      lang,
    ));
    rows.extend([
      Row::section(label("control_center.section_password")),
      Row::submenu(
        Item::ChangePassword,
        label("control_center.change_password"),
      )
      .icon(icons::KEY),
      Row::action(Item::LockPassword, label("control_center.lock_password")).icon(icons::LOCK),
      Row::action(
        Item::UnlockPassword,
        label("control_center.unlock_password"),
      )
      .icon(icons::LOCK_OPEN),
      Row::action(
        Item::ExpirePassword,
        label("control_center.require_password_change_at_login"),
      )
      .icon(icons::LOCK_RESET),
      Row::section(label("control_center.section_avatar")),
      Row::action(Item::AvatarImage, label("control_center.avatar_image")).icon(icons::AVATAR),
      Row::action(Item::RemoveAvatar, label("control_center.remove_avatar"))
        .icon(icons::IMAGE_REMOVE),
      Row::section(label("control_center.danger_zone")),
      Row::destructive(
        Item::DeleteUser,
        label("control_center.delete_user_keep_home"),
      )
      .icon(icons::ACCOUNT_REMOVE),
      Row::destructive(
        Item::DeleteUserAndHome,
        label("control_center.delete_user_and_home"),
      )
      .icon(icons::DELETE_FOREVER),
    ]);
    rows
  }

  /// Service state, the configuration draft with its Save and Cancel rows,
  /// and the rule actions.
  fn firewall_rows(&self, lang: Lang) -> Vec<Row<Item>> {
    let label = |key: &str| tr(lang, key);
    if self.firewall.is_null() {
      return vec![
        Row::action(Item::LoadFirewall, label("control_center.reload")).icon(icons::REFRESH),
      ];
    }
    let mut rows = vec![
      Row::section(label("control_center.service")),
      // Paired Start/Stop: one status row whose Enter switches the state.
      Row::action(Item::FirewallService, label("control_center.service"))
        .icon(icons::SHIELD)
        .detail(if self.firewall["active"] == true {
          label("control_center.active_095d39")
        } else {
          label("control_center.stopped_c2dfd3")
        }),
      Row::toggle(
        Item::FirewallBoot,
        label("control_center.start_at_boot"),
        self.firewall["enabled"] == true,
      )
      .icon(icons::AUTOSTART),
      Row::section(label("control_center.configuration")),
    ];
    rows.extend(
      FIREWALL_FIELDS
        .iter()
        .enumerate()
        .map(|(index, (key, field_label))| {
          let value = text(&self.config, key);
          let item = Item::FirewallField(index);
          match value.as_str() {
            "y" | "n" => Row::toggle(item, label(field_label), value == "y"),
            _ if *key == "PROTECTION_LEVEL" => Row::value(item, label(field_label), value, Some(1)),
            _ => Row::value(item, label(field_label), value, None),
          }
        }),
    );
    rows.extend(self.draft_rows(
      Item::SaveFirewall,
      label("control_center.save_configuration"),
      self.firewall_draft_changed(),
      lang,
    ));
    rows.extend([
      Row::action(
        Item::DiscardFirewall,
        label("control_center.discard_config_changes"),
      )
      .icon(icons::CANCEL),
      Row::section(label("control_center.section_rules")),
      Row::action(Item::AddRules, label("control_center.add_iptables_rules")).icon(icons::SCRIPT),
      Row::action(Item::ApplyRules, label("control_center.apply_saved_rules"))
        .icon(icons::SHIELD_REFRESH),
    ]);
    rows
  }

  /// A masked password field: 0 = current, 1 = new, 2 = confirmation.
  fn password_row(&self, lang: Lang, index: usize) -> Row<Item> {
    let key = match index {
      0 => "control_center.current_password_own_account",
      1 => "control_center.new_password",
      _ => "control_center.confirm_password",
    };
    Row::value(
      Item::Password(index),
      tr(lang, key),
      "*".repeat(argvus_tui::text::display_width(&self.passwords[index])),
      None,
    )
    .icon(if index == 0 { icons::LOCK } else { icons::KEY })
  }

  /// Label of the create-account row: the account stays password-locked
  /// unless a new password was typed.
  fn create_account_label(&self, lang: Lang) -> &'static str {
    if self.passwords[1].is_empty() {
      tr(lang, "control_center.create_account_password_locked")
    } else {
      tr(lang, "control_center.create_account_with_password")
    }
  }

  /// The Save/Create row that commits a draft: dimmed and skipped while
  /// nothing changed, marked "changed" otherwise.
  fn draft_rows(&self, item: Item, label: &str, changed: bool, lang: Lang) -> Vec<Row<Item>> {
    let mut rows = draft_actions(item, label, changed);
    if changed && let Some(row) = rows.pop() {
      rows.push(row.detail(tr(lang, "control_center.draft_changed")));
    }
    rows
  }

  /// The loaded account the user page edits.
  fn loaded_user(&self) -> Option<&Value> {
    self.accounts["users"]
      .as_array()?
      .iter()
      .find(|user| user["user"] == self.user["user"])
  }

  /// Whether the user page has unsaved changes.
  pub(crate) fn user_draft_changed(&self) -> bool {
    let Some(loaded) = self.loaded_user() else {
      return false;
    };
    let set = |value: &Value| strings(value).into_iter().collect::<BTreeSet<_>>();
    ["name", "shell", "primary_group"]
      .iter()
      .any(|field| text(loaded, field) != text(&self.user, field))
      || set(&loaded["groups"]) != set(&self.user["groups"])
  }

  /// Whether the create-user form has anything typed or chosen.
  pub(crate) fn create_user_changed(&self) -> bool {
    let default_shell = strings(&self.accounts["shells"])
      .first()
      .cloned()
      .unwrap_or_else(|| "/bin/bash".into());
    !text(&self.user, "user").is_empty()
      || !text(&self.user, "name").is_empty()
      || text(&self.user, "shell") != default_shell
      || !strings(&self.user["groups"]).is_empty()
      || self.passwords_changed()
  }

  /// Whether any password field has been typed.
  pub(crate) fn passwords_changed(&self) -> bool {
    self.passwords.iter().any(|password| !password.is_empty())
  }

  /// The loaded group the group page edits.
  fn loaded_group(&self) -> Option<&Value> {
    self.accounts["group_details"]
      .as_array()?
      .iter()
      .find(|group| group["name"] == self.group["original"])
  }

  /// Whether the group page has unsaved changes.
  pub(crate) fn group_draft_changed(&self) -> bool {
    let Some(loaded) = self.loaded_group() else {
      return false;
    };
    let set = |value: &Value| strings(value).into_iter().collect::<BTreeSet<_>>();
    text(loaded, "name") != text(&self.group, "name")
      || set(&loaded["members"]) != set(&self.group["members"])
  }

  /// Whether the create-group form has a name.
  pub(crate) fn create_group_changed(&self) -> bool {
    !text(&self.group, "name").is_empty()
  }

  /// Whether the firewall configuration differs from the saved one.
  pub(crate) fn firewall_draft_changed(&self) -> bool {
    !self.firewall.is_null() && self.config != self.firewall["config"]
  }

  /// Drops the unsaved changes of `page`, back to the loaded state.
  pub(crate) fn discard_draft(&mut self, page: Page) {
    match page {
      Page::User => {
        if let Some(loaded) = self.loaded_user().cloned() {
          self.user = loaded;
        }
      }
      Page::Group => {
        if let Some(loaded) = self.loaded_group().cloned() {
          let original = self.group["original"].clone();
          self.group = loaded;
          self.group["original"] = original;
        }
      }
      Page::Firewall => self.config = self.firewall["config"].clone(),
      Page::CreateGroup => self.group = json!({"name":""}),
      _ => {}
    }
    self.passwords = Default::default();
  }

  /// Whether leaving `page` would drop unsaved changes. Subpages that edit
  /// the same draft (shell, groups, members) keep it and do not ask.
  pub(crate) fn leaving_drops_draft(&self, page: Page) -> bool {
    match page {
      Page::User => self.user_draft_changed(),
      Page::CreateUser => self.create_user_changed(),
      Page::UserPassword => self.passwords_changed(),
      Page::Group => self.group_draft_changed(),
      Page::CreateGroup => self.create_group_changed(),
      Page::Firewall => self.firewall_draft_changed(),
      _ => false,
    }
  }

  /// Whether the new password is non-empty and matches its confirmation.
  pub(crate) fn new_password_is_valid(&self) -> bool {
    !self.passwords[1].is_empty() && self.passwords[1] == self.passwords[2]
  }
}

impl App {
  /// Executes the `admin_paste` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
  /// Executes the `admin_confirm` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn admin_confirm(&mut self, firewall: bool, value: Value, message: String) {
    self.admin.pending = Some((firewall, value));
    self.confirm = Some(PendingAction::Administration(message));
    self.confirm_apply_selected = false;
  }

  /// Runs the row `item` of an account or firewall page. Toggles, actions,
  /// submenus and destructive rows all land here; the effect and its timing
  /// (draft edit, confirmation, page change) are the same as before rows
  /// replaced the button bar.
  pub(crate) fn admin_activate(&mut self, item: Item) {
    if self.admin.busy() {
      return;
    }
    let page = self.page();
    let lang = self.lang;
    let user = text(&self.admin.user, "user");
    let confirm_user = |key: &str| format!("{}: {user}?", tr(lang, key));
    match item {
      // Firewall.
      Item::LoadFirewall | Item::DiscardFirewall => self.admin.load(true),
      Item::FirewallService | Item::FirewallBoot => {
        let value = if item == Item::FirewallService {
          json!({"action": if self.admin.firewall["active"] == true { "stop" } else { "start" }})
        } else {
          json!({"action": if self.admin.firewall["enabled"] == true { "disable" } else { "enable" }})
        };
        self.admin_confirm(
          true,
          value,
          tr(
            lang,
            "control_center.change_the_firewall_network_connectivity_may_be_interrupted_saving_doe",
          )
          .into(),
        );
      }
      Item::FirewallField(index) => {
        let Some((key, field_label)) = FIREWALL_FIELDS.get(index) else {
          return;
        };
        let value = text(&self.admin.config, key);
        if matches!(value.as_str(), "y" | "n") {
          self.admin.config[*key] = json!(if value == "y" { "n" } else { "y" });
        } else if *key == "PROTECTION_LEVEL" {
          self.cycle_protection_level(1);
        } else {
          self.admin.editor = Some(Editor::new(
            tr(lang, field_label).into(),
            value,
            EditTarget::Config((*key).into()),
            false,
          ));
        }
      }
      Item::SaveFirewall => {
        let value = json!({"action":"save-config", "original":self.admin.firewall["config_text"], "config":self.admin.config});
        self.admin_confirm(
          true,
          value,
          format!("{}?", tr(lang, "control_center.save_configuration")),
        );
      }
      Item::AddRules => {
        self.admin.editor = Some(Editor::new(
          tr(lang, "control_center.add_iptables_rules").into(),
          text(&self.admin.firewall, "rules"),
          EditTarget::Rules,
          true,
        ));
      }
      Item::ApplyRules => {
        self.admin_confirm(
          true,
          json!({"action":"restart"}),
          format!(
            "{}?\n{}",
            tr(lang, "control_center.apply_saved_rules"),
            tr(lang, "control_center.apply_saved_rules_description")
          ),
        );
      }

      // Users.
      Item::CreateUser => {
        self.admin.user = json!({"user":"", "name":"", "shell":strings(&self.admin.accounts["shells"]).first().cloned().unwrap_or("/bin/bash".into()), "groups":[]});
        self.navigation.push(Page::CreateUser);
      }
      Item::UserList => self.navigation.push(Page::UserList),
      Item::SystemUsers => self.navigation.push(Page::SystemUsers),
      Item::Reload => self.admin.load(false),
      Item::UserEntry(index) => {
        if let Some(user) = self.admin.users(page == Page::SystemUsers).get(index) {
          self.admin.user = user.clone();
          self.navigation.push(Page::User);
        }
      }
      Item::Username => {
        self.admin.editor = Some(Editor::new(
          tr(lang, "control_center.username").into(),
          user,
          EditTarget::User("user".into()),
          false,
        ));
      }
      Item::FullName => {
        self.admin.editor = Some(Editor::new(
          tr(lang, "control_center.full_name").into(),
          text(&self.admin.user, "name"),
          EditTarget::User("name".into()),
          false,
        ));
      }
      Item::Shell => self.navigation.push(Page::UserShell),
      Item::PrimaryGroup => self.navigation.push(Page::UserPrimaryGroup),
      Item::SupplementaryGroups => self.navigation.push(Page::UserGroups),
      Item::Password(index) => {
        let key = match index {
          0 => "control_center.current_password_own_account",
          1 => "control_center.new_password",
          _ => "control_center.confirm_password",
        };
        self.admin.editor = Some(Editor::new(
          tr(lang, key).into(),
          self.admin.passwords[index].clone(),
          EditTarget::Password(index),
          false,
        ));
      }
      Item::CreateAccount => {
        let title = self.admin.create_account_label(lang);
        let mut value = json!({"action":"create", "user":user, "name":self.admin.user["name"], "shell":self.admin.user["shell"], "groups":self.admin.user["groups"]});
        if self.admin.passwords[1].is_empty() && self.admin.passwords[2].is_empty() {
          self.admin_confirm(false, value, format!("{title}: {user}?"));
          return;
        }
        if !self.admin.new_password_is_valid() {
          self.reject_password();
          return;
        }
        value["password"] = self.admin.passwords[1].clone().into();
        self.admin.passwords = Default::default();
        self.admin_confirm(false, value, format!("{title}: {user}?"));
      }
      Item::SaveUser => {
        let value = json!({"action":"edit", "user":user, "name":self.admin.user["name"], "shell":self.admin.user["shell"], "groups":self.admin.user["groups"], "primary_group":self.admin.user["primary_group"]});
        self.admin_confirm(false, value, confirm_user("control_center.save_changes"));
      }
      Item::ChangePassword => {
        self.admin.passwords = Default::default();
        self.navigation.push(Page::UserPassword);
      }
      Item::LockPassword | Item::UnlockPassword => {
        let (locked, key) = if item == Item::LockPassword {
          (true, "control_center.lock_password")
        } else {
          (false, "control_center.unlock_password")
        };
        self.admin_confirm(
          false,
          json!({"action":"lock", "user":user, "locked":locked}),
          confirm_user(key),
        );
      }
      Item::ExpirePassword => {
        self.admin_confirm(
          false,
          json!({"action":"expire-password", "user":user}),
          confirm_user("control_center.require_password_change_at_login"),
        );
      }
      Item::AvatarImage => {
        self.admin.editor = Some(Editor::new(
          tr(lang, "control_center.avatar_image").into(),
          String::new(),
          EditTarget::Avatar,
          false,
        ));
      }
      Item::RemoveAvatar => {
        self.admin_confirm(
          false,
          json!({"action":"avatar", "user":user, "path":""}),
          confirm_user("control_center.remove_avatar"),
        );
      }
      Item::DeleteUser | Item::DeleteUserAndHome => {
        let (remove_home, key) = if item == Item::DeleteUserAndHome {
          (true, "control_center.delete_user_and_home")
        } else {
          (false, "control_center.delete_user_keep_home")
        };
        self.admin_confirm(
          false,
          json!({"action":"delete", "user":user, "remove_home":remove_home}),
          confirm_user(key),
        );
      }
      Item::SavePassword => {
        if !self.admin.new_password_is_valid() {
          self.reject_password();
          return;
        }
        let value = json!({"action":"password", "user":self.admin.user["user"], "old":self.admin.passwords[0], "new":self.admin.passwords[1], "confirm":self.admin.passwords[2]});
        self.admin.passwords = Default::default();
        self.admin_confirm(false, value, confirm_user("control_center.save_password"));
      }
      Item::ShellOption(index) | Item::PrimaryGroupOption(index) => {
        let (source, field) = if matches!(item, Item::ShellOption(_)) {
          ("shells", "shell")
        } else {
          ("groups", "primary_group")
        };
        if let Some(value) = strings(&self.admin.accounts[source]).get(index) {
          self.admin.user[field] = json!(value);
          self.navigation.back();
        }
      }
      Item::GroupOption(index) => {
        if let Some(group) = strings(&self.admin.accounts["groups"]).get(index) {
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

      // Groups.
      Item::CreateGroup => {
        self.admin.group = json!({"name":""});
        self.navigation.push(Page::CreateGroup);
      }
      Item::GroupList => self.navigation.push(Page::GroupList),
      Item::GroupEntry(index) => {
        if let Some(group) = self.admin.groups(true).get(index) {
          self.admin.group = group.clone();
          self.admin.group["original"] = self.admin.group["name"].clone();
          self.navigation.push(Page::Group);
        }
      }
      Item::GroupName => {
        let key = if page == Page::CreateGroup {
          "control_center.group_name"
        } else {
          "control_center.name_8d6abd"
        };
        self.admin.editor = Some(Editor::new(
          tr(lang, key).into(),
          text(&self.admin.group, "name"),
          EditTarget::Group,
          false,
        ));
      }
      Item::Members => self.navigation.push(Page::GroupMembers),
      Item::Member(index) => {
        if let Some(user) = self.admin.users(true).get(index)
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
      Item::SubmitGroup => {
        let group = text(&self.admin.group, "name");
        self.admin_confirm(
          false,
          json!({"action":"create-group", "group":group}),
          format!("{}: {group}?", tr(lang, "control_center.create_group")),
        );
      }
      Item::SaveGroup => {
        self.admin_confirm(
          false,
          json!({
            "action":"edit-group",
            "group": text(&self.admin.group, "original"),
            "name": text(&self.admin.group, "name"),
            "members": self.admin.group["members"]}),
          format!(
            "{}: {}?",
            tr(lang, "control_center.save_changes"),
            text(&self.admin.group, "original")
          ),
        );
      }
      Item::DeleteGroup => {
        let group = text(&self.admin.group, "original");
        self.admin_confirm(
          false,
          json!({"action":"delete-group", "group":group}),
          format!("{}: {group}?", tr(lang, "control_center.delete_group")),
        );
      }
      _ => {}
    }
  }

  /// `←/→` on a Value row with a step of an account or firewall page.
  pub(crate) fn admin_adjust(&mut self, item: Item, delta: i32) {
    if self.admin.busy() {
      return;
    }
    if let Item::FirewallField(index) = item
      && FIREWALL_FIELDS
        .get(index)
        .is_some_and(|(key, _)| *key == "PROTECTION_LEVEL")
    {
      self.cycle_protection_level(delta);
    }
  }

  /// Moves the firewall protection level by `delta` steps, wrapping around.
  fn cycle_protection_level(&mut self, delta: i32) {
    let levels = ["low", "medium", "high", "paranoid"];
    let value = text(&self.admin.config, "PROTECTION_LEVEL");
    let index = levels.iter().position(|level| *level == value).unwrap_or(0) as i32;
    let next = (index + delta.signum()).rem_euclid(levels.len() as i32) as usize;
    self.admin.config["PROTECTION_LEVEL"] = json!(levels[next]);
  }

  /// Rejects an empty or unconfirmed new password before any confirmation.
  fn reject_password(&mut self) {
    self.error_modal = Some(
      tr(
        self.lang,
        "control_center.the_new_password_must_be_nonempty_and_match_its_confirmation",
      )
      .into(),
    );
  }

  /// Executes the `admin_input` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
            "control_center.save_rules_fw_this_script_will_run_as_root_when_applying_the_firewall",
          )
          .into(),
        ),
        EditTarget::Group => self.admin.group["name"] = json!(value),
        EditTarget::FontSize => match value.trim().parse::<u16>() {
          Ok(size) if (8..=32).contains(&size) => self.pending_size = size,
          _ => self.fail(format!(
            "{}: 8–32",
            tr(self.lang, "control_center.font_size")
          )),
        },
        EditTarget::DateTime => match crate::system::time::set_local_time(&value) {
          Ok(()) => self.refresh_time(),
          Err(error) => self.fail(error),
        },
        EditTarget::Avatar => self.admin_confirm(
          false,
          json!({"action":"avatar", "user":self.admin.user["user"], "path":value}),
          tr(self.lang, "control_center.change_avatar").into(),
        ),
      }
    } else {
      editor.input(key);
      self.admin.editor = Some(editor);
    }
  }
}

#[derive(Debug, PartialEq, Clone)]
/// Defines `EditTarget`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub(crate) enum EditTarget {
  Config(String),
  User(String),
  Password(usize),
  Rules,
  Group,
  Avatar,
  DateTime,
  /// Size used when a font is applied on the font selector (8–32).
  FontSize,
}

/// Represents `Editor`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub struct Editor {
  title: String,
  value: Vec<char>,
  cursor: usize,
  target: EditTarget,
  multiline: bool,
}

impl Editor {
  /// Constructs `new` with this module's expected initial state. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

  /// Executes the `input` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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

  /// Renders `draw` while respecting the current domain state and semantic theme. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
      .title_bottom(tr(app.lang, "control_center.enter_ctrl_s_save_esc_cancel"))
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
  use argvus_tui::menu::RowKind;
  use crossterm::event::Event;

  /// Executes the `app` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn app(page: Page) -> App {
    let mut app = App::with_context(
      Page::Main,
      Lang::for_locale("en-US"),
      crate::theme::Theme::load(),
    );
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

  fn press(app: &mut App, code: KeyCode) {
    crate::event::handle(app, Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
  }

  /// Moves the cursor to `item` and asserts that it is selectable.
  fn select(app: &mut App, item: Item) {
    let rows = app.rows();
    assert!(
      app.navigation.current_mut().menu.select(&rows, &item),
      "{item:?} is not selectable"
    );
  }

  fn items(app: &App) -> Vec<Option<Item>> {
    app.rows().iter().map(|row| row.id().copied()).collect()
  }

  #[test]
  /// Executes the `every_firewall_field_is_editable_without_writing` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn every_firewall_field_is_editable_without_writing() {
    let mut app = app(Page::Firewall);
    app.admin.firewall = json!({"active":false,"enabled":false});
    app.admin.config = json!({});
    for (index, (key, _)) in FIREWALL_FIELDS.iter().enumerate() {
      app.admin.config[*key] = json!(if *key == "PROTECTION_LEVEL" {
        "high"
      } else {
        ""
      });
      app.admin_activate(Item::FirewallField(index));
      if *key == "PROTECTION_LEVEL" {
        assert_eq!(app.admin.config[*key], "paranoid");
      } else {
        assert!(app.admin.editor.take().is_some());
      }
    }
    app.admin.config["ALLOW_SSH"] = json!("n");
    let ssh = FIREWALL_FIELDS
      .iter()
      .position(|(key, _)| *key == "ALLOW_SSH")
      .unwrap();
    app.admin_activate(Item::FirewallField(ssh));
    assert_eq!(app.admin.config["ALLOW_SSH"], "y");
    assert!(!app.admin.busy());
  }

  #[test]
  fn firewall_rows_are_toggles_values_and_a_protection_level_step() {
    let mut app = app(Page::Firewall);
    app.admin.firewall = json!({"active":true,"enabled":true});
    app.admin.config = json!({"ALLOW_SSH":"y","PROTECTION_LEVEL":"low","SSH_PORT":"22"});
    let rows = app.rows();
    let kind = |key: &str| {
      let index = FIREWALL_FIELDS.iter().position(|(k, _)| *k == key).unwrap();
      rows
        .iter()
        .find(|row| row.id() == Some(&Item::FirewallField(index)))
        .map(|row| row.kind())
        .unwrap()
    };
    assert_eq!(kind("ALLOW_SSH"), RowKind::Toggle { on: true });
    assert_eq!(kind("PROTECTION_LEVEL"), RowKind::Value { step: Some(1) });
    assert_eq!(kind("SSH_PORT"), RowKind::Value { step: None });
    let level = FIREWALL_FIELDS
      .iter()
      .position(|(key, _)| *key == "PROTECTION_LEVEL")
      .unwrap();
    select(&mut app, Item::FirewallField(level));
    press(&mut app, KeyCode::Left);
    assert_eq!(
      app.admin.config["PROTECTION_LEVEL"], "paranoid",
      "wraps around"
    );
    press(&mut app, KeyCode::Right);
    assert_eq!(app.admin.config["PROTECTION_LEVEL"], "low");
    assert_eq!(
      app.page(),
      Page::Firewall,
      "← adjusts instead of going back"
    );
  }

  #[test]
  /// Executes the `user_group_selection_preserves_primary_group` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn user_group_selection_preserves_primary_group() {
    let mut app = app(Page::UserGroups);
    let rows = app.rows();
    assert!(!rows[0].is_selectable(), "primary group row is disabled");
    assert_eq!(rows[0].kind(), RowKind::Toggle { on: true });
    app.admin_activate(Item::GroupOption(0));
    assert_eq!(strings(&app.admin.user["groups"]), vec!["wheel"]);
    app.normalize_selection();
    select(&mut app, Item::GroupOption(2));
    press(&mut app, KeyCode::Enter);
    assert_eq!(strings(&app.admin.user["groups"]), vec!["wheel", "audio"]);
    press(&mut app, KeyCode::Char(' '));
    assert_eq!(strings(&app.admin.user["groups"]), vec!["wheel"]);
  }

  #[test]
  /// Executes the `destructive_actions_default_to_cancel_and_never_run_on_selection` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn destructive_actions_default_to_cancel_and_never_run_on_selection() {
    let mut app = app(Page::User);
    select(&mut app, Item::DeleteUserAndHome);
    assert!(app.confirm.is_none(), "selecting does not run anything");
    press(&mut app, KeyCode::Enter);
    assert!(app.confirm.is_some());
    assert!(!app.confirm_apply_selected);
    assert!(!app.admin.busy());
    app.confirm_accept();
    assert!(app.admin.pending.is_none());
    assert!(!app.admin.busy());
  }

  #[test]
  /// Executes the `password_mismatch_stays_local_and_passwords_are_masked` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn password_mismatch_stays_local_and_passwords_are_masked() {
    let mut app = app(Page::UserPassword);
    app.admin.passwords = ["old secret".into(), "new secret".into(), "different".into()];
    app.admin_activate(Item::SavePassword);
    assert!(app.error_modal.is_some());
    assert!(app.admin.pending.is_none());
    for row in app.rows() {
      assert!(!row.detail_text().unwrap_or_default().contains("secret"));
    }
  }

  #[test]
  /// Executes the `multiline_editor_moves_between_lines_and_accepts_paste` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
  /// Executes the `system_users_are_optional_and_own_user_is_visible` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn system_users_are_optional_and_own_user_is_visible() {
    let app = app(Page::Users);
    assert_eq!(app.admin.users(false).len(), 1);
    assert_eq!(app.admin.users(true).len(), 2);
  }

  #[test]
  /// Executes the `users_overview_is_separate_from_account_list` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn users_overview_is_separate_from_account_list() {
    let mut app = app(Page::Users);
    let labels = app
      .rows()
      .iter()
      .map(|row| row.label().to_string())
      .collect::<Vec<_>>();
    assert_eq!(labels, vec!["Create", "List", "System accounts"]);
    select(&mut app, Item::UserList);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.page(), Page::UserList);
    assert_eq!(
      items(&app),
      vec![Some(Item::Reload), None, Some(Item::UserEntry(0))]
    );
    assert_eq!(app.rows()[2].label(), "alice");
    assert_eq!(app.selected_item(), Some(Item::Reload));
    press(&mut app, KeyCode::Enter);
    assert!(app.admin.busy(), "Reload row reloads the accounts");
  }

  #[test]
  fn r_reloads_the_account_lists() {
    let mut app = app(Page::SystemUsers);
    press(&mut app, KeyCode::Char('r'));
    assert!(app.admin.busy());
  }

  #[test]
  fn user_search_keeps_the_original_index() {
    let mut app = app(Page::SystemUsers);
    app.search = "ali".into();
    assert_eq!(
      items(&app),
      vec![Some(Item::Reload), None, Some(Item::UserEntry(1))]
    );
    app.admin_activate(Item::UserEntry(1));
    assert_eq!(app.page(), Page::User);
    assert_eq!(text(&app.admin.user, "user"), "alice");
  }

  #[test]
  /// Executes the `create_user_screen_has_masked_password_rows_and_routes_to_editor` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn create_user_screen_has_masked_password_rows_and_routes_to_editor() {
    let mut app = app(Page::CreateUser);
    let rows = app.rows();
    let labels = rows.iter().map(|row| row.label()).collect::<Vec<_>>();
    assert_eq!(
      labels,
      vec![
        "Account",
        "Username",
        "Full name",
        tr(app.lang, "control_center.shell"),
        "Supplementary groups",
        "New password",
        "Confirm password",
        "",
        tr(app.lang, "control_center.create_account_password_locked"),
      ]
    );
    assert!(rows[0].is_section());
    assert!(rows[5].is_selectable() && rows[6].is_selectable());
    assert_eq!(rows[7].kind(), RowKind::Separator);
    assert_eq!(app.selected_item(), Some(Item::Username));
    select(&mut app, Item::Password(1));
    press(&mut app, KeyCode::Enter);
    let editor = app.admin.editor.take().expect("password row opens editor");
    assert_eq!(editor.target, EditTarget::Password(1));
    assert!(editor.value.is_empty());
  }

  #[test]
  /// Executes the `create_user_button_is_dynamic_and_validates_passwords` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn create_user_button_is_dynamic_and_validates_passwords() {
    let mut app = app(Page::CreateUser);
    let pt = Lang::for_locale("pt-BR");
    let create_label = |app: &App| {
      app
        .admin
        .rows(Page::CreateUser, pt, "")
        .iter()
        .find(|row| row.id() == Some(&Item::CreateAccount))
        .map(|row| row.label().to_string())
        .unwrap()
    };
    assert_eq!(create_label(&app), "Criar conta (senha bloqueada)");
    app.admin.passwords[1] = "segredo".into();
    app.admin.passwords[2] = "segredo".into();
    assert_eq!(create_label(&app), "Criar conta com senha");
    app.admin_activate(Item::CreateAccount);
    assert!(
      app.admin.pending.is_some(),
      "valid passwords must reach the confirm flow"
    );
    let payload = app.admin.pending.as_ref().unwrap().1.clone();
    assert_eq!(payload["password"].as_str(), Some("segredo"));
    app.admin.pending = None;

    app.admin.passwords[1] = "a".into();
    app.admin.passwords[2] = "b".into();
    app.admin_activate(Item::CreateAccount);
    assert!(
      app.error_modal.is_some(),
      "mismatched passwords must be rejected"
    );
    assert!(app.admin.pending.is_none());
    app.error_modal = None;

    app.admin.passwords[1].clear();
    app.admin.passwords[2].clear();
    app.admin_activate(Item::CreateAccount);
    assert!(
      app.admin.pending.is_some(),
      "empty passwords keep the locked-account confirm flow"
    );
  }

  #[test]
  /// The user page follows the approved layout: titled sections, read-only
  /// identity as Info, the draft fields and their Save row, the password and
  /// avatar actions, and the deletions in the Danger zone at the end.
  fn user_page_layout_has_sections_info_draft_and_danger_zone() {
    let mut app = app(Page::User);
    app.admin.user["name"] = json!("Alice Liddell");
    let rows = app.rows();
    let sections: Vec<&str> = rows
      .iter()
      .filter(|row| row.is_section())
      .map(|row| row.label())
      .collect();
    assert_eq!(
      sections,
      vec![
        "Account",
        tr(app.lang, "control_center.section_password"),
        tr(app.lang, "control_center.section_avatar"),
        tr(app.lang, "control_center.danger_zone"),
      ]
    );
    assert!(rows.iter().all(|row| !row.label().starts_with("--")));
    let infos: Vec<&str> = rows
      .iter()
      .filter(|row| row.kind() == RowKind::Info)
      .map(|row| row.label())
      .collect();
    assert_eq!(
      infos,
      vec![
        "Username",
        tr(app.lang, "control_center.uid_gid"),
        tr(app.lang, "control_center.home_directory"),
      ]
    );
    let selectable: Vec<Item> = rows
      .iter()
      .filter(|row| row.is_selectable())
      .filter_map(|row| row.id().copied())
      .collect();
    assert_eq!(
      selectable,
      vec![
        Item::FullName,
        Item::Shell,
        Item::PrimaryGroup,
        Item::SupplementaryGroups,
        Item::SaveUser,
        Item::ChangePassword,
        Item::LockPassword,
        Item::UnlockPassword,
        Item::ExpirePassword,
        Item::AvatarImage,
        Item::RemoveAvatar,
        Item::DeleteUser,
        Item::DeleteUserAndHome,
      ]
    );
    let kind = |item: Item| {
      rows
        .iter()
        .find(|row| row.id() == Some(&item))
        .map(|row| row.kind())
        .unwrap()
    };
    assert_eq!(kind(Item::FullName), RowKind::Value { step: None });
    assert_eq!(kind(Item::Shell), RowKind::Submenu);
    assert_eq!(kind(Item::RemoveAvatar), RowKind::Action);
    assert_eq!(kind(Item::DeleteUser), RowKind::Destructive);
    assert_eq!(kind(Item::DeleteUserAndHome), RowKind::Destructive);
    let save = rows
      .iter()
      .position(|row| row.id() == Some(&Item::SaveUser))
      .unwrap();
    assert_eq!(rows[save - 1].kind(), RowKind::Separator);
    assert_eq!(
      rows[save - 2].id(),
      Some(&Item::SupplementaryGroups),
      "Save follows the editable fields"
    );
  }

  #[test]
  /// Retrieves data for `readonly_user_and_group_rows_are_not_selectable` without mixing collection with TUI rendering. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn readonly_user_and_group_rows_are_not_selectable() {
    let mut user_app = app(Page::User);
    assert_eq!(user_app.selected_item(), Some(Item::FullName));
    press(&mut user_app, KeyCode::Up);
    assert_eq!(
      user_app.selected_item(),
      Some(Item::FullName),
      "Info rows above are skipped"
    );

    let group_app = app(Page::Group);
    let rows = group_app.rows();
    assert!(!rows[0].is_selectable(), "section title");
    assert_eq!(rows[1].id(), Some(&Item::GroupName));
    assert_eq!(rows[2].kind(), RowKind::Info, "GID");
  }

  #[test]
  fn group_page_has_save_after_fields_and_delete_in_the_danger_zone() {
    let app = app(Page::Group);
    let rows = app.admin.rows(Page::Group, Lang::for_locale("pt-BR"), "");
    let ids: Vec<Option<Item>> = rows.iter().map(|row| row.id().copied()).collect();
    assert_eq!(
      ids,
      vec![
        None,
        Some(Item::GroupName),
        None,
        Some(Item::Members),
        None,
        Some(Item::SaveGroup),
        None,
        Some(Item::DeleteGroup),
      ]
    );
    assert_eq!(rows[5].label(), "Salvar alterações");
    assert_eq!(rows[7].kind(), RowKind::Destructive);
    assert!(rows[6].is_section());
  }

  #[test]
  /// Executes the `group_members_picker_toggles_users_in_list` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
    assert_eq!(app.rows()[0].kind(), RowKind::Toggle { on: true });
    select(&mut app, Item::Member(1));
    press(&mut app, KeyCode::Enter);
    assert_eq!(strings(&app.admin.group["members"]), vec!["alice", "bob"]);
    press(&mut app, KeyCode::Enter);
    assert_eq!(strings(&app.admin.group["members"]), vec!["alice"]);
  }

  #[test]
  /// Executes the `group_open_keeps_original_name_for_rename_and_delete` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn group_open_keeps_original_name_for_rename_and_delete() {
    let mut app = app(Page::GroupList);
    app.admin.accounts = json!({
      "users": [],
      "groups": [{"name":"dev","gid":1000,"members":["alice"]}],
      "group_details": [{"name":"dev","gid":1000,"members":["alice"]}]
    });
    select(&mut app, Item::GroupEntry(0));
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.page(), Page::Group);
    assert_eq!(text(&app.admin.group, "original"), "dev");
  }

  #[test]
  fn firewall_actions_are_rows_in_their_sections() {
    let mut app = app(Page::Firewall);
    app.admin.firewall = json!({"active":false,"enabled":false});
    app.admin.config = json!({});
    let rows = app
      .admin
      .rows(Page::Firewall, Lang::for_locale("pt-BR"), "");
    let sections = rows.iter().filter(|row| row.is_section()).count();
    assert_eq!(sections, 3, "Service, Configuration and Rules");
    let position = |item: Item| rows.iter().position(|row| row.id() == Some(&item)).unwrap();
    assert!(
      position(Item::FirewallField(FIREWALL_FIELDS.len() - 1)) < position(Item::SaveFirewall)
    );
    assert_eq!(
      position(Item::SaveFirewall) + 1,
      position(Item::DiscardFirewall)
    );
    assert!(position(Item::DiscardFirewall) < position(Item::AddRules));
    assert_eq!(position(Item::AddRules) + 1, position(Item::ApplyRules));
    assert_eq!(
      rows[position(Item::SaveFirewall)].label(),
      "Salvar Configuração"
    );
    assert_eq!(
      rows[position(Item::FirewallBoot)].kind(),
      RowKind::Toggle { on: false }
    );
    assert!(rows.iter().all(|row| !row.label().contains("[ ")));
  }

  #[test]
  /// Executes the `firewall_buttons_route_to_confirm_editor_reload_and_cancel_like_before` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn firewall_rows_route_to_confirm_editor_reload_and_cancel_like_before() {
    let mut app = app(Page::Firewall);
    app.admin.firewall = json!({
      "active":false,
      "enabled":false,
      "config_text":"",
      "rules":"iptables -A INPUT -j DROP"
    });
    app.admin.config = json!({"ALLOW_SSH":"n"});
    app.admin_activate(Item::SaveFirewall);
    assert!(app.admin.pending.is_some());
    app.cancel_modal();
    app.admin_activate(Item::AddRules);
    assert_eq!(app.admin.editor.take().unwrap().target, EditTarget::Rules);
    app.admin_activate(Item::ApplyRules);
    assert!(app.admin.pending.is_some());
    let Some(PendingAction::Administration(message)) = app.confirm.clone() else {
      panic!("Apply saved rules must ask first");
    };
    assert!(
      message.contains(tr(app.lang, "control_center.apply_saved_rules_description")),
      "the confirmation warns that the firewall restarts: {message}"
    );
    app.cancel_modal();
    app.admin_activate(Item::FirewallService);
    assert!(
      app.admin.pending.is_some(),
      "service state change is confirmed"
    );
    app.cancel_modal();
    app.admin_activate(Item::DiscardFirewall);
    assert!(
      app.admin.busy(),
      "Cancel changes reloads the saved configuration"
    );
  }

  #[test]
  /// Applies the `save_and_delete_buttons_confirm_without_running` operation while preserving the persistence and local-update contract. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn save_and_delete_rows_confirm_without_running() {
    for item in [
      Item::SaveUser,
      Item::LockPassword,
      Item::UnlockPassword,
      Item::ExpirePassword,
      Item::RemoveAvatar,
      Item::DeleteUser,
      Item::DeleteUserAndHome,
    ] {
      let mut app = app(Page::User);
      app.admin.user["name"] = json!("Alice Liddell");
      select(&mut app, item);
      press(&mut app, KeyCode::Enter);
      assert!(app.confirm.is_some(), "{item:?} asks first");
      assert!(!app.admin.busy());
    }
  }

  #[test]
  fn arrows_walk_the_single_list_and_tab_does_nothing() {
    let mut app = app(Page::User);
    assert_eq!(app.selected_item(), Some(Item::FullName));
    for expected in [
      Item::Shell,
      Item::PrimaryGroup,
      Item::SupplementaryGroups,
      Item::ChangePassword,
    ] {
      // Save changes is dimmed and skipped while the draft is clean.
      press(&mut app, KeyCode::Down);
      assert_eq!(app.selected_item(), Some(expected));
    }
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::BackTab);
    assert_eq!(app.selected_item(), Some(Item::ChangePassword));
    press(&mut app, KeyCode::End);
    assert_eq!(app.selected_item(), Some(Item::DeleteUserAndHome));
    press(&mut app, KeyCode::Home);
    assert_eq!(app.selected_item(), Some(Item::FullName));
  }

  #[test]
  fn save_changes_is_dimmed_until_the_user_draft_changes() {
    let mut app = app(Page::User);
    let save = |app: &App| {
      app
        .rows()
        .into_iter()
        .find(|row| row.id() == Some(&Item::SaveUser))
        .unwrap()
    };
    assert!(!save(&app).is_selectable(), "nothing to save yet");
    assert_eq!(save(&app).detail_text(), None);
    app.admin.user["groups"] = json!(["wheel"]);
    assert!(!app.admin.user_draft_changed(), "same groups");
    app.admin.user["name"] = json!("Alice Liddell");
    assert!(save(&app).is_selectable());
    assert_eq!(
      save(&app).detail_text(),
      Some(tr(app.lang, "control_center.draft_changed"))
    );
  }

  #[test]
  fn esc_with_a_changed_user_draft_asks_and_discard_restores_it() {
    let mut app = app(Page::UserList);
    app.admin_activate(Item::UserEntry(0));
    assert_eq!(app.page(), Page::User);
    press(&mut app, KeyCode::Esc);
    assert_eq!(
      app.page(),
      Page::UserList,
      "clean draft leaves without asking"
    );

    app.admin_activate(Item::UserEntry(0));
    app.admin.user["name"] = json!("Changed");
    press(&mut app, KeyCode::Esc);
    assert!(matches!(app.confirm, Some(PendingAction::DiscardDraft)));
    assert!(!app.confirm_apply_selected, "focus starts on Cancel");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
      app.page(),
      Page::User,
      "Cancel keeps the draft and the page"
    );
    assert_eq!(text(&app.admin.user, "name"), "Changed");

    press(&mut app, KeyCode::Esc);
    app.confirm_apply_selected = true;
    app.confirm_accept();
    assert_eq!(app.page(), Page::UserList);
    assert_eq!(text(&app.admin.user, "name"), "Alice");
  }

  #[test]
  fn subpages_of_the_same_draft_do_not_ask() {
    let mut app = app(Page::User);
    app.admin_activate(Item::SupplementaryGroups);
    app.admin_activate(Item::GroupOption(2));
    assert!(app.admin.user_draft_changed());
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.page(), Page::User);
    assert!(app.confirm.is_none());
    app.admin_activate(Item::Shell);
    app.admin_activate(Item::ShellOption(1));
    assert_eq!(app.page(), Page::User, "choosing a shell goes back");
    assert_eq!(text(&app.admin.user, "shell"), "/bin/zsh");
  }

  #[test]
  fn reload_keeps_a_changed_user_draft() {
    let mut app = app(Page::User);
    app.admin.user["name"] = json!("Changed");
    let (sender, receiver) = mpsc::channel();
    app.admin.operation = "snapshot".into();
    app.admin.worker = Some(receiver);
    sender
      .send(Ok((false, app.admin.accounts.clone())))
      .unwrap();
    assert!(app.admin.poll().is_some_and(|result| result.is_ok()));
    assert_eq!(text(&app.admin.user, "name"), "Changed");

    app.admin.user["name"] = json!("Alice");
    let mut accounts = app.admin.accounts.clone();
    accounts["users"][1]["shell"] = json!("/bin/zsh");
    let (sender, receiver) = mpsc::channel();
    app.admin.worker = Some(receiver);
    sender.send(Ok((false, accounts))).unwrap();
    app.admin.poll();
    assert_eq!(
      text(&app.admin.user, "shell"),
      "/bin/zsh",
      "a clean draft follows the reload"
    );
  }

  #[test]
  fn group_firewall_create_and_password_drafts_ask_before_leaving() {
    let mut group = app(Page::GroupList);
    group.admin_activate(Item::GroupEntry(0));
    group.admin_activate(Item::Members);
    group.admin_activate(Item::Member(0));
    press(&mut group, KeyCode::Esc);
    assert_eq!(group.page(), Page::Group, "members share the group draft");
    assert!(group.confirm.is_none());
    press(&mut group, KeyCode::Esc);
    assert!(matches!(group.confirm, Some(PendingAction::DiscardDraft)));

    let mut firewall = app(Page::Firewall);
    firewall.admin.firewall = json!({"active":true,"enabled":true,"config":{"ALLOW_SSH":"y"}});
    firewall.admin.config = json!({"ALLOW_SSH":"y"});
    let save = |app: &App| {
      app
        .rows()
        .into_iter()
        .find(|row| row.id() == Some(&Item::SaveFirewall))
        .unwrap()
        .is_selectable()
    };
    assert!(!save(&firewall));
    let ssh = FIREWALL_FIELDS
      .iter()
      .position(|(key, _)| *key == "ALLOW_SSH")
      .unwrap();
    firewall.admin_activate(Item::FirewallField(ssh));
    assert!(save(&firewall));
    press(&mut firewall, KeyCode::Esc);
    assert!(matches!(
      firewall.confirm,
      Some(PendingAction::DiscardDraft)
    ));
    firewall.confirm_apply_selected = true;
    firewall.confirm_accept();
    assert_eq!(firewall.admin.config["ALLOW_SSH"], "y", "discard restores");

    let mut create = app(Page::Users);
    create.admin_activate(Item::CreateUser);
    let create_row = |app: &App| {
      app
        .rows()
        .into_iter()
        .find(|row| row.id() == Some(&Item::CreateAccount))
        .unwrap()
        .is_selectable()
    };
    assert!(!create_row(&create), "empty form");
    press(&mut create, KeyCode::Esc);
    assert_eq!(create.page(), Page::Users);
    create.admin_activate(Item::CreateUser);
    create.admin.user["user"] = json!("bob");
    assert!(create_row(&create));
    press(&mut create, KeyCode::Esc);
    assert!(matches!(create.confirm, Some(PendingAction::DiscardDraft)));

    let mut password = app(Page::User);
    password.admin_activate(Item::ChangePassword);
    password.admin.passwords[1] = "secret".into();
    press(&mut password, KeyCode::Esc);
    assert!(matches!(
      password.confirm,
      Some(PendingAction::DiscardDraft)
    ));
    password.confirm_apply_selected = true;
    password.confirm_accept();
    assert_eq!(password.page(), Page::User);
    assert!(!password.admin.passwords_changed());
  }

  #[test]
  /// Executes the `pages_and_password_editor_render_at_minimum_and_large_sizes` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
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
        let rendered: String = terminal
          .backend()
          .buffer()
          .content
          .iter()
          .map(|cell| cell.symbol())
          .collect();
        assert!(rendered.chars().any(|cell| cell != ' '));
        assert!(
          !rendered.replace("[ ]", "").contains("[ "),
          "{page:?} has no button labels"
        );
        if page == Page::User && height == 40 {
          assert!(rendered.contains("Delete user (keep home)"));
          assert!(!rendered.contains("--"));
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
