use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
  mpsc::{self, Receiver, TryRecvError},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(u64);

#[derive(Debug, Clone)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
  pub fn is_cancelled(&self) -> bool {
    self.0.load(Ordering::Acquire)
  }
}

#[derive(Debug)]
pub enum JobState<T> {
  Running,
  Finished(Result<T, String>),
}

pub struct JobHandle<T> {
  id: JobId,
  cancel: Arc<AtomicBool>,
  receiver: Receiver<Result<T, String>>,
}

impl<T> JobHandle<T> {
  pub const fn id(&self) -> JobId {
    self.id
  }
  pub fn cancel(&self) {
    self.cancel.store(true, Ordering::Release);
  }
  pub fn try_state(&self) -> JobState<T> {
    match self.receiver.try_recv() {
      Ok(result) => JobState::Finished(result),
      Err(TryRecvError::Empty) => JobState::Running,
      Err(TryRecvError::Disconnected) => {
        JobState::Finished(Err("background job disconnected".into()))
      }
    }
  }
}

#[derive(Debug, Default)]
pub struct JobManager {
  next_id: std::sync::atomic::AtomicU64,
}

impl JobManager {
  pub fn spawn<T, F>(&self, task: F) -> JobHandle<T>
  where
    T: Send + 'static,
    F: FnOnce(CancellationToken) -> Result<T, String> + Send + 'static,
  {
    let id = JobId(self.next_id.fetch_add(1, Ordering::Relaxed));
    let cancel = Arc::new(AtomicBool::new(false));
    let token = CancellationToken(Arc::clone(&cancel));
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
      let _ = sender.send(task(token));
    });
    JobHandle {
      id,
      cancel,
      receiver,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::time::Duration;

  #[test]
  fn job_runs_off_thread_and_can_be_cancelled() {
    let manager = JobManager::default();
    let handle = manager.spawn(|token| {
      std::thread::sleep(Duration::from_millis(20));
      Ok(token.is_cancelled())
    });
    handle.cancel();
    std::thread::sleep(Duration::from_millis(40));
    assert!(matches!(handle.try_state(), JobState::Finished(Ok(true))));
  }
}
