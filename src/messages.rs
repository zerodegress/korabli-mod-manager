use std::{collections::HashSet, path::PathBuf};

use url::Url;

use crate::{
  data::registry::Registry,
  mod_manager::{ModManager, Records},
  tasks::{
    download::DownloadUpdate, install::InstallUpdate,
    uninstall::UninstallUpdate,
  },
};

#[derive(Debug, Clone)]
pub enum Message {
  GameDirInput(String),
  RecordsUpdated {
    mod_manager: ModManager,
    records: Records,
  },
  UpdateRecords {
    mod_manager: ModManager,
  },
  QueueUpdateRecords,
  Warning {
    title: String,
    text: String,
  },
  UpdateMods {
    install: Vec<String>,
    uninstall: Vec<String>,
  },
  GetMod {
    url: Url,
    id: String,
  },
  GetModUpdated {
    id: String,
    update: DownloadUpdate,
  },
  InstallMod {
    path: PathBuf,
    id: String,
  },
  InstallModUpdated {
    id: String,
    update: InstallUpdate,
  },
  UninstallMod {
    id: String,
  },
  UninstallModUpdated {
    id: String,
    update: UninstallUpdate,
  },
  ModManagerReady {
    mod_manager: ModManager,
  },
  AddInstallMod {
    id: String,
  },
  RemoveInstallMod {
    id: String,
  },
  AddUninstallMod {
    id: String,
  },
  RemoveUninstallMod {
    id: String,
  },
  AddCurrentMod {
    id: String,
  },
  RemoveCurrentMod {
    id: String,
  },
  LoadRegistries {
    urls: Vec<Url>,
  },
  RegistryLoaded(Registry),
  PrepareModManager {
    game_dir_path: PathBuf,
  },
  QueueUpdateCurrentMods,
  UpdateCurrentMods {
    mod_manager: ModManager,
  },
  CurrentModsUpdated {
    mod_manager: ModManager,
    current_mods: HashSet<String>,
  },
}
