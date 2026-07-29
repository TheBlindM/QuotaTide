use std::error::Error;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;
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

/// Stable error categories shared by the Rust application layer and IPC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum PublicErrorCode {
    InvalidPath,
    AuthNotFound,
    AuthPermissionDenied,
    AuthIo,
    AuthNotRegularFile,
    AuthTooLarge,
    AuthInvalidUtf8,
    AuthInvalidJson,
    AuthUnsupportedMode,
    AuthMissingAccessToken,
    AuthMissingAccountId,
    AuthInvalidAccountId,
    SettingsConflict,
    StorageUnavailable,
    NativeDialogUnavailable,
}

/// Deliberately narrow context whose fields are safe to serialize.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct SafeErrorContext {
    pub max_bytes: Option<u32>,
}

/// The only account-configuration error payload allowed across IPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct PublicError {
    pub code: PublicErrorCode,
    pub message_key: String,
    pub safe_context: SafeErrorContext,
}

impl PublicError {
    #[must_use]
    pub fn new(code: PublicErrorCode, message_key: impl Into<String>) -> Self {
        Self {
            code,
            message_key: message_key.into(),
            safe_context: SafeErrorContext::default(),
        }
    }

    #[must_use]
    pub fn with_max_bytes(mut self, max_bytes: u32) -> Self {
        self.safe_context.max_bytes = Some(max_bytes);
        self
    }
}

/// A validated candidate. It intentionally implements neither `Debug` nor serialization.
pub struct ValidatedAccountCandidate {
    canonical_path: String,
    canonical_account_id: String,
}

impl ValidatedAccountCandidate {
    #[must_use]
    pub fn new(canonical_path: String, canonical_account_id: String) -> Self {
        Self {
            canonical_path,
            canonical_account_id,
        }
    }
}

/// Read-only validation seam owned by the application layer.
pub trait AuthCandidateValidator: Clone + Send + Sync + 'static {
    type Error: Error + Send + Sync + 'static;

    /// Validates one user-selected path without modifying the source file.
    ///
    /// # Errors
    ///
    /// Returns an adapter error when the candidate cannot be read or validated.
    fn validate(&self, path: &Path) -> Result<ValidatedAccountCandidate, Self::Error>;
    fn public_error(error: &Self::Error) -> PublicError;
}

/// Internal application error that preserves its source while exposing a safe projection.
#[derive(Debug, Error)]
pub enum AccountConfigError<E: Error + Send + Sync + 'static> {
    #[error("authentication candidate validation failed")]
    Validation(#[source] E),
    #[error(transparent)]
    Storage(#[from] SettingsStoreError),
}

impl<E: Error + Send + Sync + 'static> AccountConfigError<E> {
    #[must_use]
    pub fn public<V>(&self) -> PublicError
    where
        V: AuthCandidateValidator<Error = E>,
    {
        match self {
            Self::Validation(error) => V::public_error(error),
            Self::Storage(SettingsStoreError::Conflict) => PublicError::new(
                PublicErrorCode::SettingsConflict,
                "settings.revision_conflict",
            ),
            Self::Storage(SettingsStoreError::Database(_)) => PublicError::new(
                PublicErrorCode::StorageUnavailable,
                "settings.storage_unavailable",
            ),
        }
    }
}

/// Core-owned settings use case. Tauri only supplies the picker path.
#[derive(Clone)]
pub struct SettingsManager<V> {
    store: AccountSettingsStore,
    validator: V,
}

impl<V: AuthCandidateValidator> SettingsManager<V> {
    #[must_use]
    pub const fn new(store: AccountSettingsStore, validator: V) -> Self {
        Self { store, validator }
    }

    /// Reads the current secret-free projection.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the versioned state cannot be read.
    pub async fn public_settings(
        &self,
    ) -> Result<PublicAccountSettings, AccountConfigError<V::Error>> {
        self.store.public_settings().await.map_err(Into::into)
    }

    /// Validates and atomically selects a candidate at the expected revision.
    ///
    /// # Errors
    ///
    /// Returns validation, storage, or optimistic-concurrency errors.
    pub async fn configure_account(
        &self,
        expected_revision: u32,
        path: &Path,
    ) -> Result<PublicAccountSettings, AccountConfigError<V::Error>> {
        let candidate = self
            .validator
            .validate(path)
            .map_err(AccountConfigError::Validation)?;
        self.store
            .configure_account(
                expected_revision,
                candidate.canonical_path,
                candidate.canonical_account_id,
            )
            .await
            .map_err(Into::into)
    }
}

/// Sole account-configuration facade exposed to the native shell.
#[derive(Clone)]
pub struct AccountApplication<V> {
    settings: SettingsManager<V>,
}

impl<V: AuthCandidateValidator> AccountApplication<V> {
    #[must_use]
    pub const fn new(settings: SettingsManager<V>) -> Self {
        Self { settings }
    }

    /// Reads account settings through the application facade.
    ///
    /// # Errors
    ///
    /// Returns a storage error if settings cannot be read.
    pub async fn account_settings(
        &self,
    ) -> Result<PublicAccountSettings, AccountConfigError<V::Error>> {
        self.settings.public_settings().await
    }

    /// Selects an account through the application facade.
    ///
    /// # Errors
    ///
    /// Returns validation, storage, or optimistic-concurrency errors.
    pub async fn select_account(
        &self,
        expected_revision: u32,
        path: &Path,
    ) -> Result<PublicAccountSettings, AccountConfigError<V::Error>> {
        self.settings
            .configure_account(expected_revision, path)
            .await
    }
}

/// Stable storage failure category. Database details never cross the public boundary.
#[derive(Debug, Error)]
pub enum SettingsStoreError {
    #[error("account settings changed while the picker was open")]
    Conflict,
    #[error("account settings store unavailable")]
    Database(#[source] Box<dyn Error + Send + Sync>),
}

impl SettingsStoreError {
    fn database(error: impl Error + Send + Sync + 'static) -> Self {
        Self::Database(Box::new(error))
    }
}

#[derive(Debug, Error)]
enum StoreCallError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("settings revision conflict")]
    Conflict,
}

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
    /// Returns a database error if initialization or migration fails.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, SettingsStoreError> {
        let connection = Connection::open(path)
            .await
            .map_err(SettingsStoreError::database)?;
        let salt = new_salt().map_err(SettingsStoreError::database)?;
        let app_instance_id = Uuid::now_v7().to_string();
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
                     PRAGMA trusted_schema = OFF;",
                )?;
                if current_version == 0 {
                    let transaction = database
                        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                    transaction.execute_batch(
                        "CREATE TABLE schema_migrations (
                           version INTEGER PRIMARY KEY,
                           applied_at_ms INTEGER NOT NULL,
                           app_version TEXT NOT NULL,
                           checksum TEXT NOT NULL
                         );
                         CREATE TABLE app_meta (
                           singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
                           app_instance_id TEXT NOT NULL UNIQUE,
                           local_hash_salt BLOB NOT NULL CHECK (length(local_hash_salt) = 32),
                           settings_revision INTEGER NOT NULL CHECK (settings_revision >= 0),
                           created_at_ms INTEGER NOT NULL,
                           updated_at_ms INTEGER NOT NULL
                         );
                         CREATE TABLE account_streams (
                           id INTEGER PRIMARY KEY,
                           stream_key TEXT NOT NULL UNIQUE,
                           account_key BLOB NOT NULL UNIQUE,
                           first_seen_at_ms INTEGER NOT NULL,
                           last_seen_at_ms INTEGER NOT NULL
                         );
                         CREATE TABLE app_settings (
                           singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
                           auth_path TEXT,
                           configured_account_stream_id INTEGER REFERENCES account_streams(id),
                           active_account_stream_id INTEGER REFERENCES account_streams(id),
                           created_at_ms INTEGER NOT NULL,
                           updated_at_ms INTEGER NOT NULL
                         );",
                    )?;
                    transaction.execute(
                        "INSERT INTO schema_migrations
                         (version, applied_at_ms, app_version, checksum)
                         VALUES (?1, ?2, ?3, ?4)",
                        rusqlite::params![
                            SCHEMA_VERSION,
                            now,
                            env!("CARGO_PKG_VERSION"),
                            SCHEMA_CHECKSUM
                        ],
                    )?;
                    transaction.execute(
                        "INSERT INTO app_meta
                         (singleton_id, app_instance_id, local_hash_salt, settings_revision,
                          created_at_ms, updated_at_ms)
                         VALUES (1, ?1, ?2, 0, ?3, ?3)",
                        rusqlite::params![app_instance_id, salt.as_slice(), now],
                    )?;
                    transaction.execute(
                        "INSERT INTO app_settings
                         (singleton_id, auth_path, configured_account_stream_id,
                          active_account_stream_id, created_at_ms, updated_at_ms)
                         VALUES (1, NULL, NULL, NULL, ?1, ?1)",
                        [now],
                    )?;
                    transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
                    transaction.commit()?;
                } else {
                    let checksum: String = database.query_row(
                        "SELECT checksum FROM schema_migrations WHERE version = ?1",
                        [SCHEMA_VERSION],
                        |row| row.get(0),
                    )?;
                    if checksum != SCHEMA_CHECKSUM {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                }
                Ok::<_, rusqlite::Error>(())
            })
            .await
            .map_err(SettingsStoreError::database)?;

        Ok(Self { connection })
    }

    /// Atomically commits a validated account if the revision still matches.
    ///
    /// # Errors
    ///
    /// Returns [`SettingsStoreError::Conflict`] for stale revisions or a
    /// database error when the transaction cannot be committed.
    pub async fn configure_account(
        &self,
        expected_revision: u32,
        canonical_path: impl AsRef<str>,
        canonical_account_id: impl AsRef<str>,
    ) -> Result<PublicAccountSettings, SettingsStoreError> {
        let path = canonical_path.as_ref().to_owned();
        let account_id = canonical_account_id.as_ref().to_owned();
        self.connection
            .call(move |database| {
                let transaction = database
                    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let (salt, revision): (Vec<u8>, i64) = transaction.query_row(
                    "SELECT local_hash_salt, settings_revision
                     FROM app_meta WHERE singleton_id = 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                if revision != i64::from(expected_revision) {
                    return Err(StoreCallError::Conflict);
                }
                let account_key = account_key(&salt, &account_id);
                let now = unix_time_ms();
                transaction.execute(
                    "INSERT INTO account_streams
                     (stream_key, account_key, first_seen_at_ms, last_seen_at_ms)
                     VALUES (?1, ?2, ?3, ?3)
                     ON CONFLICT(account_key) DO UPDATE SET last_seen_at_ms = excluded.last_seen_at_ms",
                    rusqlite::params![Uuid::now_v7().to_string(), account_key.as_slice(), now],
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
                Ok(PublicAccountSettings {
                    settings_revision: expected_revision.saturating_add(1),
                    configured: true,
                    path_summary: Some("…/auth.json".to_owned()),
                    account_label: Some(account_label(&account_key)),
                })
            })
            .await
            .map_err(|error| match error {
                tokio_rusqlite::Error::Error(StoreCallError::Conflict) => {
                    SettingsStoreError::Conflict
                }
                other => SettingsStoreError::database(other),
            })
    }

    /// Reads the current secret-free projection.
    ///
    /// # Errors
    ///
    /// Returns a database error when the projection cannot be read.
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
            .map_err(SettingsStoreError::database)
    }

    /// Counts isolated streams for contract tests.
    ///
    /// # Errors
    ///
    /// Returns a database error when the stream table cannot be queried.
    pub async fn account_stream_count(&self) -> Result<u64, SettingsStoreError> {
        self.connection
            .call(|database| {
                database.query_row("SELECT COUNT(*) FROM account_streams", [], |row| {
                    let count: i64 = row.get(0)?;
                    Ok(u64::try_from(count).unwrap_or_default())
                })
            })
            .await
            .map_err(SettingsStoreError::database)
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

fn new_salt() -> Result<[u8; 32], getrandom::Error> {
    let mut salt = [0_u8; 32];
    getrandom::fill(&mut salt)?;
    Ok(salt)
}

fn unix_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
        })
}
