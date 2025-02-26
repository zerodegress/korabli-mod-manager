use std::{
  collections::{HashSet, VecDeque},
  env::current_dir,
};

use crate::mod_manager::{ModManager, Records};
use crate::tasks::download::{Download, DownloadState};
use crate::tasks::install::Install;
use crate::tasks::uninstall::Uninstall;
use crate::{
  data::registry::{Mod, Registry},
  messages::Message,
};
use iced::{
  Element, Font, Length, Task, Theme,
  alignment::Vertical,
  widget::{
    button, checkbox, column, container, container::bordered_box,
    image, progress_bar, row, text, text_input,
  },
};
use url::Url;

mod update;

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
