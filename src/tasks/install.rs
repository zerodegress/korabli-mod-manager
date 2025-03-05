use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use iced::{
  Task,
  task::{self, Straw, sipper},
};

use crate::{
  data::progress::Progress,
  mod_manager::{self, ModManager},
};

#[derive(Debug, Clone)]
pub struct Install {
  id: String,
  path: PathBuf,
  version: String,
  state: InstallState,
  ty: String,
}

#[derive(Debug, Clone)]
pub enum InstallState {
  Running {
    progress: f32,
    _task_handle: task::Handle,
  },
  Failed,
  Finished,
  Ready,
}

#[derive(Debug, thiserror::Error, Clone)]
pub enum Error {
  #[error("ModManager: {0}")]
  ModManager(#[from] Arc<mod_manager::Error>),
}

#[derive(Debug, Clone)]
pub enum InstallUpdate {
  Running(Progress),
  Finished((Result<(), Error>, ModManager)),
}

impl Install {
  pub fn new(id: &str, path: &Path, version: &str, ty: &str) -> Self {
    Self {
      id: id.to_string(),
      path: path.to_path_buf(),
      version: version.to_string(),
      state: InstallState::Ready,
      ty: ty.to_string(),
    }
  }

  pub fn state(&self) -> &InstallState {
    &self.state
  }

  pub fn id(&self) -> &str {
    &self.id
  }

  pub fn start(
    &mut self,
    mod_manager: ModManager,
  ) -> Task<InstallUpdate> {
    match self.state {
      InstallState::Failed
      | InstallState::Finished
      | InstallState::Ready => {
        let (task, handle) = Task::sip(
          install_mod(
            self.id.to_owned(),
            self.path.to_owned(),
            self.version.to_owned(),
            self.ty.to_owned(),
            mod_manager,
          ),
          InstallUpdate::Running,
          |res| {
            InstallUpdate::Finished(match res {
              Ok(mod_manager) => (Ok(()), mod_manager),
              Err((err, mod_manager)) => (Err(err), mod_manager),
            })
          },
        )
        .abortable();
        self.state = InstallState::Running {
          progress: 0.,
          _task_handle: handle,
        };
        task
      }
      InstallState::Running { .. } => Task::none(),
    }
  }

  pub fn update(&mut self, update: InstallUpdate) {
    if let InstallState::Running { progress, .. } = &mut self.state {
      match update {
        InstallUpdate::Running(new_progress) => {
          *progress = if new_progress.max == 0 {
            -1.
          } else {
            new_progress.current as f32 / new_progress.max as f32
          };
        }
        InstallUpdate::Finished((res, ..)) => {
          self.state = if res.is_ok() {
            InstallState::Finished
          } else {
            InstallState::Failed
          }
        }
      }
    }
  }
}

fn install_mod(
  id: String,
  path: PathBuf,
  version: String,
  ty: String,
  mut mod_manager: ModManager,
) -> impl Straw<ModManager, Progress, (Error, ModManager)> {
  sipper(async move |progress| {
    match ty.as_str() {
      "zip" => mod_manager.install_zip_mod(
        path.as_ref(),
        id.as_ref(),
        version.as_ref(),
      ),
      _ => todo!(),
    }
    .await
    .map_err(|err| {
      (Error::ModManager(Arc::new(err)), mod_manager.to_owned())
    })?;
    Ok(mod_manager)
  })
}
