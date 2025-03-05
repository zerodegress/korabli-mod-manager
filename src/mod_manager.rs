use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  time::{SystemTime, SystemTimeError, UNIX_EPOCH},
};

use futures::FutureExt;
use serde::{Deserialize, Serialize};
use tokio::{fs, io::AsyncWriteExt};
use tokio_util::compat::TokioAsyncReadCompatExt;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Metadata {}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Record {
  pub metadata: Option<Metadata>,
  pub update_time: u64,
  pub version: String,
  pub files: Vec<PathBuf>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Records {
  #[serde(flatten)]
  pub records: HashMap<String, Record>,
}

#[derive(Debug, Clone)]
pub struct ModManager {
  res_mods_path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("SerdeJson: {0}")]
  SerdeJson(#[from] serde_json::Error),
  #[error("Io: {0}")]
  Io(#[from] std::io::Error),
  #[error("AsyncZip: {0}")]
  AsyncZip(#[from] async_zip::error::ZipError),
  #[error("SystemTime: {0}")]
  SystemTime(#[from] SystemTimeError),
  #[error("FileCoflict: files: {file}")]
  FileConflict { file: PathBuf },
  #[error("ResModsDirNotFound: {game_dir_path}")]
  ResModsDirNotFound { game_dir_path: PathBuf },
}

impl ModManager {
  pub fn try_from_game_dir(
    game_dir_path: &Path,
  ) -> Result<Self, Error> {
    #[allow(clippy::manual_try_fold)]
    let dir = std::fs::read_dir(game_dir_path.join("bin"))?.fold(
      Err(Error::ResModsDirNotFound {
        game_dir_path: game_dir_path.to_path_buf(),
      }),
      |x, y| {
        let y = y?;
        let Ok(y_num) =
          y.file_name().to_string_lossy().to_string().parse::<u64>()
        else {
          return x;
        };
        let Ok(x) = x else {
          return Ok(y);
        };
        let Ok(x_num) =
          x.file_name().to_string_lossy().to_string().parse::<u64>()
        else {
          return Ok(y);
        };
        Ok(if x_num > y_num { x } else { y })
      },
    )?;
    Ok(Self {
      res_mods_path: dir.path().join("res_mods"),
    })
  }

  pub async fn ensure_records(&mut self) -> Result<(), Error> {
    let mut file = match fs::File::options()
      .create_new(true)
      .write(true)
      .open(self.res_mods_path.join(".kmmgr.json"))
      .await
    {
      Err(err) => {
        return match err.kind() {
          std::io::ErrorKind::AlreadyExists => Ok(()),
          _ => Err(err.into()),
        };
      }
      Ok(file) => file,
    };
    file
      .write_all(serde_json::to_vec(&Records::default())?.as_slice())
      .await?;
    Ok(())
  }

  pub async fn records(&self) -> Result<Records, Error> {
    Ok(serde_json::from_slice(
      fs::read(self.res_mods_path.join(".kmmgr.json"))
        .await?
        .as_slice(),
    )?)
  }

  async fn write_records(
    &mut self,
    records: &Records,
  ) -> Result<(), Error> {
    fs::write(
      self.res_mods_path.join(".kmmgr.json"),
      serde_json::to_vec(&records)?,
    )
    .await?;
    Ok(())
  }

  pub async fn install_zip_mod(
    &mut self,
    mod_path: &Path,
    id: &str,
    version: &str,
  ) -> Result<(), Error> {
    let mut record = Record {
      metadata: None,
      version: version.to_string(),
      update_time: SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs(),
      files: Vec::new(),
    };
    let zip_mod =
      async_zip::tokio::read::fs::ZipFileReader::new(mod_path)
        .await?;
    let mut tasks = Vec::new();

    for (index, entry) in zip_mod.file().entries().iter().enumerate()
    {
      let sanitized_file_path =
        sanitize_file_path(entry.filename().as_str().unwrap());

      record.files.push(sanitized_file_path.to_owned());

      let path =
        self.res_mods_path.join(sanitized_file_path.as_path());

      if entry.dir()? {
        if path.exists() {
          continue;
        }
        tasks.push(
          async move {
            fs::create_dir_all(path).await?;
            Ok(())
          }
          .boxed(),
        );
      } else {
        if path.exists() {
          return Err(Error::FileConflict {
            file: sanitized_file_path.to_owned(),
          });
        }

        let mut reader = zip_mod.reader_without_entry(index).await?;

        tasks.push(
          async move {
            let mut writer = fs::File::options()
              .create(true)
              .truncate(true)
              .write(true)
              .open(path)
              .await?
              .compat();
            futures::io::copy(&mut reader, &mut writer).await?;
            Ok::<(), Error>(())
          }
          .boxed(),
        );
      }
    }

    for task in tasks {
      task.await?;
    }
    let mut records = self.records().await?;

    records.records.insert(id.to_owned(), record);

    self.write_records(&records).await?;

    Ok(())
  }

  pub async fn uninstall_mod(
    &mut self,
    id: &str,
  ) -> Result<bool, Error> {
    let mut records = self.records().await?;
    let Some(record) = records.records.get(id) else {
      records.records.remove(id);
      self.write_records(&records).await?;
      return Ok(false);
    };

    for file_path in record.files.iter() {
      let file_path = self.res_mods_path.join(file_path.as_path());
      if !file_path.exists() {
        continue;
      }

      if file_path.is_dir() {
        // TODO: 最好还是清理一下文件夹
        continue;
      }
      fs::remove_file(file_path.as_path()).await?;
    }

    Ok(true)
  }
}

fn sanitize_file_path(path: &str) -> PathBuf {
  // Replaces backwards slashes
  path
    .replace('\\', "/")
    // Sanitizes each component
    .split('/')
    .map(sanitize_filename::sanitize)
    .collect()
}
