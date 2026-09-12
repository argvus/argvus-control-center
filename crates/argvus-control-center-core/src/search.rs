#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchEntry {
  pub id: String,
  pub category: String,
  pub title: String,
  pub keywords: Vec<String>,
  pub route: String,
}

#[derive(Debug, Default, Clone)]
pub struct SearchRegistry {
  entries: Vec<SearchEntry>,
}

impl SearchRegistry {
  pub fn register(&mut self, entry: SearchEntry) -> Result<(), String> {
    if entry.id.trim().is_empty() || self.entries.iter().any(|current| current.id == entry.id) {
      return Err("search entry id must be unique and non-empty".into());
    }
    self.entries.push(entry);
    Ok(())
  }
  pub fn entries(&self) -> &[SearchEntry] {
    &self.entries
  }
  pub fn search(&self, query: &str) -> Vec<&SearchEntry> {
    let query = query.trim().to_ascii_lowercase();
    self
      .entries
      .iter()
      .filter(|entry| {
        query.is_empty()
          || [
            entry.id.as_str(),
            entry.category.as_str(),
            entry.title.as_str(),
          ]
          .into_iter()
          .chain(entry.keywords.iter().map(String::as_str))
          .any(|value| value.to_ascii_lowercase().contains(&query))
      })
      .collect()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn finds_by_keyword_and_rejects_duplicate_ids() {
    let mut registry = SearchRegistry::default();
    let entry = SearchEntry {
      id: "network.wifi".into(),
      category: "network".into(),
      title: "Wi-Fi".into(),
      keywords: vec!["wireless".into()],
      route: "network/wifi".into(),
    };
    registry.register(entry.clone()).unwrap();
    assert_eq!(registry.search("wireless").len(), 1);
    assert!(registry.register(entry).is_err());
  }
}
