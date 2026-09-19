//! Implements page navigation and selection rules in crate `argvus control center settings`. This separation keeps external effects from contaminating models, routes, or rendering.
//!
//! External tool dependencies remain in backend layers;
//! the UI consumes normalized models and results.
use argvus_control_center_apps::catalog::Category;

use crate::config::fonts::{FontTarget, SettingKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Defines `Page`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub enum Page {
  Main,
  DefaultApps,
  AppSelector(Category),
  Fonts,
  FontSelector(FontTarget),
  SettingSelector(SettingKind),
  LocaleRegion,
  TimeZone,
  DateTime,
  RegionalLocale,
  SystemLocales,
  Keyboard,
  Keybindings,
  KeybindingEdit,
  KeybindingCapture,
  MouseTouchpad,
  KeyboardLayout,
  KeyboardVariant,
  ConsoleKeymap,
  Language,
  System,
  Hostname,
  Firewall,
  Users,
  UserList,
  SystemUsers,
  User,
  CreateUser,
  UserGroups,
  UserPassword,
  UserShell,
  UserPrimaryGroup,
  Groups,
  GroupList,
  SystemGroups,
  Group,
  GroupMembers,
  CreateGroup,
}

#[derive(Debug, Clone, Copy)]
/// Represents `Location`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub struct Location {
  pub page: Page,
  pub selected: usize,
  pub scroll: usize,
}

impl Location {
  /// Executes the const function documented in this module. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
  pub const fn new(page: Page) -> Self {
    Self {
      page,
      selected: 0,
      scroll: 0,
    }
  }
}

#[derive(Debug)]
/// Represents `Navigation`. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
pub struct Navigation {
  current: Location,
  stack: Vec<Location>,
}

impl Navigation {
  /// Constructs `new` with this module's expected initial state. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn new(page: Page) -> Self {
    Self {
      current: Location::new(page),
      stack: if page == Page::Main {
        Vec::new()
      } else {
        vec![Location::new(Page::Main)]
      },
    }
  }

  /// Executes the const function documented in this module. Its explicit shape preserves the contract consumed by the rest of the workspace and keeps the intent visible as the module evolves.
  pub const fn current(&self) -> Location {
    self.current
  }

  /// Executes the `current_mut` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn current_mut(&mut self) -> &mut Location {
    &mut self.current
  }

  /// Executes the `push` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn push(&mut self, page: Page) {
    self.stack.push(self.current);
    self.current = Location::new(page);
  }

  /// Executes the `back` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  pub fn back(&mut self) -> bool {
    if let Some(previous) = self.stack.pop() {
      self.current = previous;
      true
    } else {
      false
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  /// Executes the `stack_restores_page_selection_and_scroll` step in this module. The behavior is encapsulated here so callers depend on a clear domain decision instead of duplicating system or UI details.
  fn stack_restores_page_selection_and_scroll() {
    let mut navigation = Navigation::new(Page::Main);
    navigation.current_mut().selected = 1;
    navigation.current_mut().scroll = 3;
    navigation.push(Page::Fonts);
    assert_eq!(navigation.current().page, Page::Fonts);
    assert!(navigation.back());
    assert_eq!(navigation.current().page, Page::Main);
    assert_eq!(navigation.current().selected, 1);
    assert_eq!(navigation.current().scroll, 3);
    assert!(!navigation.back());
  }
}
