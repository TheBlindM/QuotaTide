use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::backup::Backup;
use rusqlite::{Connection, OpenFlags};
use thiserror::Error;

const BACKUP_LIMIT: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreflightDisposition {
    New,
    Ready,
    BackedUp,
    Recovered,
}

#[derive(Debug, Error)]
pub(crate) enum LocalDataError {
    #[error("database schema is newer than this application")]
    UnsupportedSchema,
    #[error("no valid local database backup is available")]
    RecoveryRequired,
    #[error("local data preparation failed")]
    Io(#[from] std::io::Error),
    #[error("local database preparation failed")]
    Sqlite(#[from] rusqlite::Error),
}

pub(crate) fn prepare_database(
    database_path: &Path,
    supported_schema: i64,
) -> Result<PreflightDisposition, LocalDataError> {
    let metadata = match fs::symlink_metadata(database_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PreflightDisposition::New);
        }
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "database path is not a regular file",
        )
        .into());
    }
    match inspect_read_only(database_path, supported_schema) {
        Ok(version) => {
            if version == 0 || version == supported_schema {
                return Ok(PreflightDisposition::Ready);
            }
            create_validated_backup(database_path, version)?;
            Ok(PreflightDisposition::BackedUp)
        }
        Err(LocalDataError::UnsupportedSchema) => Err(LocalDataError::UnsupportedSchema),
        Err(_) => recover_from_backups(database_path, supported_schema),
    }
}

pub(crate) fn secure_database_artifacts(database_path: &Path) -> Result<(), LocalDataError> {
    for artifact in database_artifacts(database_path) {
        if artifact.exists() {
            secure_file(&artifact)?;
        }
    }
    Ok(())
}

pub(crate) fn begin_recovery(
    database_path: &Path,
    supported_schema: i64,
) -> Result<Vec<PathBuf>, LocalDataError> {
    isolate_database(database_path)?;
    let backup_directory = sibling_directory(database_path, "backups")?;
    let mut candidates = Vec::new();
    for backup in sorted_backups(&backup_directory)?.into_iter().rev() {
        if inspect_read_only(&backup, supported_schema).is_ok() {
            candidates.push(backup);
        }
    }
    Ok(candidates)
}

pub(crate) fn restore_backup(database_path: &Path, backup: &Path) -> Result<(), LocalDataError> {
    discard_database_artifacts(database_path)?;
    fs::copy(backup, database_path)?;
    secure_file(database_path)?;
    Ok(())
}

pub(crate) fn discard_database_artifacts(database_path: &Path) -> Result<(), LocalDataError> {
    for artifact in database_artifacts(database_path) {
        if artifact.exists() {
            fs::remove_file(artifact)?;
        }
    }
    Ok(())
}

fn inspect_read_only(path: &Path, supported_schema: i64) -> Result<i64, LocalDataError> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > supported_schema {
        return Err(LocalDataError::UnsupportedSchema);
    }
    ensure_integrity(&connection, "quick_check")?;
    Ok(version)
}

fn create_validated_backup(database_path: &Path, schema: i64) -> Result<(), LocalDataError> {
    let backup_directory = sibling_directory(database_path, "backups")?;
    fs::create_dir_all(&backup_directory)?;
    secure_directory(&backup_directory)?;
    let backup_path =
        backup_directory.join(format!("state-v{schema}-{}.sqlite3", timestamp_millis()));
    let source = Connection::open(database_path)?;
    let mut destination = Connection::open(&backup_path)?;
    Backup::new(&source, &mut destination)?.run_to_completion(
        64,
        std::time::Duration::from_millis(10),
        None,
    )?;
    drop(destination);
    secure_file(&backup_path)?;
    let verified = Connection::open_with_flags(&backup_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    if let Err(error) = ensure_integrity(&verified, "integrity_check") {
        let _ = fs::remove_file(&backup_path);
        return Err(error);
    }
    rotate_backups(&backup_directory)?;
    Ok(())
}

fn recover_from_backups(
    database_path: &Path,
    supported_schema: i64,
) -> Result<PreflightDisposition, LocalDataError> {
    let candidates = begin_recovery(database_path, supported_schema)?;
    for backup in candidates {
        restore_backup(database_path, &backup)?;
        if inspect_read_only(database_path, supported_schema).is_ok() {
            return Ok(PreflightDisposition::Recovered);
        }
        let _ = discard_database_artifacts(database_path);
    }
    Err(LocalDataError::RecoveryRequired)
}

fn isolate_database(database_path: &Path) -> Result<(), LocalDataError> {
    let recovery_root = sibling_directory(database_path, "recovery")?;
    fs::create_dir_all(&recovery_root)?;
    secure_directory(&recovery_root)?;
    let destination = recovery_root.join(format!("state-corrupt-{}", timestamp_millis()));
    fs::create_dir_all(&destination)?;
    secure_directory(&destination)?;
    for path in database_artifacts(database_path) {
        if path.exists() {
            let file_name = path
                .file_name()
                .ok_or_else(|| std::io::Error::other("database artifact has no file name"))?;
            fs::rename(&path, destination.join(file_name))?;
        }
    }
    Ok(())
}

fn rotate_backups(directory: &Path) -> Result<(), LocalDataError> {
    let backups = sorted_backups(directory)?;
    let remove_count = backups.len().saturating_sub(BACKUP_LIMIT);
    for backup in backups.into_iter().take(remove_count) {
        fs::remove_file(backup)?;
    }
    Ok(())
}

fn sorted_backups(directory: &Path) -> Result<Vec<PathBuf>, LocalDataError> {
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut backups = fs::read_dir(directory)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let file_type = entry.file_type().ok()?;
            (file_type.is_file() && !file_type.is_symlink()).then(|| entry.path())
        })
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("state-v") && name.ends_with(".sqlite3"))
        })
        .collect::<Vec<_>>();
    backups.sort();
    Ok(backups)
}

fn ensure_integrity(connection: &Connection, pragma: &str) -> Result<(), LocalDataError> {
    let statement = format!("PRAGMA {pragma}");
    let result: String = connection.query_row(&statement, [], |row| row.get(0))?;
    if result == "ok" {
        Ok(())
    } else {
        Err(LocalDataError::Sqlite(rusqlite::Error::InvalidQuery))
    }
}

fn sibling_directory(database_path: &Path, name: &str) -> Result<PathBuf, LocalDataError> {
    database_path
        .parent()
        .map(|parent| parent.join(name))
        .ok_or_else(|| std::io::Error::other("database has no parent directory").into())
}

fn database_artifacts(database_path: &Path) -> [PathBuf; 3] {
    [
        database_path.to_path_buf(),
        database_path.with_file_name(format!(
            "{}-wal",
            database_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("state.sqlite3")
        )),
        database_path.with_file_name(format!(
            "{}-shm",
            database_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("state.sqlite3")
        )),
    ]
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}

#[cfg(unix)]
fn secure_directory(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "local data directory is not a regular directory",
        ));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn secure_directory(path: &Path) -> std::io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "local data directory is not a regular directory",
        ))
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn secure_file(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "local data file is not a regular file",
        ));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn secure_file(path: &Path) -> std::io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "local data file is not a regular file",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validated_backups_rotate_only_after_the_new_copy_exists() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let database = directory.path().join("state.sqlite3");
        let connection = Connection::open(&database).expect("create database");
        connection
            .execute_batch(
                "CREATE TABLE facts (id INTEGER PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO facts (value) VALUES ('preserved');",
            )
            .expect("seed database");
        drop(connection);

        for _ in 0..4 {
            create_validated_backup(&database, 10).expect("create backup");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }

        let backups = sorted_backups(&directory.path().join("backups")).expect("list backups");
        assert_eq!(backups.len(), 3);
        for backup in backups {
            let connection = Connection::open_with_flags(backup, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .expect("open backup");
            ensure_integrity(&connection, "integrity_check").expect("valid backup");
            assert_eq!(
                connection
                    .query_row("SELECT value FROM facts", [], |row| row.get::<_, String>(0))
                    .expect("read fact"),
                "preserved"
            );
        }
    }
}
