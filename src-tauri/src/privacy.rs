//! Strictly allowlisted local logging, diagnostics, and scoped deletion.

use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const LOG_FILE_LIMIT: u64 = 1024 * 1024;
const LOG_FILE_COUNT: usize = 5;
const CLEAR_MARKER: &str = "clear-requested";
static SAFE_LOGGER: OnceLock<SafeLogWriter> = OnceLock::new();
const FORBIDDEN_DIAGNOSTIC_TERMS: &[&str] = &[
    "access_token",
    "refresh_token",
    "id_token",
    "authorization",
    "cookie",
    "account_id",
    "user_id",
    "app_instance_id",
    "credential_ref",
    "password",
    "auth_path",
    "path_summary",
    "account_label",
    "from_address",
    "from_name",
    "recipient",
    "smtp_host",
    "smtp_username",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SafeLogLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeLogFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_error_code: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempt_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings_revision: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dashboard_revision: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_fingerprint: Option<&'static str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SafeLogRecord<'a> {
    timestamp_unix_ms: u128,
    level: SafeLogLevel,
    component: &'a str,
    operation: &'a str,
    #[serde(flatten)]
    fields: SafeLogFields,
}

#[derive(Debug)]
pub struct SafeLogWriter {
    directory: PathBuf,
    enabled: Mutex<bool>,
}

impl SafeLogWriter {
    pub fn new(app_data: &Path) -> std::io::Result<Self> {
        let directory = app_data.join("logs");
        fs::create_dir_all(&directory)?;
        secure_directory(&directory)?;
        Ok(Self {
            directory,
            enabled: Mutex::new(true),
        })
    }

    pub fn record(
        &self,
        level: SafeLogLevel,
        component: &str,
        operation: &str,
        fields: SafeLogFields,
    ) {
        let Ok(mut enabled) = self.enabled.lock() else {
            return;
        };
        if !*enabled {
            return;
        }
        let record = SafeLogRecord {
            timestamp_unix_ms: timestamp_millis(),
            level,
            component,
            operation,
            fields,
        };
        let result = serde_json::to_vec(&record)
            .map_err(std::io::Error::other)
            .and_then(|mut line| {
                line.push(b'\n');
                scan_bytes(&line, &[])?;
                self.append_line(&line)
            });
        if result.is_err() {
            *enabled = false;
        }
    }

    fn append_line(&self, line: &[u8]) -> std::io::Result<()> {
        if u64::try_from(line.len()).unwrap_or(u64::MAX) > LOG_FILE_LIMIT {
            return Err(std::io::Error::other(
                "safe log record exceeds the file limit",
            ));
        }
        let current = self.directory.join("quotatide.jsonl");
        let current_size = current.metadata().map_or(0, |metadata| metadata.len());
        if current_size.saturating_add(u64::try_from(line.len()).unwrap_or(u64::MAX))
            > LOG_FILE_LIMIT
        {
            self.rotate()?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&current)?;
        secure_file(&current)?;
        file.write_all(line)
    }

    fn rotate(&self) -> std::io::Result<()> {
        let oldest = self
            .directory
            .join(format!("quotatide.{}.jsonl", LOG_FILE_COUNT - 1));
        if oldest.exists() {
            fs::remove_file(oldest)?;
        }
        for generation in (1..LOG_FILE_COUNT - 1).rev() {
            let source = self.directory.join(format!("quotatide.{generation}.jsonl"));
            if source.exists() {
                let destination = self
                    .directory
                    .join(format!("quotatide.{}.jsonl", generation + 1));
                fs::rename(source, destination)?;
            }
        }
        let current = self.directory.join("quotatide.jsonl");
        if current.exists() {
            fs::rename(current, self.directory.join("quotatide.1.jsonl"))?;
        }
        Ok(())
    }
}

pub fn initialize_safe_logging(app_data: &Path) -> std::io::Result<()> {
    let writer = SafeLogWriter::new(app_data)?;
    SAFE_LOGGER
        .set(writer)
        .map_err(|_| std::io::Error::other("safe logging is already initialized"))
}

pub fn safe_log(level: SafeLogLevel, component: &str, operation: &str, fields: SafeLogFields) {
    if let Some(writer) = SAFE_LOGGER.get() {
        writer.record(level, component, operation, fields);
    }
}

#[derive(Debug, Clone)]
pub struct DiagnosticPayload {
    pub manifest: Value,
    pub app: Value,
    pub safe_settings: Value,
    pub source_health: Value,
    pub current_epoch_observations: Value,
}

pub fn database_diagnostic(database_path: &Path) -> Value {
    let Ok(connection) = rusqlite::Connection::open_with_flags(
        database_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return serde_json::json!({
            "present": database_path.is_file(),
            "integrity": "unavailable",
            "schemaVersion": null,
            "migrationChecksums": [],
        });
    };
    let schema_version = connection
        .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
        .ok();
    let integrity = connection
        .query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0))
        .unwrap_or_else(|_| "unavailable".to_owned());
    let checksums = connection
        .prepare("SELECT version, checksum FROM schema_migrations ORDER BY version")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok(serde_json::json!({
                        "version": row.get::<_, i64>(0)?,
                        "checksum": row.get::<_, String>(1)?,
                    }))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap_or_default();
    serde_json::json!({
        "present": true,
        "integrity": integrity,
        "schemaVersion": schema_version,
        "migrationChecksums": checksums,
    })
}

pub fn export_diagnostic_zip(
    app_data: &Path,
    target: &Path,
    payload: &DiagnosticPayload,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temporary = tempfile::Builder::new()
        .prefix("quotatide-diagnostic-")
        .tempdir()?;
    secure_directory(temporary.path())?;

    write_safe_json(temporary.path(), "manifest.json", &payload.manifest)?;
    write_safe_json(temporary.path(), "app.json", &payload.app)?;
    write_safe_json(
        temporary.path(),
        "safe-settings.json",
        &payload.safe_settings,
    )?;
    write_safe_json(
        temporary.path(),
        "source-health.json",
        &payload.source_health,
    )?;
    write_safe_json(
        temporary.path(),
        "current-epoch-observations.json",
        &payload.current_epoch_observations,
    )?;
    copy_safe_logs(app_data, temporary.path())?;
    scan_export_tree(temporary.path(), &[])?;
    if let Err(error) = write_zip(temporary.path(), target) {
        let _ = fs::remove_file(target);
        return Err(error);
    }
    secure_file(target)?;
    Ok(())
}

pub fn write_clear_marker(app_data: &Path) -> std::io::Result<()> {
    let marker = app_data.join(CLEAR_MARKER);
    fs::write(&marker, b"v1\n")?;
    secure_file(&marker)
}

pub fn clear_requested(app_data: &Path) -> bool {
    app_data.join(CLEAR_MARKER).is_file()
}

pub fn clear_scoped_local_data(app_data: &Path) -> std::io::Result<()> {
    for name in ["state.sqlite3", "state.sqlite3-wal", "state.sqlite3-shm"] {
        remove_file_if_present(&app_data.join(name))?;
    }
    clear_matching_directory(&app_data.join("backups"), |name| {
        name.starts_with("state-v") && name.ends_with(".sqlite3")
    })?;
    clear_matching_directory(&app_data.join("recovery"), |name| {
        matches!(
            name,
            "state.sqlite3" | "state.sqlite3-wal" | "state.sqlite3-shm"
        )
    })?;
    clear_matching_directory(&app_data.join("logs"), is_log_file)?;
    clear_matching_directory(&app_data.join("export-tmp"), |name| {
        matches!(
            name,
            "manifest.json"
                | "app.json"
                | "safe-settings.json"
                | "source-health.json"
                | "current-epoch-observations.json"
        ) || is_log_file(name)
    })?;
    remove_file_if_present(&app_data.join(CLEAR_MARKER))
}

fn write_safe_json(directory: &Path, name: &str, value: &Value) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?;
    scan_bytes(&bytes, &[])?;
    let path = directory.join(name);
    fs::write(&path, bytes)?;
    secure_file(&path)
}

fn copy_safe_logs(app_data: &Path, temporary: &Path) -> std::io::Result<()> {
    let source = app_data.join("logs");
    let Ok(source_metadata) = fs::symlink_metadata(&source) else {
        return Ok(());
    };
    if source_metadata.file_type().is_symlink() || !source_metadata.is_dir() {
        return Ok(());
    }
    let destination = temporary.join("logs");
    fs::create_dir(&destination)?;
    secure_directory(&destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_file() || file_type.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !is_log_file(name) {
            continue;
        }
        let bytes = fs::read(entry.path())?;
        scan_bytes(&bytes, &[])?;
        let target = destination.join(name);
        fs::write(&target, bytes)?;
        secure_file(&target)?;
    }
    Ok(())
}

fn write_zip(source: &Path, target: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let file = File::create(target)?;
    let mut archive = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .unix_permissions(0o600);
    let mut files = Vec::new();
    collect_files(source, source, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    for (name, path) in files {
        archive.start_file(name, options)?;
        let mut source_file = File::open(path)?;
        std::io::copy(&mut source_file, &mut archive)?;
    }
    archive.finish()?;
    Ok(())
}

fn collect_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<(String, PathBuf)>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, files)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(std::io::Error::other)?
                .to_string_lossy()
                .replace('\\', "/");
            files.push((relative, path));
        }
    }
    Ok(())
}

fn scan_export_tree(directory: &Path, extra_forbidden: &[&str]) -> std::io::Result<()> {
    let mut files = Vec::new();
    collect_files(directory, directory, &mut files)?;
    for (_, file) in files {
        scan_bytes(&fs::read(file)?, extra_forbidden)?;
    }
    Ok(())
}

fn scan_bytes(bytes: &[u8], extra_forbidden: &[&str]) -> std::io::Result<()> {
    let lower = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    if FORBIDDEN_DIAGNOSTIC_TERMS
        .iter()
        .chain(extra_forbidden)
        .any(|term| lower.contains(&term.to_ascii_lowercase()))
    {
        Err(std::io::Error::other(
            "diagnostic payload contains a forbidden field",
        ))
    } else {
        Ok(())
    }
}

fn is_log_file(name: &str) -> bool {
    (name == "quotatide.jsonl" || name.starts_with("quotatide."))
        && Path::new(name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("jsonl"))
}

fn clear_matching_directory(
    directory: &Path,
    should_remove: impl Copy + Fn(&str) -> bool,
) -> std::io::Result<()> {
    let Ok(metadata) = fs::symlink_metadata(directory) else {
        return Ok(());
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Ok(());
    }
    clear_matching_tree(directory, should_remove)?;
    remove_directory_if_empty(directory)
}

fn clear_matching_tree(
    directory: &Path,
    should_remove: impl Copy + Fn(&str) -> bool,
) -> std::io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            clear_matching_tree(&path, should_remove)?;
            remove_directory_if_empty(&path)?;
        } else if file_type.is_file() && entry.file_name().to_str().is_some_and(should_remove) {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn remove_directory_if_empty(directory: &Path) -> std::io::Result<()> {
    if directory.is_dir() && fs::read_dir(directory)?.next().is_none() {
        fs::remove_dir(directory)?;
    }
    Ok(())
}

fn remove_file_if_present(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
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
            "privacy directory is not a regular directory",
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
            "privacy directory is not a regular directory",
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
            "privacy file is not a regular file",
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
            "privacy file is not a regular file",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_logs_rotate_with_a_five_mebibyte_hard_ceiling() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let writer = SafeLogWriter::new(directory.path()).expect("safe logger");
        let component = "x".repeat(700_000);
        for _ in 0..12 {
            writer.record(
                SafeLogLevel::Warning,
                &component,
                "bounded-write",
                SafeLogFields::default(),
            );
        }
        let logs = directory.path().join("logs");
        let files = fs::read_dir(logs)
            .expect("list logs")
            .map(|entry| entry.expect("log entry").path())
            .collect::<Vec<_>>();
        assert!(files.len() <= LOG_FILE_COUNT);
        let total = files
            .iter()
            .map(|path| path.metadata().expect("metadata").len())
            .sum::<u64>();
        assert!(total <= LOG_FILE_LIMIT * u64::try_from(LOG_FILE_COUNT).expect("count"));
        assert!(
            files
                .iter()
                .all(|path| path.metadata().expect("metadata").len() <= LOG_FILE_LIMIT)
        );
    }

    #[test]
    fn safe_logger_refuses_forbidden_canary_fields_before_writing() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let writer = SafeLogWriter::new(directory.path()).expect("safe logger");

        writer.record(
            SafeLogLevel::Error,
            "access_token=log-canary-secret",
            "request",
            SafeLogFields::default(),
        );

        assert!(!directory.path().join("logs/quotatide.jsonl").exists());
    }

    #[test]
    fn diagnostic_zip_contains_only_reserialized_allowlisted_files() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let target = directory.path().join("diagnostic.zip");
        let payload = DiagnosticPayload {
            manifest: serde_json::json!({"formatVersion": 1}),
            app: serde_json::json!({"version": "0.1.0", "osFamily": "macos"}),
            safe_settings: serde_json::json!({"configured": true, "smtpConfigured": true}),
            source_health: serde_json::json!({"codex": {"status": "fresh"}}),
            current_epoch_observations: serde_json::json!({"usedMicropoints": 42_000_000}),
        };
        export_diagnostic_zip(directory.path(), &target, &payload).expect("diagnostic export");

        let file = File::open(target).expect("open zip");
        let mut archive = zip::ZipArchive::new(file).expect("read zip");
        let mut names = (0..archive.len())
            .map(|index| archive.by_index(index).expect("entry").name().to_owned())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(
            names,
            [
                "app.json",
                "current-epoch-observations.json",
                "manifest.json",
                "safe-settings.json",
                "source-health.json",
            ]
        );
        assert!(archive.by_name("state.sqlite3").is_err());
    }

    #[test]
    fn forbidden_canary_stops_diagnostic_creation() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let payload = DiagnosticPayload {
            manifest: serde_json::json!({"formatVersion": 1}),
            app: serde_json::json!({"version": "0.1.0"}),
            safe_settings: serde_json::json!({"access_token": "canary-secret"}),
            source_health: serde_json::json!({}),
            current_epoch_observations: serde_json::json!({}),
        };
        let target = directory.path().join("diagnostic.zip");
        assert!(export_diagnostic_zip(directory.path(), &target, &payload).is_err());
        assert!(!target.exists());
    }

    #[test]
    fn scoped_clear_preserves_auth_and_unrecognized_files() {
        let directory = tempfile::tempdir().expect("temporary directory");
        fs::write(directory.path().join("state.sqlite3"), b"database").expect("database");
        fs::write(directory.path().join("auth.json"), b"auth-canary").expect("auth");
        let recovery = directory.path().join("recovery/old");
        fs::create_dir_all(&recovery).expect("recovery");
        fs::write(recovery.join("state.sqlite3"), b"database").expect("recovery database");
        fs::write(recovery.join("auth.json"), b"nested-auth-canary").expect("nested auth");
        let logs = directory.path().join("logs");
        fs::create_dir_all(&logs).expect("logs");
        fs::write(logs.join("quotatide.jsonl"), b"safe log").expect("safe log");

        clear_scoped_local_data(directory.path()).expect("scoped clear");

        assert!(!directory.path().join("state.sqlite3").exists());
        assert!(!logs.join("quotatide.jsonl").exists());
        assert_eq!(
            fs::read(directory.path().join("auth.json")).expect("root auth"),
            b"auth-canary"
        );
        assert_eq!(
            fs::read(recovery.join("auth.json")).expect("nested auth"),
            b"nested-auth-canary"
        );
    }
}
