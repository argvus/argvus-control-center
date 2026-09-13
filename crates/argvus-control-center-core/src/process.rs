use std::io::{self, BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{
  Arc, Mutex,
  atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct ProcessRequest {
  pub program: String,
  pub args: Vec<String>,
  pub timeout: Option<Duration>,
}

impl ProcessRequest {
  pub fn new(program: impl Into<String>) -> Self {
    Self {
      program: program.into(),
      args: Vec::new(),
      timeout: None,
    }
  }

  pub fn arg(mut self, value: impl Into<String>) -> Self {
    self.args.push(value.into());
    self
  }

  pub fn timeout(mut self, timeout: Duration) -> Self {
    self.timeout = Some(timeout);
    self
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutput {
  pub stdout: Vec<u8>,
  pub stderr: Vec<u8>,
  pub status: Option<i32>,
  pub timed_out: bool,
}

#[derive(Debug)]
pub enum ProcessError {
  Io(io::Error),
  EmptyProgram,
}

impl std::fmt::Display for ProcessError {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Io(error) => write!(formatter, "process failed: {error}"),
      Self::EmptyProgram => formatter.write_str("process program cannot be empty"),
    }
  }
}

impl std::error::Error for ProcessError {}

#[derive(Debug, Default)]
pub struct LiveProcess {
  inner: Arc<LiveInner>,
}
#[derive(Debug, Default)]
struct LiveInner {
  lines: Mutex<Vec<String>>,
  finished: AtomicBool,
}

impl LiveProcess {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn push_line(&self, line: &str) {
    if let Ok(mut lines) = self.inner.lines.lock() {
      lines.push(line.to_owned());
    }
  }

  pub fn output(&self) -> String {
    self
      .inner
      .lines
      .lock()
      .map(|lines| lines.join("\n"))
      .unwrap_or_default()
  }

  pub fn is_finished(&self) -> bool {
    self.inner.finished.load(Ordering::Acquire)
  }

  pub fn finish(&self) {
    self.inner.finished.store(true, Ordering::Release);
  }
}

impl Clone for LiveProcess {
  fn clone(&self) -> Self {
    Self {
      inner: Arc::clone(&self.inner),
    }
  }
}

pub trait ProcessRunner: Send + Sync {
  fn run(&self, request: &ProcessRequest) -> Result<ProcessOutput, ProcessError>;

  fn run_live(
    &self,
    request: &ProcessRequest,
    _live: &LiveProcess,
  ) -> Result<ProcessOutput, ProcessError> {
    self.run(request)
  }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemProcessRunner;

impl ProcessRunner for SystemProcessRunner {
  fn run(&self, request: &ProcessRequest) -> Result<ProcessOutput, ProcessError> {
    if request.program.trim().is_empty() {
      return Err(ProcessError::EmptyProgram);
    }
    let mut child = Command::new(&request.program)
      .args(&request.args)
      .stdin(Stdio::null())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .map_err(ProcessError::Io)?;
    let stdout = child
      .stdout
      .take()
      .ok_or_else(|| ProcessError::Io(io::Error::other("stdout was not captured")))?;
    let stderr = child
      .stderr
      .take()
      .ok_or_else(|| ProcessError::Io(io::Error::other("stderr was not captured")))?;
    // Drain both pipes on dedicated threads so commands with large output
    // (e.g. `pacman -Sl`, which emits hundreds of KB) do not fill the pipe
    // buffer while the parent polls `try_wait`, which would block the child
    // until the timeout kills it.
    let stdout_reader = std::thread::spawn(|| read_pipe(stdout));
    let stderr_reader = std::thread::spawn(|| read_pipe(stderr));
    let deadline = request.timeout.map(|timeout| Instant::now() + timeout);
    loop {
      match child.try_wait().map_err(ProcessError::Io)? {
        Some(status) => {
          let stdout = stdout_reader.join().unwrap_or_default();
          let stderr = stderr_reader.join().unwrap_or_default();
          return Ok(ProcessOutput {
            stdout,
            stderr,
            status: status.code(),
            timed_out: false,
          });
        }
        None if deadline.is_some_and(|value| Instant::now() >= value) => {
          let _ = child.kill();
          let _ = child.wait();
          let stdout = stdout_reader.join().unwrap_or_default();
          let stderr = stderr_reader.join().unwrap_or_default();
          return Ok(ProcessOutput {
            stdout,
            stderr,
            status: None,
            timed_out: true,
          });
        }
        None => thread::sleep(Duration::from_millis(10)),
      }
    }
  }

  fn run_live(
    &self,
    request: &ProcessRequest,
    live: &LiveProcess,
  ) -> Result<ProcessOutput, ProcessError> {
    if request.program.trim().is_empty() {
      return Err(ProcessError::EmptyProgram);
    }
    let mut child = Command::new(&request.program)
      .args(&request.args)
      .stdin(Stdio::null())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .map_err(ProcessError::Io)?;
    let stdout = child
      .stdout
      .take()
      .ok_or_else(|| ProcessError::Io(io::Error::other("stdout was not captured")))?;
    let stderr = child
      .stderr
      .take()
      .ok_or_else(|| ProcessError::Io(io::Error::other("stderr was not captured")))?;
    let live_stdout = live.clone();
    let stdout_reader =
      std::thread::spawn(move || read_pipe_live(BufReader::new(stdout), &live_stdout));
    let live_stderr = live.clone();
    let stderr_reader =
      std::thread::spawn(move || read_pipe_live(BufReader::new(stderr), &live_stderr));
    let deadline = request.timeout.map(|timeout| Instant::now() + timeout);
    let output = loop {
      match child.try_wait().map_err(ProcessError::Io)? {
        Some(status) => {
          let stdout = stdout_reader.join().unwrap_or_default();
          let stderr = stderr_reader.join().unwrap_or_default();
          break Ok(ProcessOutput {
            stdout,
            stderr,
            status: status.code(),
            timed_out: false,
          });
        }
        None if deadline.is_some_and(|value| Instant::now() >= value) => {
          let _ = child.kill();
          let _ = child.wait();
          let stdout = stdout_reader.join().unwrap_or_default();
          let stderr = stderr_reader.join().unwrap_or_default();
          break Ok(ProcessOutput {
            stdout,
            stderr,
            status: None,
            timed_out: true,
          });
        }
        None => thread::sleep(Duration::from_millis(10)),
      }
    };
    live.finish();
    output
  }
}

fn read_pipe(mut pipe: impl std::io::Read) -> Vec<u8> {
  let mut output = Vec::new();
  let _ = pipe.read_to_end(&mut output);
  output
}

fn read_pipe_live(mut pipe: impl BufRead, live: &LiveProcess) -> Vec<u8> {
  let mut output = Vec::new();
  let mut line = String::new();
  loop {
    line.clear();
    match pipe.read_line(&mut line) {
      Ok(0) => break,
      Ok(_) => {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if !trimmed.is_empty() {
          live.push_line(trimmed);
        }
        output.extend_from_slice(line.as_bytes());
      }
      Err(_) => break,
    }
  }
  output
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn builds_arguments_without_a_shell() {
    let request = ProcessRequest::new("printf")
      .arg("%s")
      .arg("hello;not-a-command");
    let output = SystemProcessRunner.run(&request).unwrap();
    assert_eq!(output.stdout, b"hello;not-a-command");
  }

  #[test]
  fn rejects_empty_program() {
    assert!(matches!(
      SystemProcessRunner.run(&ProcessRequest::new(" ")),
      Err(ProcessError::EmptyProgram)
    ));
  }

  #[test]
  fn drains_large_stdout_without_deadlocking() {
    let request = ProcessRequest::new("head")
      .arg("-c")
      .arg("500000")
      .arg("/dev/zero")
      .timeout(Duration::from_secs(30));
    let output = SystemProcessRunner.run(&request).unwrap();
    assert_eq!(output.stdout.len(), 500_000);
    assert_eq!(output.status, Some(0));
    assert!(!output.timed_out);
  }

  #[test]
  fn run_live_streams_lines_into_the_sink() {
    let live = LiveProcess::new();
    let request = ProcessRequest::new("printf").arg("%s").arg("alpha\nbeta\n");
    let output = SystemProcessRunner.run_live(&request, &live).unwrap();
    assert_eq!(output.stdout, b"alpha\nbeta\n");
    assert_eq!(output.status, Some(0));
    assert_eq!(live.output(), "alpha\nbeta");
    assert!(live.is_finished());
  }

  #[test]
  fn run_live_captures_stderr_but_only_streams_non_empty_lines() {
    let live = LiveProcess::new();
    let request = ProcessRequest::new("sh")
      .arg("-c")
      .arg("printf 'oops\\n' >&2")
      .timeout(Duration::from_secs(30));
    let output = SystemProcessRunner.run_live(&request, &live).unwrap();
    assert_eq!(output.stderr, b"oops\n");
    assert_eq!(output.stdout, Vec::<u8>::new());
    assert_eq!(live.output(), "oops");
    assert!(live.is_finished());
  }

  #[test]
  fn run_live_default_falls_back_to_buffered_run() {
    #[derive(Debug, Default)]
    struct RecordingRunner(std::sync::Mutex<Option<ProcessRequest>>);
    impl ProcessRunner for RecordingRunner {
      fn run(&self, request: &ProcessRequest) -> Result<ProcessOutput, ProcessError> {
        *self.0.lock().unwrap() = Some(request.clone());
        Ok(ProcessOutput {
          stdout: b"recorded".to_vec(),
          stderr: Vec::new(),
          status: Some(0),
          timed_out: false,
        })
      }
    }
    let runner = RecordingRunner::default();
    let live = LiveProcess::new();
    let request = ProcessRequest::new("echo").arg("hello");
    let output = runner.run_live(&request, &live).unwrap();
    assert_eq!(output.stdout, b"recorded");
    assert!(live.output().is_empty());
    assert!(!live.is_finished());
    assert_eq!(runner.0.lock().unwrap().clone().unwrap().args, ["hello"]);
  }
}
