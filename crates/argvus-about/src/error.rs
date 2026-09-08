#[derive(Debug, thiserror::Error)]
pub enum AboutError {
  #[error("failed to open link: {0}")]
  OpenLink(#[source] std::io::Error),
}
