use crate::model::*;
use argvus_control_center_core::{
  capabilities::Capabilities,
  process::{ProcessRequest, ProcessRunner, SystemProcessRunner},
  sanitize::terminal_text,
};
use argvus_control_center_storage::model::{SmartHealth, UsageLevel, usage_level};
use std::{fs, time::Duration};

pub fn collect(cap: &Capabilities) -> DiagnosticFacts {
  let mut facts = DiagnosticFacts {
    storage: argvus_control_center_storage::backend::collect(cap).unwrap_or_default(),
    ..Default::default()
  };
  facts.failed_system_units = cap
    .has_systemd
    .then(|| failed_units(&["--system", "--type=service"]))
    .flatten();
  facts.failed_user_units = cap
    .has_systemd
    .then(|| failed_units(&["--user", "--type=service"]))
    .flatten();
  if cap.has_lspci {
    let devices = run("lspci", &["-nnk"]);
    facts.nvidia_present = Some(devices.to_ascii_lowercase().contains("nvidia"));
  }
  facts.nvidia_module_loaded = fs::read_to_string("/proc/modules").ok().map(|value| {
    value.lines().any(|line| {
      line
        .split_whitespace()
        .next()
        .is_some_and(|module| module == "nvidia" || module.starts_with("nvidia_"))
    })
  });
  if cap.has_glxinfo {
    let output = run("glxinfo", &["-B"]);
    facts.opengl_renderer = output
      .lines()
      .find_map(|line| line.strip_prefix("OpenGL renderer string:"))
      .map(|value| terminal_text(value.trim()));
  }
  if cap.has_vulkaninfo {
    facts.vulkan_available = Some(run_status("vulkaninfo", &["--summary"]));
  }
  facts
}

fn failed_units(args: &[&str]) -> Option<usize> {
  let mut arguments = vec!["--failed", "--no-legend", "--plain"];
  arguments.extend_from_slice(args);
  run_output("systemctl", &arguments).map(|output| {
    output
      .lines()
      .filter(|line| !line.trim().is_empty())
      .count()
  })
}

fn run(program: &str, args: &[&str]) -> String {
  run_output(program, args).unwrap_or_default()
}

fn run_output(program: &str, args: &[&str]) -> Option<String> {
  let request = args
    .iter()
    .fold(ProcessRequest::new(program), |request, arg| {
      request.arg(*arg)
    })
    .timeout(Duration::from_secs(5));
  SystemProcessRunner
    .run(&request)
    .ok()
    .filter(|output| output.status == Some(0) && !output.timed_out)
    .map(|output| terminal_text(&String::from_utf8_lossy(&output.stdout)))
}

fn run_status(program: &str, args: &[&str]) -> bool {
  let request = args
    .iter()
    .fold(ProcessRequest::new(program), |request, arg| {
      request.arg(*arg)
    })
    .timeout(Duration::from_secs(5));
  SystemProcessRunner
    .run(&request)
    .is_ok_and(|output| output.status == Some(0) && !output.timed_out)
}

pub fn evaluate(facts: &DiagnosticFacts, cap: &Capabilities) -> Vec<DiagnosticCheck> {
  let mut checks = Vec::new();
  match facts.failed_system_units {
    Some(0) => checks.push(ok(
      "services.system",
      "System services",
      "No failed system units",
    )),
    Some(n) => checks.push(DiagnosticCheck {
      id: "system.failed_units".into(),
      category: "services".into(),
      severity: Severity::Warning,
      title: "Failed system units".into(),
      summary: format!("{n} system unit(s) failed"),
      details: "The system manager reports failed units.".into(),
      remediation_hint: Some("Open Services > Failed".into()),
      route: Some("services/failed".into()),
    }),
    None => checks.push(unknown(
      "services.system",
      "System services",
      "Service state unavailable",
    )),
  }
  if facts.failed_user_units.is_some_and(|n| n > 0) {
    checks.push(DiagnosticCheck {
      id: "user.failed_units".into(),
      category: "services".into(),
      severity: Severity::Warning,
      title: "Failed user units".into(),
      summary: format!("{} user unit(s) failed", facts.failed_user_units.unwrap()),
      details: "The user system manager reports failed units.".into(),
      remediation_hint: Some("Open Services > User".into()),
      route: Some("services/user".into()),
    });
  }
  if facts.nvidia_present == Some(true) {
    match facts.nvidia_module_loaded {
      Some(true) => checks.push(ok(
        "graphics.nvidia",
        "NVIDIA driver",
        "NVIDIA hardware and kernel module are available",
      )),
      Some(false) => {
        checks.push(DiagnosticCheck {
          id: "graphics.nvidia".into(),
          category: "graphics".into(),
          severity: Severity::Warning,
          title: "NVIDIA driver".into(),
          summary: "NVIDIA hardware found, but the kernel module is not loaded".into(),
          details:
            "Applications may fall back to another renderer until the NVIDIA module is loaded."
              .into(),
          remediation_hint: Some("Check Hardware > GPU and the installed NVIDIA driver.".into()),
          route: Some("hardware/gpu".into()),
        })
      }
      None => checks.push(unknown(
        "graphics.nvidia",
        "NVIDIA driver",
        "NVIDIA hardware found; module state unavailable",
      )),
    }
  }
  if let Some(renderer) = facts.opengl_renderer.as_deref() {
    let renderer_lower = renderer.to_ascii_lowercase();
    let software = renderer_lower.contains("llvmpipe")
      || renderer_lower.contains("softpipe")
      || renderer_lower.contains("software rasterizer");
    checks.push(DiagnosticCheck {
      id: "graphics.opengl".into(),
      category: "graphics".into(),
      severity: if software {
        Severity::Warning
      } else {
        Severity::Ok
      },
      title: "OpenGL renderer".into(),
      summary: renderer.into(),
      details: if software {
        "OpenGL is using software rendering instead of a hardware GPU.".into()
      } else {
        "OpenGL reports a hardware renderer.".into()
      },
      remediation_hint: software.then(|| "Check Hardware > GPU and graphics drivers.".into()),
      route: Some("hardware/gpu".into()),
    });
  }
  if let Some(available) = facts.vulkan_available {
    checks.push(if available {
      ok("graphics.vulkan", "Vulkan", "Vulkan is available")
    } else {
      DiagnosticCheck {
        id: "graphics.vulkan".into(),
        category: "graphics".into(),
        severity: Severity::Warning,
        title: "Vulkan".into(),
        summary: "Vulkan is not available".into(),
        details: "vulkaninfo could not initialize a Vulkan device.".into(),
        remediation_hint: Some("Check the installed graphics driver and Vulkan packages.".into()),
        route: Some("hardware/gpu".into()),
      }
    });
  }
  let relevant = facts.storage.filesystems.iter().filter(|f| {
    f.mountpoint
      .as_deref()
      .is_some_and(|p| p == "/" || p == "/home")
  });
  for fs in relevant {
    if let (Some(total), Some(available)) = (fs.total_bytes, fs.available_bytes) {
      let level = usage_level(total, available);
      checks.push(DiagnosticCheck {
        id: format!(
          "storage.usage.{}",
          fs.mountpoint.as_deref().unwrap_or("unknown")
        ),
        category: "storage".into(),
        severity: match level {
          UsageLevel::Ok => Severity::Ok,
          UsageLevel::Warning => Severity::Warning,
          UsageLevel::Critical => Severity::Error,
        },
        title: format!("Filesystem {}", fs.mountpoint.as_deref().unwrap_or("?")),
        summary: format!(
          "{}% used",
          total.saturating_sub(available).saturating_mul(100) / total.max(1)
        ),
        details: format!("{} bytes available", available),
        remediation_hint: Some("Open Storage > Disk usage".into()),
        route: Some("storage/usage".into()),
      });
    }
  }
  if facts.storage.smart_available {
    for smart in &facts.storage.smart {
      let severity = match smart.health {
        SmartHealth::Failed => Severity::Error,
        SmartHealth::Warning => Severity::Warning,
        SmartHealth::Passed => Severity::Ok,
        SmartHealth::Unsupported => Severity::Info,
        SmartHealth::Unknown => Severity::Unknown,
      };
      checks.push(DiagnosticCheck {
        id: format!("storage.smart.{}", smart.device),
        category: "storage".into(),
        severity,
        title: format!("SMART {}", smart.device),
        summary: format!("{:?}", smart.health),
        details: "Read-only SMART evidence from the device.".into(),
        remediation_hint: None,
        route: Some("storage/smart".into()),
      });
    }
  } else {
    checks.push(info("storage.smart", "SMART", "smartctl is unavailable"));
  }
  if facts.package_lock {
    checks.push(DiagnosticCheck {
      id: "packages.lock".into(),
      category: "packages".into(),
      severity: Severity::Warning,
      title: "Package database lock".into(),
      summary: "A package operation may be active".into(),
      details: "The lock is reported as evidence only and is never removed automatically.".into(),
      remediation_hint: None,
      route: Some("packages".into()),
    });
  }
  if let Some(n) = facts.package_updates {
    checks.push(DiagnosticCheck {
      id: "packages.updates".into(),
      category: "packages".into(),
      severity: if n == 0 { Severity::Ok } else { Severity::Info },
      title: "Package updates".into(),
      summary: format!("{n} update(s) available"),
      details: "Package status is consumed from the existing package backend.".into(),
      remediation_hint: None,
      route: Some("packages/updates".into()),
    });
  }
  checks.push(if cap.is_virtual_machine {
    info(
      "graphics.virtual-machine",
      "Graphics environment",
      "Virtual machine detected",
    )
  } else {
    info(
      "graphics.environment",
      "Graphics environment",
      "Hardware graphics checks are provided by Hardware",
    )
  });
  checks.sort_by_key(|c| c.severity);
  checks
}
fn info(id: &str, title: &str, summary: &str) -> DiagnosticCheck {
  DiagnosticCheck {
    id: id.into(),
    category: "system".into(),
    severity: Severity::Info,
    title: title.into(),
    summary: summary.into(),
    details: summary.into(),
    ..Default::default()
  }
}
fn unknown(id: &str, title: &str, summary: &str) -> DiagnosticCheck {
  DiagnosticCheck {
    severity: Severity::Unknown,
    ..info(id, title, summary)
  }
}
fn ok(id: &str, title: &str, summary: &str) -> DiagnosticCheck {
  DiagnosticCheck {
    severity: Severity::Ok,
    ..info(id, title, summary)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use argvus_control_center_storage::model::Filesystem;
  #[test]
  fn root_usage_severity_is_objective() {
    let mut f = DiagnosticFacts::default();
    f.storage.filesystems.push(Filesystem {
      mountpoint: Some("/".into()),
      total_bytes: Some(100),
      available_bytes: Some(3),
      ..Default::default()
    });
    let c = evaluate(&f, &Capabilities::default());
    assert!(
      c.iter()
        .any(|v| v.severity == Severity::Error && v.id == "storage.usage./")
    );
  }
  #[test]
  fn unavailable_smart_is_info() {
    let c = evaluate(&DiagnosticFacts::default(), &Capabilities::default());
    assert!(
      c.iter()
        .any(|v| v.id == "storage.smart" && v.severity == Severity::Info)
    );
  }

  #[test]
  fn reports_failed_services_and_software_rendering() {
    let facts = DiagnosticFacts {
      failed_system_units: Some(2),
      opengl_renderer: Some("llvmpipe (LLVM 20.0.0, 256 bits)".into()),
      ..Default::default()
    };
    let checks = evaluate(&facts, &Capabilities::default());
    assert!(
      checks
        .iter()
        .any(|check| { check.id == "system.failed_units" && check.severity == Severity::Warning })
    );
    assert!(
      checks
        .iter()
        .any(|check| { check.id == "graphics.opengl" && check.severity == Severity::Warning })
    );
  }

  #[test]
  fn reports_nvidia_module_gap() {
    let facts = DiagnosticFacts {
      nvidia_present: Some(true),
      nvidia_module_loaded: Some(false),
      ..Default::default()
    };
    let checks = evaluate(&facts, &Capabilities::default());
    let check = checks
      .iter()
      .find(|check| check.id == "graphics.nvidia")
      .unwrap();
    assert_eq!(check.severity, Severity::Warning);
    assert!(check.summary.contains("not loaded"));
  }
}
