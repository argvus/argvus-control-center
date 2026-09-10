use argvus_control_center_apps::catalog::Category;

use crate::config::fonts::{FontTarget, SettingKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
  Encoding,
  Keyboard,
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
pub struct Location {
  pub page: Page,
  pub selected: usize,
  pub scroll: usize,
}

impl Location {
  pub const fn new(page: Page) -> Self {
    Self {
      page,
      selected: 0,
      scroll: 0,
    }
  }
}

#[derive(Debug)]
pub struct Navigation {
  current: Location,
  stack: Vec<Location>,
}

impl Navigation {
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

  pub const fn current(&self) -> Location {
    self.current
  }

  pub fn current_mut(&mut self) -> &mut Location {
    &mut self.current
  }

  pub fn push(&mut self, page: Page) {
    self.stack.push(self.current);
    self.current = Location::new(page);
  }

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
