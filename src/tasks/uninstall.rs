use std::sync::Arc;

use iced::{
  Task,
  task::{self, Straw, sipper},
};

use crate::{
  data::progress::Progress,
  mod_manager::{self, ModManager},
};

#[derive(Debug, Clone)]
pub struct Uninstall {
  id: String,
  state: UninstallState,
}

#[derive(Debug, Clone)]
pub enum UninstallState {
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
pub enum UninstallUpdate {
  Running(Progress),
  Finished((Result<(), Error>, ModManager)),
}

impl Uninstall {
  pub fn new(id: &str) -> Self {
    Self {
      id: id.to_string(),
      state: UninstallState::Ready,
    }
  }

  pub fn id(&self) -> &str {
    &self.id
  }

  pub fn state(&self) -> &UninstallState {
    &self.state
  }

  pub fn start(
    &mut self,
    mod_manager: ModManager,
  ) -> Task<UninstallUpdate> {
    match self.state {
      UninstallState::Failed
      | UninstallState::Finished
      | UninstallState::Ready => {
        let (task, handle) = Task::sip(
          uninstall_mod(self.id.to_owned(), mod_manager),
          UninstallUpdate::Running,
          |res| match res {
            Err((err, mod_manager)) => {
              UninstallUpdate::Finished((Err(err), mod_manager))
            }
            Ok(mod_manager) => {
              UninstallUpdate::Finished((Ok(()), mod_manager))
            }
          },
        )
        .abortable();
        self.state = UninstallState::Running {
          progress: 0.,
          _task_handle: handle,
        };
        task
      }
      UninstallState::Running { .. } => Task::none(),
    }
  }

  pub fn update(&mut self, update: UninstallUpdate) {
    if let UninstallState::Running { progress, .. } = &mut self.state
    {
      match update {
        UninstallUpdate::Running(new_progress) => {
          *progress = if new_progress.max == 0 {
            -1.
          } else {
            new_progress.current as f32 / new_progress.max as f32
          };
        }
        UninstallUpdate::Finished((res, ..)) => {
          self.state = if res.is_ok() {
            UninstallState::Finished
          } else {
            UninstallState::Failed
          }
        }
      }
    }
  }
}

fn uninstall_mod(
  id: String,
  mut mod_manager: ModManager,
) -> impl Straw<ModManager, Progress, (Error, ModManager)> {
  sipper(async move |progress| {
    mod_manager.uninstall_mod(&id).await.map_err(|err| {
      (Error::ModManager(Arc::new(err)), mod_manager.to_owned())
    })?;
    Ok(mod_manager)
  })
}
