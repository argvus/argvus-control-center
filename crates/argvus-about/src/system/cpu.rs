pub fn cpu_info(input: &str) -> Option<String> {
  let model = input
    .lines()
    .find_map(|line| {
      line
        .split_once(':')
        .filter(|(key, _)| key.trim() == "model name")
    })
    .map(|(_, value)| value.trim().to_string())?;
  let count = input
    .lines()
    .filter(|line| line.starts_with("processor"))
    .count();
  Some(if count > 1 {
    format!("{model} x {count}")
  } else {
    model
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_cpu_model_and_count() {
    let cpu = "processor\t: 0\nmodel name\t: Example CPU\nprocessor\t: 1\n";
    assert_eq!(cpu_info(cpu).as_deref(), Some("Example CPU x 2"));
  }

  #[test]
  fn handles_single_core() {
    let cpu = "model name\t: Example CPU\n";
    assert_eq!(cpu_info(cpu).as_deref(), Some("Example CPU"));
  }

  #[test]
  fn returns_none_without_model() {
    assert_eq!(cpu_info("processor\t: 0\n"), None);
  }
}
