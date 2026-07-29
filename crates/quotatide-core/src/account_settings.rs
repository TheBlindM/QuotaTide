use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio_rusqlite::{Connection, rusqlite};
use ts_rs::TS;
use uuid::Uuid;

const SCHEMA_VERSION: i64 = 1;
const SCHEMA_CHECKSUM: &str = "quotatide-settings-v1-account-path-stream";

/// A stable, secret-free account configuration projection for the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct PublicAccountSettings {
    pub settings_revision: u32,
    pub configured: bool,
    pub path_summary: Option<String>,
    pub account_label: Option<String>,
}

/// Stable storage failure category. Database details never cross the public boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsStoreError {
    Database,
}

impl std::fmt::Display for SettingsStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("account settings store unavailable")
    }
}

impl std::error::Error for SettingsStoreError {}

/// Versioned `SQLite` owner for non-secret current-account settings.
#[derive(Clone)]
pub struct AccountSettingsStore {
    connection: Connection,
}

impl AccountSettingsStore {
    /// Opens or creates the versioned settings database.
    ///
    /// # Errors
    ///
    /// Returns [`SettingsStoreError::Database`] when the database cannot be
    /// opened or initialized.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, SettingsStoreError> {
        let connection = Connection::open(path)
            .await
            .map_err(|_| SettingsStoreError::Database)?;
        let salt = new_salt();
        let app_instance_id = Uuid::new_v4().to_string();
        let now = unix_time_ms();
        connection
            .call(move |database| {
                let current_version: i64 =
                    database.pragma_query_value(None, "user_version", |row| row.get(0))?;
                if current_version > SCHEMA_VERSION {
                    return Err(rusqlite::Error::InvalidQuery);
                }
                database.execute_batch(
                    "PRAGMA foreign_keys = ON;
                     PRAGMA journal_mode = WAL;
                     PRAGMA synchronous = FULL;
                     PRAGMA busy_timeout = 5000;
                     PRAGMA trusted_schema = OFF;
                     CREATE TABLE IF NOT EXISTS schema_migrations (
                       version INTEGER PRIMARY KEY,
                       applied_at_ms INTEGER NOT NULL,
                       app_version TEXT NOT NULL,
                       checksum TEXT NOT NULL
                     );
                     CREATE TABLE IF NOT EXISTS app_meta (
                       singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
                       app_instance_id TEXT NOT NULL UNIQUE,
                       local_hash_salt BLOB NOT NULL CHECK (length(local_hash_salt) = 32),
                       settings_revision INTEGER NOT NULL CHECK (settings_revision >= 0),
                       created_at_ms INTEGER NOT NULL,
                       updated_at_ms INTEGER NOT NULL
                     );
                     CREATE TABLE IF NOT EXISTS account_streams (
                       id INTEGER PRIMARY KEY,
                       stream_key TEXT NOT NULL UNIQUE,
                       account_key BLOB NOT NULL UNIQUE,
                       first_seen_at_ms INTEGER NOT NULL,
                       last_seen_at_ms INTEGER NOT NULL
                     );
                     CREATE TABLE IF NOT EXISTS app_settings (
                       singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
                       auth_path TEXT,
                       configured_account_stream_id INTEGER REFERENCES account_streams(id),
                       active_account_stream_id INTEGER REFERENCES account_streams(id),
                       created_at_ms INTEGER NOT NULL,
                       updated_at_ms INTEGER NOT NULL
                     );",
                )?;
                database.execute(
                    "INSERT OR IGNORE INTO schema_migrations
                     (version, applied_at_ms, app_version, checksum)
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![
                        SCHEMA_VERSION,
                        now,
                        env!("CARGO_PKG_VERSION"),
                        SCHEMA_CHECKSUM
                    ],
                )?;
                database.execute(
                    "INSERT OR IGNORE INTO app_meta
                     (singleton_id, app_instance_id, local_hash_salt, settings_revision,
                      created_at_ms, updated_at_ms)
                     VALUES (1, ?1, ?2, 0, ?3, ?3)",
                    rusqlite::params![app_instance_id, salt.as_slice(), now],
                )?;
                database.execute(
                    "INSERT OR IGNORE INTO app_settings
                     (singleton_id, auth_path, configured_account_stream_id,
                      active_account_stream_id,
                      created_at_ms, updated_at_ms)
                     VALUES (1, NULL, NULL, NULL, ?1, ?1)",
                    [now],
                )?;
                database.pragma_update(None, "user_version", SCHEMA_VERSION)?;
                Ok::<_, rusqlite::Error>(())
            })
            .await
            .map_err(|_| SettingsStoreError::Database)?;

        Ok(Self { connection })
    }

    /// Atomically saves a validated canonical path and selects its isolated account stream.
    ///
    /// # Errors
    ///
    /// Returns [`SettingsStoreError::Database`] when the transaction cannot
    /// read or commit the new account configuration.
    pub async fn configure_account(
        &self,
        canonical_path: impl AsRef<str>,
        canonical_account_id: impl AsRef<str>,
    ) -> Result<PublicAccountSettings, SettingsStoreError> {
        let path = canonical_path.as_ref().to_owned();
        let account_id = canonical_account_id.as_ref().to_owned();
        self.connection
            .call(move |database| {
                let transaction = database.transaction()?;
                let (salt, revision): (Vec<u8>, i64) = transaction.query_row(
                    "SELECT local_hash_salt, settings_revision
                     FROM app_meta WHERE singleton_id = 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                let account_key = account_key(&salt, &account_id);
                let now = unix_time_ms();
                transaction.execute(
                    "INSERT INTO account_streams
                     (stream_key, account_key, first_seen_at_ms, last_seen_at_ms)
                     VALUES (?1, ?2, ?3, ?3)
                     ON CONFLICT(account_key) DO UPDATE SET last_seen_at_ms = excluded.last_seen_at_ms",
                    rusqlite::params![Uuid::new_v4().to_string(), account_key.as_slice(), now],
                )?;
                let stream_id: i64 = transaction.query_row(
                    "SELECT id FROM account_streams WHERE account_key = ?1",
                    [account_key.as_slice()],
                    |row| row.get(0),
                )?;
                transaction.execute(
                    "UPDATE app_settings
                     SET auth_path = ?1, configured_account_stream_id = ?2,
                         updated_at_ms = ?3
                     WHERE singleton_id = 1",
                    rusqlite::params![path, stream_id, now],
                )?;
                transaction.execute(
                    "UPDATE app_meta
                     SET settings_revision = settings_revision + 1, updated_at_ms = ?1
                     WHERE singleton_id = 1",
                    [now],
                )?;
                transaction.commit()?;
                Ok::<_, rusqlite::Error>(PublicAccountSettings {
                    settings_revision: u32::try_from(revision + 1).unwrap_or(u32::MAX),
                    configured: true,
                    path_summary: Some("…/auth.json".to_owned()),
                    account_label: Some(account_label(&account_key)),
                })
            })
            .await
            .map_err(|_| SettingsStoreError::Database)
    }

    /// Returns the current secret-free account settings.
    ///
    /// # Errors
    ///
    /// Returns [`SettingsStoreError::Database`] when the current projection
    /// cannot be read.
    pub async fn public_settings(&self) -> Result<PublicAccountSettings, SettingsStoreError> {
        self.connection
            .call(|database| {
                database.query_row(
                    "SELECT m.settings_revision, s.auth_path, a.account_key
                     FROM app_meta m
                     JOIN app_settings s ON s.singleton_id = m.singleton_id
                     LEFT JOIN account_streams a ON a.id = s.configured_account_stream_id
                     WHERE m.singleton_id = 1",
                    [],
                    |row| {
                        let revision: i64 = row.get(0)?;
                        let path: Option<String> = row.get(1)?;
                        let account_key: Option<Vec<u8>> = row.get(2)?;
                        Ok(PublicAccountSettings {
                            settings_revision: u32::try_from(revision).unwrap_or_default(),
                            configured: path.is_some() && account_key.is_some(),
                            path_summary: path.as_ref().map(|_| "…/auth.json".to_owned()),
                            account_label: account_key.as_deref().map(account_label),
                        })
                    },
                )
            })
            .await
            .map_err(|_| SettingsStoreError::Database)
    }

    /// Counts isolated streams for contract verification.
    ///
    /// # Errors
    ///
    /// Returns [`SettingsStoreError::Database`] when the stream table cannot
    /// be queried.
    pub async fn account_stream_count(&self) -> Result<u64, SettingsStoreError> {
        self.connection
            .call(|database| {
                database.query_row("SELECT COUNT(*) FROM account_streams", [], |row| {
                    let count: i64 = row.get(0)?;
                    Ok(u64::try_from(count).unwrap_or_default())
                })
            })
            .await
            .map_err(|_| SettingsStoreError::Database)
    }
}

fn account_key(salt: &[u8], account_id: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(salt);
    hasher.update(account_id.as_bytes());
    hasher.finalize().into()
}

fn account_label(key: &[u8]) -> String {
    key.iter()
        .take(2)
        .fold(String::from("账号 • "), |mut output, byte| {
            use std::fmt::Write as _;
            let _ = write!(output, "{byte:02X}");
            output
        })
}

fn new_salt() -> [u8; 32] {
    let mut salt = [0_u8; 32];
    salt[..16].copy_from_slice(Uuid::new_v4().as_bytes());
    salt[16..].copy_from_slice(Uuid::new_v4().as_bytes());
    salt
}

fn unix_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
        })
}
