use std::{path::PathBuf, sync::Arc};

use futures::StreamExt;
use iced::{
  task::{self, sipper, Straw},
  Task,
};
use tokio::{fs, io::AsyncWriteExt};
use url::Url;

use crate::Progress;

#[derive(Debug, Clone)]
pub struct Download {
  url: Url,
  id: String,
  state: DownloadState,
}

#[derive(Debug, thiserror::Error, Clone)]
pub enum Error {
  #[error("Reqwest: {0}")]
  Reqwest(#[from] Arc<reqwest::Error>),
  #[error("Io: {0}")]
  Io(#[from] Arc<std::io::Error>),
}

#[derive(Debug, Clone)]
pub enum DownloadState {
  Running {
    progress: f32,
    _task_handle: task::Handle,
  },
  Finished,
  Failed,
  Ready,
}

#[derive(Debug, Clone)]
pub enum DownloadUpdate {
  Downloading(Progress),
  Finished(Result<PathBuf, Error>),
}

impl Download {
  pub fn new(id: String, url: Url) -> Self {
    Self {
      url,
      id,
      state: DownloadState::Ready,
    }
  }

  pub fn state(&self) -> &DownloadState {
    &self.state
  }

  pub fn id(&self) -> &str {
    &self.id
  }

  pub fn start(&mut self) -> Task<DownloadUpdate> {
    match self.state {
      DownloadState::Failed
      | DownloadState::Ready
      | DownloadState::Finished => {
        let (task, handle) = Task::sip(
          download_to(
            self.url.to_owned(),
            temp_file::empty().path().to_path_buf(),
          ),
          DownloadUpdate::Downloading,
          DownloadUpdate::Finished,
        )
        .abortable();

        self.state = DownloadState::Running {
          progress: 0.,
          _task_handle: handle.abort_on_drop(),
        };

        task
      }
      DownloadState::Running { .. } => Task::none(),
    }
  }

  pub fn update(&mut self, update: DownloadUpdate) {
    if let DownloadState::Running { progress, .. } = &mut self.state {
      match update {
        DownloadUpdate::Downloading(new_progress) => {
          *progress = if new_progress.max == 0 {
            -1.
          } else {
            new_progress.current as f32 / new_progress.max as f32
          };
        }
        DownloadUpdate::Finished(res) => {
          self.state = if res.is_ok() {
            DownloadState::Finished
          } else {
            DownloadState::Failed
          }
        }
      }
    }
  }
}

fn download_to(
  url: Url,
  path: PathBuf,
) -> impl Straw<PathBuf, Progress, Error> {
  sipper(move |mut progress| async move {
    let res = reqwest::get(url).await.map_err(Arc::new)?;
    let mut current = 0;
    let max = res.content_length().unwrap_or(0);
    progress.send(Progress { current, max }).await;
    let mut reader_stream = res.bytes_stream();

    let mut writer = fs::File::options()
      .create(true)
      .truncate(true)
      .write(true)
      .open(&path)
      .await
      .map_err(Arc::new)?;

    while let Some(bytes) = reader_stream.next().await {
      let bytes = bytes.map_err(Arc::new)?;
      current += bytes.len() as u64;
      writer.write_all(&bytes).await.map_err(Arc::new)?;
      progress.send(Progress { current, max }).await;
    }
    Ok(path)
  })
}
