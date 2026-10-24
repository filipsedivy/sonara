//! The desktop's logger: the prototype's `_configure_logging` (`sonara.ui.app`), which logs to
//! stderr and to `logs_dir()/sonara.log` with the format
//! `%(asctime)s %(levelname)s %(name)s: %(message)s`.
//!
//! The log file is appended to; when it cannot be opened, the logger writes to stderr only, as
//! the prototype's suppressed `OSError` does. The time is local, `2026-10-22 13:45:01,123`.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

use sonara_app::log_level::{level_filter, level_name};
use sonara_app::paths::logs_dir;

/// Writes every log line to stderr and, when it could be opened, to the log file.
struct Tee {
    file: Option<File>,
}

impl Write for Tee {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Some(file) = &mut self.file {
            // ERR-06: a full disk must not silence stderr; the line still reaches it.
            let _ = file.write_all(buf);
        }
        io::stderr().write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(file) = &mut self.file {
            let _ = file.flush();
        }
        io::stderr().flush()
    }
}

/// The log file, `logs_dir()/sonara.log`, opened for appending; `None` (with the reason on
/// stderr) when it cannot be.
fn open_log_file() -> Option<(PathBuf, File)> {
    let opened = logs_dir()
        .map_err(|error| error.to_string())
        .and_then(|dir| {
            fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
            let path = dir.join("sonara.log");
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map(|file| (path, file))
                .map_err(|error| error.to_string())
        });
    match opened {
        Ok(opened) => Some(opened),
        Err(reason) => {
            eprintln!("sonara-desktop: logging to stderr only: {reason}");
            None
        }
    }
}

/// Installs the logger at `level` (a Python level name) and returns the log file's path.
///
/// # Errors
///
/// `Unknown level: 'NAME'` for an unknown level name.
pub fn init(level: &str) -> Result<Option<PathBuf>, String> {
    let filter = level_filter(level).map_err(|error| error.to_string())?;
    let (path, file) = open_log_file().unzip();
    let target = env_logger::Target::Pipe(Box::new(Tee { file }));
    // ERR-06: an error only means that a logger is installed already.
    let _ = env_logger::Builder::new()
        .filter_level(filter)
        .target(target)
        .format(|out, record| {
            let now = jiff::Zoned::now();
            writeln!(
                out,
                "{},{:03} {} {}: {}",
                now.strftime("%Y-%m-%d %H:%M:%S"),
                now.millisecond(),
                level_name(record.level()),
                record.target(),
                record.args()
            )
        })
        .try_init();
    Ok(path)
}
