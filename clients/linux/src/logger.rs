//! Linux rotating diagnostic log sink.

use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

use mailcal_bindings::{LogLevel, Logger};
use time::{OffsetDateTime, UtcOffset};

const DEFAULT_MAX_BYTES: u64 = 1024 * 1024;
const DEFAULT_BACKUPS: u8 = 3;

/// The Linux diagnostic log path from the cross-platform logging contract.
pub(crate) fn diagnostic_log_path() -> PathBuf {
    gtk::glib::user_data_dir().join("mailcal/mailcal.log")
}

#[derive(Clone, Copy, Debug)]
struct RotationPolicy {
    max_bytes: u64,
    backups: u8,
}

impl Default for RotationPolicy {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
            backups: DEFAULT_BACKUPS,
        }
    }
}

/// Thread-safe, best-effort rotating file sink for core diagnostics.
#[derive(Debug)]
pub(crate) struct FileLogger {
    path: PathBuf,
    policy: RotationPolicy,
    gate: Mutex<()>,
}

impl FileLogger {
    /// Opens the standard XDG log path and writes this process's session marker.
    pub(crate) fn new() -> Self {
        Self::with_policy(diagnostic_log_path(), RotationPolicy::default())
    }

    fn with_policy(path: PathBuf, policy: RotationPolicy) -> Self {
        let logger = Self {
            path,
            policy,
            gate: Mutex::new(()),
        };
        let marker = format!(
            "{} INFO [mailcal_linux] {}\n",
            timestamp(),
            session_marker(
                env!("CARGO_PKG_VERSION"),
                env!("MAILCAL_BUILD_ID"),
                std::env::consts::ARCH,
            )
        );
        logger.write(&marker);
        logger
    }

    fn write(&self, line: &str) {
        let Ok(_guard) = self.gate.lock() else {
            return;
        };
        if let Some(parent) = self.path.parent()
            && fs::create_dir_all(parent).is_err()
        {
            return;
        }
        if self.should_rotate() {
            self.rotate();
        }
        let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        else {
            return;
        };
        let _ = file.write_all(line.as_bytes());
    }

    fn should_rotate(&self) -> bool {
        fs::metadata(&self.path).is_ok_and(|metadata| metadata.len() >= self.policy.max_bytes)
    }

    fn rotate(&self) {
        for index in (1..=self.policy.backups).rev() {
            let source = if index == 1 {
                self.path.clone()
            } else {
                backup_path(&self.path, index - 1)
            };
            let destination = backup_path(&self.path, index);
            let _ = fs::remove_file(&destination);
            let _ = fs::rename(source, destination);
        }
    }
}

fn session_marker(version: &str, build: &str, arch: &str) -> String {
    format!("--- session start ({version} build {build}, linux/{arch}) ---")
}

impl Logger for FileLogger {
    fn log(&self, level: LogLevel, target: String, message: String) {
        let line = format!(
            "{} {} [{target}] {message}\n",
            timestamp(),
            level_name(level)
        );
        self.write(&line);
    }
}

fn backup_path(path: &Path, index: u8) -> PathBuf {
    let mut value: OsString = path.as_os_str().to_owned();
    value.push(format!(".{index}"));
    value.into()
}

fn level_name(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Error => "ERROR",
        LogLevel::Warn => "WARN",
        LogLevel::Info => "INFO",
        LogLevel::Debug => "DEBUG",
        LogLevel::Trace => "TRACE",
    }
}

fn timestamp() -> String {
    let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let now = OffsetDateTime::now_utc().to_offset(offset);
    let offset_seconds = offset.whole_seconds();
    let offset_sign = if offset_seconds < 0 { '-' } else { '+' };
    let offset_minutes = offset_seconds.unsigned_abs() / 60;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}{offset_sign}{:02}:{:02}",
        now.year(),
        now.month() as u8,
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
        now.millisecond(),
        offset_minutes / 60,
        offset_minutes % 60,
    )
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use mailcal_bindings::{LogLevel, Logger};

    use super::{FileLogger, RotationPolicy, session_marker};

    #[test]
    fn the_session_marker_names_the_source_build() {
        assert_eq!(
            session_marker("0.5.0", "7cc762f1.1787600400", "x86_64"),
            "--- session start (0.5.0 build 7cc762f1.1787600400, linux/x86_64) ---"
        );
        let build = env!("MAILCAL_BUILD_ID");
        let (fingerprint, epoch) = build.split_once('.').expect("fingerprint.build-epoch");
        assert!(fingerprint.len() >= 12);
        assert!(fingerprint.chars().all(|value| value.is_ascii_hexdigit()));
        assert!(epoch.chars().all(|value| value.is_ascii_digit()));
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("mailcal-linux-{name}-{nonce}"));
        fs::create_dir_all(&dir).expect("create scratch directory");
        dir.join("mailcal.log")
    }

    #[test]
    fn rotates_before_writing_and_keeps_exactly_three_backups() {
        let path = scratch("rotation");
        let logger = FileLogger::with_policy(
            path.clone(),
            RotationPolicy {
                max_bytes: 160,
                backups: 3,
            },
        );

        for index in 0..40 {
            logger.log(
                LogLevel::Info,
                "mailcal_linux::test".to_owned(),
                format!("record-{index:02}-with-enough-bytes-to-rotate"),
            );
        }

        assert!(path.exists(), "the live log remains present");
        for backup in 1..=3 {
            assert!(
                path.with_extension(format!("log.{backup}")).exists(),
                "backup {backup} exists"
            );
        }
        assert!(
            !path.with_extension("log.4").exists(),
            "the fourth backup is always dropped"
        );
        let live = fs::read_to_string(&path).expect("read live log");
        assert!(live.contains("record-39"), "the newest record stays live");
    }
}
