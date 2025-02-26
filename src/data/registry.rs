use std::{collections::HashMap, path::Path};

use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("Io: {0}")]
  Io(#[from] std::io::Error),
  #[error("SerdeJson: {0}")]
  SerdeJson(#[from] serde_json::Error),
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct Registry {
  #[serde(flatten)]
  pub mods: HashMap<String, Mod>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Mod {
  pub id: String,
  pub version: String,
  pub url: String,
  pub image_url: String,
  pub name: String,
}

impl Registry {
  pub async fn load(path: &Path) -> Result<Self, Error> {
    Ok(serde_json::from_slice(fs::read(path).await?.as_slice())?)
  }
}
