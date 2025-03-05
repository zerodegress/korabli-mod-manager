use super::App;

use std::{collections::HashSet, path::PathBuf};

use crate::mod_manager::ModManager;
use crate::tasks::download::{Download, DownloadUpdate};
use crate::tasks::install::{Install, InstallState, InstallUpdate};
use crate::tasks::uninstall::{
  Uninstall, UninstallState, UninstallUpdate,
};
use crate::{data::registry::Registry, messages::Message};
use futures::stream::FuturesOrdered;
use iced::Task;

impl App {
  pub(super) fn update(&mut self, message: Message) -> Task<Message> {
    match message {
      Message::UpdateRecords { mod_manager } => Task::perform(
        async move {
          let records =
            mod_manager.records().await.unwrap_or_default();
          (mod_manager, records)
        },
        |(mod_manager, records)| Message::RecordsUpdated {
          mod_manager,
          records,
        },
      ),
      Message::QueueUpdateRecords => {
        if let Some(mod_manager) = self.mod_manager.take() {
          Task::done(Message::UpdateRecords { mod_manager })
        } else {
          self.need_records_update = true;
          Task::none()
        }
      }
      Message::RecordsUpdated {
        mod_manager,
        records,
      } => {
        self.records = records;
        Task::done(Message::ModManagerReady { mod_manager })
      }
      Message::Warning { title, text } => {
        let _ = native_dialog::MessageDialog::new()
          .set_title(title.as_str())
          .set_text(text.as_str())
          .set_type(native_dialog::MessageType::Warning)
          .show_alert();
        Task::none()
      }
      Message::CurrentModsUpdated {
        mod_manager,
        current_mods,
      } => {
        self.current_mods = current_mods;
        Task::done(Message::ModManagerReady { mod_manager })
      }
      Message::UpdateCurrentMods { mod_manager } => {
        let old_current_mods = self.current_mods.to_owned();
        self.need_current_mods_update = false;
        Task::perform(
          async move {
            let records = mod_manager.records().await;
            (
              mod_manager,
              records.map(|records| {
                records
                  .records
                  .keys()
                  .cloned()
                  .collect::<HashSet<_>>()
              }),
            )
          },
          move |(mod_manager, current_mods)| {
            Message::CurrentModsUpdated {
              mod_manager,
              current_mods: current_mods
                .unwrap_or(old_current_mods.to_owned()),
            }
          },
        )
      }
      Message::QueueUpdateCurrentMods => {
        if let Some(mod_manager) = self.mod_manager.take() {
          Task::done(Message::UpdateCurrentMods { mod_manager })
        } else {
          self.need_current_mods_update = true;
          Task::none()
        }
      }
      Message::PrepareModManager { game_dir_path } => Task::perform(
        async move {
          let mut mod_manager =
            ModManager::try_from_game_dir(game_dir_path.as_path())
              .expect("wtf mod_manager init failed");
          mod_manager
            .ensure_records()
            .await
            .expect("wtf cannot ensure records");
          mod_manager
        },
        |mod_manager| Message::ModManagerReady { mod_manager },
      ),
      Message::RegistryLoaded(registry) => {
        self.registries.push_front(registry);
        self.loading_registry = false;
        Task::none()
      }
      Message::LoadRegistries { urls: url } => {
        self.registries.clear();
        Task::stream({
          FuturesOrdered::from_iter(url.into_iter().map(
            |url| async move {
              match url.scheme() {
                "http" | "https" => {
                  let Ok(res) = reqwest::get(url.to_owned()).await
                  else {
                    return Message::Warning {
                      title: "Registry加载失败".to_string(),
                      text: "从网络加载Registry时遭遇错误"
                        .to_string(),
                    };
                  };
                  let Ok(registry) = serde_json::from_reader(
                    res
                      .bytes()
                      .await
                      .unwrap_or_default()
                      .iter()
                      .as_slice(),
                  ) else {
                    return Message::Warning {
                      title: "Registry加载失败".to_string(),
                      text: "从网络获取的Registry格式错误"
                        .to_string(),
                    };
                  };
                  Message::RegistryLoaded(registry)
                }
                "file" => Message::RegistryLoaded(
                  Registry::load(PathBuf::from(url.path()).as_path())
                    .await
                    .unwrap_or_default(),
                ),
                "data" => {
                  let (ty, data) = url
                    .path()
                    .split_once(";")
                    .unwrap_or(("hex", url.path()));
                  match ty {
                    "hex" => {
                      let Ok(data) = hex::decode(data) else {
                        return Message::Warning {
                          title: "Registry加载失败".to_string(),
                          text: "hex data格式错误".to_string(),
                        };
                      };
                      let registry =
                        serde_json::from_slice(data.as_slice());
                      match registry {
                        Err(err) => Message::Warning {
                          title: "Registry加载失败".to_string(),
                          text: format!(
                            "hex data内容格式错误: {}",
                            err
                          ),
                        },
                        Ok(registry) => {
                          Message::RegistryLoaded(registry)
                        }
                      }
                    }
                    _ => todo!(),
                  }
                }
                _ => todo!(),
              }
            },
          ))
        })
      }
      Message::AddCurrentMod { id } => {
        self.current_mods.insert(id);
        Task::none()
      }
      Message::RemoveCurrentMod { id } => {
        self.current_mods.remove(&id);
        Task::none()
      }
      Message::AddInstallMod { id } => {
        self.uninstall_mods.remove(&id);
        self.install_mods.insert(id);
        Task::none()
      }
      Message::AddUninstallMod { id } => {
        self.install_mods.remove(&id);
        self.uninstall_mods.insert(id);
        Task::none()
      }
      Message::RemoveInstallMod { id } => {
        self.install_mods.remove(&id);
        Task::none()
      }
      Message::RemoveUninstallMod { id } => {
        self.uninstall_mods.remove(&id);
        Task::none()
      }
      Message::GameDirInput(game_dir) => {
        self.game_dir = game_dir;
        Task::none()
      }
      Message::UpdateMods { install, uninstall } => Task::batch(
        uninstall
          .into_iter()
          .map(|id| Task::done(Message::UninstallMod { id }))
          .chain(install.into_iter().map(|id| {
            if let Some(modr) = self
              .registries
              .iter()
              .find_map(|registry| registry.mods.get(&id))
            {
              Task::done(Message::GetMod {
                url: modr.url.parse().expect("wtf illegal registry"),
                id: modr.id.to_owned(),
              })
            } else {
              todo!()
            }
          })),
      ),
      Message::GetMod { url, id } => {
        let mut download = Download::new(id.to_owned(), url);
        let task = download.start();
        self.downloads.push(download);

        task.map(move |update| Message::GetModUpdated {
          id: id.to_owned(),
          update,
        })
      }
      Message::GetModUpdated { id, update } => {
        if let Some(download) =
          self.downloads.iter_mut().find(|x| x.id() == id)
        {
          download.update(update.to_owned());
          match update {
            DownloadUpdate::Downloading(_) => Task::none(),
            DownloadUpdate::Finished(res) => match res {
              Err(err) => panic!("{}", err),
              Ok(path) => {
                if let Some(pos) =
                  self.downloads.iter().position(|x| x.id() == id)
                {
                  self.downloads.remove(pos);
                }
                Task::done(Message::InstallMod {
                  path,
                  ty: match self.request_mod(&id) {
                    None => "".to_string(),
                    Some(m) => m.ty.to_owned(),
                  },
                  id,
                })
              }
            },
          }
        } else {
          Task::none()
        }
      }
      Message::InstallMod { path, id, ty } => {
        let mut install = Install::new(
          id.as_str(),
          path.as_path(),
          self
            .request_mod(id.as_str())
            .map(|m| m.version.to_owned())
            .unwrap_or_default()
            .as_str(),
          ty.as_str(),
        );
        if let Some(mod_manager) = self.mod_manager.take() {
          let task = install.start(mod_manager);
          self.installs.push_back(install);

          task.map(move |update| Message::InstallModUpdated {
            id: id.to_owned(),
            update,
          })
        } else {
          self.installs.push_back(install);
          Task::none()
        }
      }
      Message::InstallModUpdated { id, update } => {
        if let Some(install) =
          self.installs.iter_mut().find(|x| x.id() == id.as_str())
        {
          install.update(update.to_owned());
          match update {
            InstallUpdate::Running(_) => Task::none(),
            InstallUpdate::Finished((res, mod_manager)) => {
              match res {
                Err(err) => Task::batch([
                  Task::done(Message::ModManagerReady {
                    mod_manager,
                  }),
                  Task::done(Message::Warning {
                    title: "模组安装失败！".to_string(),
                    text: format!("理由：{}", err),
                  }),
                ]),
                Ok(()) => {
                  if let Some(pos) =
                    self.installs.iter().position(|x| x.id() == id)
                  {
                    self.installs.remove(pos);
                  }
                  Task::batch([
                    Task::done(Message::ModManagerReady {
                      mod_manager,
                    }),
                    Task::done(Message::AddCurrentMod {
                      id: id.to_string(),
                    }),
                    Task::done(Message::QueueUpdateRecords),
                  ])
                }
              }
            }
          }
        } else {
          Task::none()
        }
      }
      Message::UninstallMod { id } => {
        let mut uninstall = Uninstall::new(id.as_str());
        if let Some(mod_manager) = self.mod_manager.take() {
          let task = uninstall.start(mod_manager);
          self.uninstalls.push_back(uninstall);

          task.map(move |update| Message::UninstallModUpdated {
            id: id.to_owned(),
            update,
          })
        } else {
          self.uninstalls.push_back(uninstall);
          Task::none()
        }
      }
      Message::UninstallModUpdated { id, update } => {
        if let Some(uninstall) =
          self.uninstalls.iter_mut().find(|x| x.id() == id.as_str())
        {
          uninstall.update(update.to_owned());
          match update {
            UninstallUpdate::Running(_) => Task::none(),
            UninstallUpdate::Finished((res, mod_manager)) => {
              match res {
                Err(err) => Task::batch([
                  Task::done(Message::ModManagerReady {
                    mod_manager,
                  }),
                  Task::done(Message::Warning {
                    title: "模组卸载失败！".to_string(),
                    text: format!("理由：{}", err),
                  }),
                ]),
                Ok(()) => {
                  if let Some(pos) =
                    self.uninstalls.iter().position(|x| x.id() == id)
                  {
                    self.uninstalls.remove(pos);
                  }
                  Task::batch([
                    Task::done(Message::ModManagerReady {
                      mod_manager,
                    }),
                    Task::done(Message::RemoveCurrentMod {
                      id: id.to_string(),
                    }),
                    Task::done(Message::QueueUpdateRecords),
                  ])
                }
              }
            }
          }
        } else {
          Task::none()
        }
      }
      Message::ModManagerReady { mod_manager } => loop {
        if let Some(mut uninstall) = self.uninstalls.pop_front() {
          if let &UninstallState::Ready /* | &UninstallState::Failed */ =
            uninstall.state()
          {
            let task = uninstall.start(mod_manager);
            let id = uninstall.id().to_owned();
            self.uninstalls.push_front(uninstall);
            return task.map(move |update| {
              Message::UninstallModUpdated {
                id: id.to_owned(),
                update,
              }
            });
          }
        } else if let Some(mut install) = self.installs.pop_front() {
          if let &InstallState::Ready /* | &InstallState::Failed */ =
            install.state()
          {
            let task = install.start(mod_manager);
            let id = install.id().to_owned();
            self.installs.push_front(install);
            return task.map(move |update| {
              Message::InstallModUpdated {
                id: id.to_owned(),
                update,
              }
            });
          }
        } else if self.need_current_mods_update {
          self.need_current_mods_update = false;
          return Task::done(Message::UpdateCurrentMods {
            mod_manager,
          });
        } else if self.need_records_update {
          self.need_records_update = false;
          return Task::done(Message::UpdateRecords { mod_manager });
        } else {
          self.mod_manager.replace(mod_manager);
          return Task::none();
        }
      },
    }
  }
}
