use std::{
  collections::{HashSet, VecDeque},
  env::current_dir,
  path::PathBuf,
};

use download::{Download, DownloadState, DownloadUpdate};
use futures::stream::FuturesOrdered;
use iced::{
  Element, Font, Length, Task, Theme,
  alignment::Vertical,
  widget::{
    button, checkbox, column, container, container::bordered_box,
    image, progress_bar, row, text, text_input,
  },
};
use install::{Install, InstallState, InstallUpdate};
use mod_manager::{ModManager, Records};
use registry::{Mod, Registry};
use uninstall::{Uninstall, UninstallState, UninstallUpdate};
use url::Url;

mod download;
mod install;
mod mod_manager;
mod registry;
mod uninstall;

fn main() {
  iced_main().expect("wtf iced")
}

pub fn iced_main() -> iced::Result {
  let registries = VecDeque::new();

  let init_task_batch = [
    Task::done(Message::PrepareModManager {
      game_dir_path: current_dir().expect("wtf current dir"),
    }),
    Task::done(Message::LoadRegistries {
      urls: vec![
        Url::parse("https://kmm.worker.zerodegress.ink/registry")
          .expect("wtf web registry"),
      ],
    }),
    Task::done(Message::QueueUpdateCurrentMods),
    Task::done(Message::QueueUpdateRecords),
  ];
  let app = iced::application(App::title, App::update, App::view);

  let app = if cfg!(feature = "builtin-font") {
    app.font(include_bytes!("../assets/SourceHanSansCN-Regular.otf"))
  } else {
    app
  };

  app
    .default_font(Font::with_name("Source Han Sans CN"))
    .theme(App::theme)
    .centered()
    .run_with(|| {
      (
        App {
          game_dir: current_dir()
            .expect("wtf current dir")
            .to_string_lossy()
            .to_string(),
          registries,
          ..Default::default()
        },
        Task::batch(init_task_batch),
      )
    })
}

#[derive(Debug, Clone)]
enum Message {
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

#[derive(Debug, Default)]
struct App {
  game_dir: String,
  downloads: Vec<Download>,
  installs: VecDeque<Install>,
  uninstalls: VecDeque<Uninstall>,
  mod_manager: Option<ModManager>,
  current_mods: HashSet<String>,
  install_mods: HashSet<String>,
  uninstall_mods: HashSet<String>,
  registries: VecDeque<Registry>,
  records: Records,
  loading_registry: bool,
  need_current_mods_update: bool,
  need_records_update: bool,
}

impl App {
  fn available_mods(&self) -> Vec<&str> {
    self
      .registries
      .iter()
      .flat_map(|registry| registry.mods.keys().map(|id| id.as_str()))
      .collect::<Vec<_>>()
  }

  fn theme(&self) -> Theme {
    Theme::Nord
  }

  fn title(&self) -> String {
    "战舰世界莱服模组管理器".to_string()
  }

  fn update(&mut self, message: Message) -> Task<Message> {
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
                Task::done(Message::InstallMod { path, id })
              }
            },
          }
        } else {
          Task::none()
        }
      }
      Message::InstallMod { path, id } => {
        let mut install = Install::new(
          id.as_str(),
          path.as_path(),
          self
            .request_mod(id.as_str())
            .map(|m| m.version.to_owned())
            .unwrap_or_default()
            .as_str(),
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
            InstallUpdate::Finished(res) => match res {
              Err(err) => panic!("{}", err),
              Ok(mod_manager) => {
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
            },
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
            UninstallUpdate::Finished(res) => match res {
              Err(err) => panic!("{}", err),
              Ok(mod_manager) => {
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
            },
          }
        } else {
          Task::none()
        }
      }
      Message::ModManagerReady { mod_manager } => loop {
        if let Some(mut uninstall) = self.uninstalls.pop_front() {
          if let &UninstallState::Ready | &UninstallState::Failed =
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
          if let &InstallState::Ready | &InstallState::Failed =
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

  fn view(&self) -> Element<Message> {
    let element: Element<_> = column![]
      .push(text("战舰世界莱服模组管理器"))
      .push(
        row![]
          .push(text("游戏根目录"))
          .push(
            text_input("游戏根目录", &self.game_dir), // .on_input(Message::GameDirInput),
          )
          .align_y(Vertical::Center),
      )
      .push(
        container(
          column![]
            .extend(self.available_mods().iter().map(|modid| {
              let modid = modid.to_owned();
              let Some(modr) = self.request_mod(modid) else {
                return row![].into();
              };
              row![]
                .push(checkbox("", self.current_mods.contains(modid)))
                .push(image(""))
                .push(text(modid).width(Length::Fixed(100.)))
                .push(
                  text(modr.name.as_str()).width(Length::Fixed(100.)),
                )
                .push(
                  text(format!(
                    "{}->{}",
                    self
                      .records
                      .records
                      .get(modid)
                      .map(|x| x.version.to_owned())
                      .unwrap_or_default(),
                    modr.version
                  ))
                  .width(Length::Fixed(100.)),
                )
                .push(
                  progress_bar(0.0..=100., {
                    if let Some(download) =
                      self.downloads.iter().find(|x| x.id() == modid)
                    {
                      match download.state() {
                        DownloadState::Running {
                          progress, ..
                        } => progress * 100.,
                        _ => 100.,
                      }
                    } else {
                      100.
                    }
                  })
                  .length(Length::Fixed(200.)),
                )
                .push(
                  checkbox(
                    "安装/更新",
                    self.install_mods.contains(modid),
                  )
                  .on_toggle(|flag| {
                    if flag {
                      Message::AddInstallMod {
                        id: modid.to_string(),
                      }
                    } else {
                      Message::RemoveInstallMod {
                        id: modid.to_string(),
                      }
                    }
                  }),
                )
                .push(
                  checkbox(
                    "卸载",
                    self.uninstall_mods.contains(modid),
                  )
                  .on_toggle(|flag| {
                    if flag {
                      Message::AddUninstallMod {
                        id: modid.to_string(),
                      }
                    } else {
                      Message::RemoveUninstallMod {
                        id: modid.to_string(),
                      }
                    }
                  }),
                )
                .spacing(5)
                .width(Length::Fill)
                .align_y(Vertical::Center)
                .into()
            }))
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .style(bordered_box)
        .padding(10)
        .width(Length::Fill)
        .height(Length::Fill),
      )
      .push(
        container(
          button("更新模组").on_press(Message::UpdateMods {
            install: self.install_mods.iter().cloned().collect(),
            uninstall: self
              .install_mods
              .iter()
              .cloned()
              .chain(self.uninstall_mods.iter().cloned())
              .collect(),
          }),
        )
        .align_right(Length::Fill),
      )
      .spacing(10)
      .padding(20)
      .into();

    element
    // .explain(Color::BLACK)
  }

  fn request_mod(&self, id: &str) -> Option<&Mod> {
    self
      .registries
      .iter()
      .find_map(|registry| registry.mods.get(id))
  }
}

#[derive(Debug, Clone, Copy)]
struct Progress {
  pub current: u64,
  pub max: u64,
}

#[derive(Debug, thiserror::Error)]
enum Error {
  #[error("Reqwest: {0}")]
  Reqwest(#[from] reqwest::Error),
}
