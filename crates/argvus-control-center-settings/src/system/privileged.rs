use std::process::{Command, Stdio};

use crate::error::SettingsError;

pub fn run(args: &[&str]) -> Result<String, SettingsError> {
  let output = Command::new("argvus-control-center")
    .arg("system-settings")
    .args(args)
    .stdin(Stdio::null())
    .output()
    .map_err(|error| SettingsError::System(format!("argvus-control-center: {error}")))?;
  if output.status.success() {
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
  } else {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(SettingsError::System(if stderr.is_empty() {
      format!(
        "argvus-control-center system-settings exited with {}",
        output.status
      )
    } else {
      stderr
    }))
  }
}
