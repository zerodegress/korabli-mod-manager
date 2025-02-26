#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("Reqwest: {0}")]
  Reqwest(#[from] reqwest::Error),
}
