use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use directories::ProjectDirs;
use tracing_subscriber::{
    fmt::MakeWriter, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

use crate::{
    config::Config,
    error::{AstralError, Result},
};

const LOG_FILE_NAME: &str = "astral.log";
const MAX_LOG_FILE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_LOG_BACKUPS: usize = 5;

pub fn init(config: &Config) -> Result<()> {
    let filter = EnvFilter::try_new(&config.log_filter).map_err(|error| AstralError::Logging {
        message: error.to_string(),
    })?;
    let log_path = default_log_path().map_err(|error| AstralError::Logging {
        message: format!("failed to resolve log path: {error}"),
    })?;
    let file_writer = RollingFile::open(&log_path).map_err(|error| AstralError::Logging {
        message: format!("failed to open log file '{}': {error}", log_path.display()),
    })?;
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_ansi(false)
        .with_writer(std::io::stderr);
    let file_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_target(true)
        .with_current_span(false)
        .with_span_list(false)
        .with_ansi(false)
        .with_writer(file_writer);

    tracing_subscriber::registry()
        .with(filter)
        .with(stderr_layer)
        .with(file_layer)
        .try_init()
        .map_err(|error| AstralError::Logging {
            message: error.to_string(),
        })
}

fn default_log_path() -> io::Result<PathBuf> {
    let data_directory = env::var_os("ASTRAL_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            ProjectDirs::from("com", "astral", "astral")
                .map(|directories| directories.data_dir().to_path_buf())
        })
        .unwrap_or_else(|| PathBuf::from(".astral"));
    fs::create_dir_all(&data_directory)?;
    Ok(data_directory.join(LOG_FILE_NAME))
}

#[derive(Clone)]
struct RollingFile {
    state: Arc<Mutex<RollingFileState>>,
}

struct RollingFileState {
    path: PathBuf,
    max_bytes: u64,
    max_backups: usize,
    file: Option<File>,
    length: u64,
}

impl RollingFile {
    fn open(path: &Path) -> io::Result<Self> {
        Self::open_with_limits(path, MAX_LOG_FILE_BYTES, MAX_LOG_BACKUPS)
    }

    fn open_with_limits(path: &Path, max_bytes: u64, max_backups: usize) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let length = file.metadata()?.len();
        Ok(Self {
            state: Arc::new(Mutex::new(RollingFileState {
                path: path.to_path_buf(),
                max_bytes,
                max_backups,
                file: Some(file),
                length,
            })),
        })
    }
}

struct RollingFileWriter {
    state: Arc<Mutex<RollingFileState>>,
}

impl<'a> MakeWriter<'a> for RollingFile {
    type Writer = RollingFileWriter;

    fn make_writer(&'a self) -> Self::Writer {
        RollingFileWriter {
            state: Arc::clone(&self.state),
        }
    }
}

impl Write for RollingFileWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("log writer lock poisoned"))?;
        state.rotate_if_needed(buffer.len() as u64)?;
        let written = state
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("log file is not open"))?
            .write(buffer)?;
        state.length += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("log writer lock poisoned"))?;
        state
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("log file is not open"))?
            .flush()
    }
}

impl RollingFileState {
    fn rotate_if_needed(&mut self, incoming_bytes: u64) -> io::Result<()> {
        if self.length.saturating_add(incoming_bytes) <= self.max_bytes {
            return Ok(());
        }

        if let Some(file) = self.file.take() {
            file.sync_data()?;
        }
        let expired_backup = backup_path(&self.path, self.max_backups);
        if expired_backup.exists() {
            fs::remove_file(expired_backup)?;
        }
        for index in (1..self.max_backups).rev() {
            let source = backup_path(&self.path, index);
            let target = backup_path(&self.path, index + 1);
            if target.exists() {
                fs::remove_file(&target)?;
            }
            if source.exists() {
                fs::rename(source, target)?;
            }
        }
        let first_backup = backup_path(&self.path, 1);
        if first_backup.exists() {
            fs::remove_file(&first_backup)?;
        }
        if self.path.exists() {
            fs::rename(&self.path, first_backup)?;
        }
        self.file = Some(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?,
        );
        self.length = 0;
        Ok(())
    }
}

fn backup_path(path: &Path, index: usize) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(format!(".{index}"));
    value.into()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::tempdir;
    use tracing_subscriber::fmt::MakeWriter;

    use super::RollingFile;

    #[test]
    fn rotates_and_keeps_configured_backup_count() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("astral.log");
        let rolling = RollingFile::open_with_limits(&path, 10, 5).expect("open log");
        let mut writer = rolling.make_writer();

        writer.write_all(b"first log\n").expect("write first log");
        writer.write_all(b"second log\n").expect("write second log");
        writer.write_all(b"third log\n").expect("write third log");
        writer.write_all(b"fourth log\n").expect("write fourth log");
        writer.write_all(b"fifth log\n").expect("write fifth log");
        writer.write_all(b"sixth log\n").expect("write sixth log");
        writer
            .write_all(b"seventh log\n")
            .expect("write seventh log");

        assert_eq!(
            std::fs::read_to_string(&path).expect("read active log"),
            "seventh log\n"
        );
        let expected_backups = [
            "sixth log\n",
            "fifth log\n",
            "fourth log\n",
            "third log\n",
            "second log\n",
        ];
        for index in 1..=5 {
            assert_eq!(
                std::fs::read_to_string(format!("{}.{}", path.display(), index))
                    .expect("read log backup"),
                expected_backups[index - 1]
            );
        }
        assert!(!std::path::Path::new(&format!("{}.6", path.display())).exists());
    }
}
