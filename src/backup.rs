use std::{
    error::Error,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OpenFlags};
use tempfile::NamedTempFile;

const DAY: i64 = 86_400;
const NIGHT: i64 = 2 * 60 * 60;
const KEEP: usize = 7;
const PREFIX: &str = "kringkring-";
const SUFFIX: &str = ".sqlite3";

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

pub async fn run(db: PathBuf, directory: PathBuf) {
    let mut ticks = tokio::time::interval(Duration::from_secs(60));
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        ticks.tick().await;
        let db = db.clone();
        let directory = directory.clone();
        let result = tokio::task::spawn_blocking(move || {
            let now = i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())?;
            snapshot_due(&db, &directory, now)
        })
        .await;
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => eprintln!("Nightly backup failed: {error}"),
            Err(error) => eprintln!("Nightly backup task failed: {error}"),
        }
    }
}

fn snapshot_due(path: &Path, directory: &Path, now: i64) -> Result<()> {
    let source = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let slot = (now - NIGHT).div_euclid(DAY) * DAY + NIGHT;
    let date: String =
        source.query_row("SELECT date(?1, 'unixepoch')", [slot], |row| row.get(0))?;
    create_directory(directory)?;
    let destination = directory.join(format!("{PREFIX}{date}{SUFFIX}"));
    match fs::symlink_metadata(&destination) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Err("backup name is occupied by a non-file".into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let temp = NamedTempFile::new_in(directory)?;
            let location = temp
                .path()
                .to_str()
                .ok_or("backup path is not valid UTF-8")?;
            source.execute("VACUUM INTO ?1", [location])?;
            temp.as_file().sync_all()?;
            // Publish only complete snapshots, without replacing a same-night backup.
            match temp.persist_noclobber(&destination) {
                Ok(_) => {}
                Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err(error) => return Err(error.into()),
    }
    #[cfg(unix)]
    File::open(directory)?.sync_all()?;
    rotate(&source, directory)
}

fn create_directory(path: &Path) -> Result<()> {
    let mut directory = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        directory.mode(0o700);
    }
    match directory.create(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            if fs::symlink_metadata(path)?.is_dir() {
                Ok(())
            } else {
                Err("backup directory is not a real directory".into())
            }
        }
        Err(error) => Err(error.into()),
    }
}

fn rotate(source: &Connection, directory: &Path) -> Result<()> {
    let mut snapshots = Vec::new();
    let mut valid_day = source.prepare("SELECT COALESCE(date(?1, '+0 days') = ?1, 0)")?;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        if let Some(date) = name
            .to_str()
            .and_then(|name| name.strip_prefix(PREFIX)?.strip_suffix(SUFFIX))
            && date.len() == 10
            && entry.file_type()?.is_file()
            && valid_day.query_row([date], |row| row.get::<_, bool>(0))?
        {
            snapshots.push(entry.path());
        }
    }
    snapshots.sort_unstable();
    let excess = snapshots.len().saturating_sub(KEEP);
    for path in snapshots.into_iter().take(excess) {
        if let Err(error) = fs::remove_file(path)
            && error.kind() != io::ErrorKind::NotFound
        {
            return Err(error.into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
