//! Session-owned component lifecycle operations.

use std::time::Duration;

use argvus_control_center_core::process::{ProcessRequest, ProcessRunner, SystemProcessRunner};

/// Restarts the Quickshell Control Panel after a persisted interface-language change.
pub fn restart_control_panel() -> Result<(), String> {
  restart_control_panel_with(&SystemProcessRunner)
}

fn restart_control_panel_with<R: ProcessRunner>(runner: &R) -> Result<(), String> {
  let output = runner
    .run(
      &ProcessRequest::new("argvus-sessionctl")
        .arg("restart")
        .arg("control-panel")
        .timeout(Duration::from_secs(10)),
    )
    .map_err(|error| error.to_string())?;
  if output.timed_out || output.status != Some(0) {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    return Err(if stderr.is_empty() {
      "argvus-sessionctl restart control-panel failed".into()
    } else {
      stderr
    });
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use argvus_control_center_core::process::{ProcessError, ProcessOutput};

  struct RecordingRunner {
    output: ProcessOutput,
  }

  impl ProcessRunner for RecordingRunner {
    fn run(&self, request: &ProcessRequest) -> Result<ProcessOutput, ProcessError> {
      assert_eq!(request.program, "argvus-sessionctl");
      assert_eq!(request.args, ["restart", "control-panel"]);
      assert_eq!(request.timeout, Some(Duration::from_secs(10)));
      Ok(self.output.clone())
    }
  }

  #[test]
  fn restarts_only_the_control_panel_component() {
    let runner = RecordingRunner {
      output: ProcessOutput {
        stdout: Vec::new(),
        stderr: Vec::new(),
        status: Some(0),
        timed_out: false,
      },
    };
    assert!(restart_control_panel_with(&runner).is_ok());
  }

  #[test]
  fn reports_restart_failure() {
    let runner = RecordingRunner {
      output: ProcessOutput {
        stdout: Vec::new(),
        stderr: b"service unavailable".to_vec(),
        status: Some(1),
        timed_out: false,
      },
    };
    assert_eq!(
      restart_control_panel_with(&runner).unwrap_err(),
      "service unavailable"
    );
  }

  #[test]
  fn reports_restart_timeout() {
    let runner = RecordingRunner {
      output: ProcessOutput {
        stdout: Vec::new(),
        stderr: Vec::new(),
        status: None,
        timed_out: true,
      },
    };
    assert_eq!(
      restart_control_panel_with(&runner).unwrap_err(),
      "argvus-sessionctl restart control-panel failed"
    );
  }
}
