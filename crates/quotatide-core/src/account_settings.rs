use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::future::Future;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio_rusqlite::{Connection, rusqlite};
use ts_rs::TS;
use uuid::Uuid;

use crate::alerts::{
    AlertTarget, ClaimedSystemDelivery, NotificationPermissionStatus, PublicAlertEvent,
    PublicAlertInbox, PublicDeliveryState,
};
use crate::live_quota::CombinedRefreshDisposition;
use crate::quota_ledger::{
    DailyLimitSnapshot, DailyPolicyStatus, PersistedLedgerEpoch, PolicyDayProjection, PolicyError,
    PolicyWindowFacts, QuotaPolicy, ThresholdTransition,
};
use crate::{
    LedgerApplyKind, LedgerDayStatus, PublicLedgerDay, PublicLiveQuota, PublicRadarAnnouncement,
    PublicRadarPrediction, PublicResetRadar, QuotaLedger, RadarChance, RadarCommitDisposition,
    RadarSnapshot, RadarSourceError, RadarSourceErrorCode, RefreshAccountBinding, RefreshOutcome,
    SourceStatus, UsageRefreshAttempt, UsageSourceErrorCode, WeeklyUsageObservation,
    radar_bucket_label,
};

const SCHEMA_VERSION: i64 = 9;
const SETTINGS_SCHEMA_CHECKSUM: &str = "quotatide-settings-v1-account-path-stream";
const LIVE_QUOTA_SCHEMA_CHECKSUM: &str = "quotatide-v2-live-quota-health";
const QUOTA_LEDGER_SCHEMA_CHECKSUM: &str = "quotatide-v3-current-seven-day-ledger";
const IMMUTABLE_IANA_SCHEMA_CHECKSUM: &str = "quotatide-v4-immutable-observations-iana-policy";
const DAILY_POLICY_SCHEMA_CHECKSUM: &str = "quotatide-v5-versioned-daily-policy";
const RESET_RADAR_SCHEMA_CHECKSUM: &str = "quotatide-v6-independent-reset-radar";
const ATOMIC_RADAR_SCHEMA_CHECKSUM: &str = "quotatide-v7-atomic-radar-refresh";
const ATOMIC_SETTINGS_SCHEMA_CHECKSUM: &str = "quotatide-v8-atomic-settings-journal";
const DURABLE_ALERTS_SCHEMA_CHECKSUM: &str = "quotatide-v9-durable-alert-outbox";
const FRESH_FOR_MS: i64 = 90 * 60 * 1000;

/// A stable, secret-free account configuration projection for the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct PublicAccountSettings {
    pub settings_revision: u32,
    pub configured: bool,
    pub path_summary: Option<String>,
    pub account_label: Option<String>,
    pub quota_policy: PublicQuotaPolicy,
}

/// Current immutable daily-policy revision exposed without account secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct PublicQuotaPolicy {
    #[ts(type = "number")]
    pub policy_revision: u64,
    pub policy_timezone: String,
    pub carry_workdays_enabled: bool,
    pub base_micropoints: Vec<u32>,
}

/// Complete replacement draft submitted by the settings UI.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct QuotaPolicyDraft {
    pub policy_timezone: String,
    pub carry_workdays_enabled: bool,
    #[ts(type = "number[]")]
    pub base_micropoints: Vec<i64>,
}

/// One stable alert event configurable by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum AlertEventKind {
    #[serde(rename = "daily_80")]
    Daily80,
    #[serde(rename = "daily_100")]
    Daily100,
    #[serde(rename = "weekly_remaining_20")]
    WeeklyRemaining20,
    #[serde(rename = "weekly_remaining_10")]
    WeeklyRemaining10,
    #[serde(rename = "radar_chance_70")]
    RadarChance70,
    QuotaResetConfirmed,
    #[serde(rename = "source_failures_3")]
    SourceFailures3,
}

impl AlertEventKind {
    pub const ALL: [Self; 7] = [
        Self::Daily80,
        Self::Daily100,
        Self::WeeklyRemaining20,
        Self::WeeklyRemaining10,
        Self::RadarChance70,
        Self::QuotaResetConfirmed,
        Self::SourceFailures3,
    ];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Daily80 => "daily_80",
            Self::Daily100 => "daily_100",
            Self::WeeklyRemaining20 => "weekly_remaining_20",
            Self::WeeklyRemaining10 => "weekly_remaining_10",
            Self::RadarChance70 => "radar_chance_70",
            Self::QuotaResetConfirmed => "quota_reset_confirmed",
            Self::SourceFailures3 => "source_failures_3",
        }
    }

    pub(crate) fn parse(value: &str) -> rusqlite::Result<Self> {
        match value {
            "daily_80" => Ok(Self::Daily80),
            "daily_100" => Ok(Self::Daily100),
            "weekly_remaining_20" => Ok(Self::WeeklyRemaining20),
            "weekly_remaining_10" => Ok(Self::WeeklyRemaining10),
            "radar_chance_70" => Ok(Self::RadarChance70),
            "quota_reset_confirmed" => Ok(Self::QuotaResetConfirmed),
            "source_failures_3" => Ok(Self::SourceFailures3),
            _ => Err(rusqlite::Error::InvalidQuery),
        }
    }
}

/// Delivery channel for one alert event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum AlertChannel {
    System,
    Email,
}

impl AlertChannel {
    pub const ALL: [Self; 2] = [Self::System, Self::Email];

    const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Email => "email",
        }
    }

    fn parse(value: &str) -> rusqlite::Result<Self> {
        match value {
            "system" => Ok(Self::System),
            "email" => Ok(Self::Email),
            _ => Err(rusqlite::Error::InvalidQuery),
        }
    }
}

/// Secret-free persisted alert preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct AlertPreference {
    pub event_kind: AlertEventKind,
    pub channel: AlertChannel,
    pub enabled: bool,
}

/// Complete replacement value for one alert preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct AlertPreferenceDraft {
    pub event_kind: AlertEventKind,
    pub channel: AlertChannel,
    pub enabled: bool,
}

/// Every non-secret setting returned as one revisioned projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct PublicSettings {
    pub settings_revision: u32,
    pub configured: bool,
    pub path_summary: Option<String>,
    pub account_label: Option<String>,
    pub notification_permission_status: NotificationPermissionStatus,
    pub quota_policy: PublicQuotaPolicy,
    pub alert_preferences: Vec<AlertPreference>,
    pub autostart_enabled: bool,
}

/// Complete settings replacement submitted against one optimistic revision.
#[derive(Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct SettingsDraft {
    pub expected_settings_revision: u32,
    pub auth_path: Option<String>,
    pub quota_policy: QuotaPolicyDraft,
    pub alert_preferences: Vec<AlertPreferenceDraft>,
    pub autostart_enabled: bool,
}

/// Narrow operating-system boundary for current-user login startup.
pub trait AutostartControl: Clone + Send + Sync + 'static {
    type Error: Error + Send + Sync + 'static;

    fn is_enabled(&self) -> impl Future<Output = Result<bool, Self::Error>> + Send;
    fn set_enabled(&self, enabled: bool) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/// Core-owned settings use case coordinating `SQLite` and external state.
#[derive(Clone)]
pub struct AtomicSettingsManager<V, A> {
    store: AccountSettingsStore,
    validator: V,
    autostart: A,
}

impl<V: AuthCandidateValidator, A: AutostartControl> AtomicSettingsManager<V, A> {
    #[must_use]
    pub const fn new(store: AccountSettingsStore, validator: V, autostart: A) -> Self {
        Self {
            store,
            validator,
            autostart,
        }
    }

    /// Reads all current non-secret settings.
    ///
    /// # Errors
    ///
    /// Returns a stable storage error when the projection is unavailable.
    pub async fn public_settings(
        &self,
    ) -> Result<PublicSettings, AtomicSettingsError<V::Error, A::Error>> {
        self.store
            .public_atomic_settings()
            .await
            .map_err(AtomicSettingsError::Storage)
    }

    /// Replaces all settings in one revisioned operation.
    ///
    /// # Errors
    ///
    /// Returns validation, external-state, conflict, or storage failures.
    pub async fn save_settings(
        &self,
        draft: SettingsDraft,
    ) -> Result<PublicSettings, AtomicSettingsError<V::Error, A::Error>> {
        let SettingsDraft {
            expected_settings_revision,
            auth_path,
            quota_policy,
            alert_preferences,
            autostart_enabled,
        } = draft;
        let account_candidate = auth_path
            .as_deref()
            .map(Path::new)
            .map(|path| {
                self.validator
                    .validate(path)
                    .map_err(AtomicSettingsError::Validation)
            })
            .transpose()?;
        let policy = validate_quota_policy_draft(quota_policy)
            .map_err(|error| AtomicSettingsError::Storage(error.into()))?;
        validate_alert_preferences(&alert_preferences).map_err(AtomicSettingsError::Storage)?;

        let old_autostart_enabled = self
            .autostart
            .is_enabled()
            .await
            .map_err(AtomicSettingsError::Autostart)?;
        let operation_key = self
            .store
            .prepare_external_settings_change(
                expected_settings_revision,
                old_autostart_enabled,
                autostart_enabled,
            )
            .await
            .map_err(AtomicSettingsError::Storage)?;

        if old_autostart_enabled != autostart_enabled {
            if let Err(error) = self.autostart.set_enabled(autostart_enabled).await {
                self.restore_prepared_autostart(&operation_key, old_autostart_enabled)
                    .await;
                return Err(AtomicSettingsError::Autostart(error));
            }
        }
        let confirmed_autostart = match self.autostart.is_enabled().await {
            Ok(enabled) => enabled,
            Err(error) => {
                self.restore_prepared_autostart(&operation_key, old_autostart_enabled)
                    .await;
                return Err(AtomicSettingsError::Autostart(error));
            }
        };
        if confirmed_autostart != autostart_enabled {
            self.restore_prepared_autostart(&operation_key, old_autostart_enabled)
                .await;
            return Err(AtomicSettingsError::AutostartReadback);
        }

        let settings = self
            .store
            .commit_atomic_settings(
                &operation_key,
                expected_settings_revision,
                account_candidate,
                policy,
                alert_preferences,
                confirmed_autostart,
            )
            .await;
        match settings {
            Ok(settings) => {
                // A committed journal is intentionally recoverable. Cleanup is
                // best effort because reporting failure here would invite a
                // retry after the settings transaction already succeeded.
                let _ = self.store.clear_external_change(&operation_key).await;
                Ok(settings)
            }
            Err(error) => {
                self.restore_prepared_autostart(&operation_key, old_autostart_enabled)
                    .await;
                Err(AtomicSettingsError::Storage(error))
            }
        }
    }

    /// Reconciles an interrupted external change before workers start.
    ///
    /// A prepared operation restores the last confirmed external state. A
    /// committed operation reapplies the newly committed state before its
    /// durable journal entry is cleaned up.
    ///
    /// # Errors
    ///
    /// Returns an external-state or storage error and keeps the journal for a
    /// later retry when convergence cannot be confirmed.
    pub async fn recover_external_changes(
        &self,
    ) -> Result<(), AtomicSettingsError<V::Error, A::Error>> {
        let changes = self
            .store
            .external_changes()
            .await
            .map_err(AtomicSettingsError::Storage)?;
        for change in changes {
            let desired = match change.phase {
                ExternalChangePhase::Prepared => change.old_autostart_enabled,
                ExternalChangePhase::Committed => change.new_autostart_enabled,
            };
            let current = self
                .autostart
                .is_enabled()
                .await
                .map_err(AtomicSettingsError::Autostart)?;
            if current != desired {
                self.autostart
                    .set_enabled(desired)
                    .await
                    .map_err(AtomicSettingsError::Autostart)?;
            }
            let confirmed = self
                .autostart
                .is_enabled()
                .await
                .map_err(AtomicSettingsError::Autostart)?;
            if confirmed != desired {
                return Err(AtomicSettingsError::AutostartReadback);
            }
            self.store
                .clear_external_change(&change.operation_key)
                .await
                .map_err(AtomicSettingsError::Storage)?;
        }
        Ok(())
    }

    async fn restore_prepared_autostart(&self, operation_key: &str, old_enabled: bool) {
        if self.autostart.set_enabled(old_enabled).await.is_ok()
            && self
                .autostart
                .is_enabled()
                .await
                .is_ok_and(|enabled| enabled == old_enabled)
        {
            let _ = self.store.clear_external_change(operation_key).await;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExternalChangePhase {
    Prepared,
    Committed,
}

struct ExternalSettingsChange {
    operation_key: String,
    phase: ExternalChangePhase,
    old_autostart_enabled: bool,
    new_autostart_enabled: bool,
}

/// Atomic settings failure preserving source errors behind a safe boundary.
#[derive(Debug, Error)]
pub enum AtomicSettingsError<VE: Error + Send + Sync + 'static, AE: Error + Send + Sync + 'static> {
    #[error("authentication candidate validation failed")]
    Validation(#[source] VE),
    #[error("autostart state unavailable")]
    Autostart(#[source] AE),
    #[error("autostart state did not match the requested value after mutation")]
    AutostartReadback,
    #[error(transparent)]
    Storage(#[from] SettingsStoreError),
}

impl<VE, AE> AtomicSettingsError<VE, AE>
where
    VE: Error + Send + Sync + 'static,
    AE: Error + Send + Sync + 'static,
{
    #[must_use]
    pub fn public<V>(&self) -> PublicError
    where
        V: AuthCandidateValidator<Error = VE>,
    {
        match self {
            Self::Validation(error) => V::public_error(error),
            Self::Autostart(_) | Self::AutostartReadback => PublicError::new(
                PublicErrorCode::AutostartUnavailable,
                "settings.autostart_unavailable",
            ),
            Self::Storage(SettingsStoreError::Conflict) => PublicError::new(
                PublicErrorCode::SettingsConflict,
                "settings.revision_conflict",
            ),
            Self::Storage(SettingsStoreError::InvalidPolicy(_)) => PublicError::new(
                PublicErrorCode::InvalidQuotaPolicy,
                "settings.invalid_quota_policy",
            ),
            Self::Storage(SettingsStoreError::InvalidAlertPreferences) => PublicError::new(
                PublicErrorCode::InvalidAlertPreferences,
                "settings.invalid_alert_preferences",
            ),
            Self::Storage(
                SettingsStoreError::Database(_) | SettingsStoreError::InvalidNotificationState,
            ) => PublicError::new(
                PublicErrorCode::StorageUnavailable,
                "settings.storage_unavailable",
            ),
        }
    }
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
    InvalidQuotaPolicy,
    StorageUnavailable,
    NativeDialogUnavailable,
    InvalidAlertPreferences,
    AutostartUnavailable,
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
            Self::Storage(SettingsStoreError::InvalidPolicy(_)) => PublicError::new(
                PublicErrorCode::InvalidQuotaPolicy,
                "settings.invalid_quota_policy",
            ),
            Self::Storage(SettingsStoreError::InvalidAlertPreferences) => PublicError::new(
                PublicErrorCode::InvalidAlertPreferences,
                "settings.invalid_alert_preferences",
            ),
            Self::Storage(
                SettingsStoreError::Database(_) | SettingsStoreError::InvalidNotificationState,
            ) => PublicError::new(
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

    /// Validates and atomically activates a complete daily-policy draft.
    ///
    /// # Errors
    ///
    /// Returns validation, storage, or optimistic-concurrency errors.
    pub async fn update_quota_policy(
        &self,
        expected_revision: u32,
        draft: QuotaPolicyDraft,
    ) -> Result<PublicAccountSettings, AccountConfigError<V::Error>> {
        self.store
            .update_quota_policy(expected_revision, draft)
            .await
            .map_err(Into::into)
    }

    /// Returns the current live quota projection.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the projection cannot be read.
    pub async fn live_quota(
        &self,
        now_unix_ms: i64,
    ) -> Result<Option<PublicLiveQuota>, AccountConfigError<V::Error>> {
        self.store
            .public_live_quota(now_unix_ms)
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

    /// Replaces the active daily quota policy through the application facade.
    ///
    /// # Errors
    ///
    /// Returns validation, storage, or optimistic-concurrency errors.
    pub async fn update_quota_policy(
        &self,
        expected_revision: u32,
        draft: QuotaPolicyDraft,
    ) -> Result<PublicAccountSettings, AccountConfigError<V::Error>> {
        self.settings
            .update_quota_policy(expected_revision, draft)
            .await
    }

    /// Returns the secret-free live quota projection.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the projection cannot be read.
    pub async fn live_quota(
        &self,
        now_unix_ms: i64,
    ) -> Result<Option<PublicLiveQuota>, AccountConfigError<V::Error>> {
        self.settings.live_quota(now_unix_ms).await
    }
}

/// Stable storage failure category. Database details never cross the public boundary.
#[derive(Debug, Error)]
pub enum SettingsStoreError {
    #[error("account settings changed while the picker was open")]
    Conflict,
    #[error(transparent)]
    InvalidPolicy(#[from] PolicyError),
    #[error("alert preferences must replace every supported event and channel exactly once")]
    InvalidAlertPreferences,
    #[error("persisted notification state is invalid")]
    InvalidNotificationState,
    #[error("account settings store unavailable")]
    Database(#[source] Box<dyn Error + Send + Sync>),
}

/// Whether a refresh result still belongs to the selected account revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageCommitDisposition {
    Committed,
    Superseded,
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

fn initialize_database(
    database: &mut rusqlite::Connection,
    salt: &[u8; 32],
    app_instance_id: &str,
    now: i64,
    policy_timezone: &str,
) -> rusqlite::Result<()> {
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
        migrate_settings_v1(database, salt, app_instance_id, now)?;
    } else {
        validate_migration(database, 1, SETTINGS_SCHEMA_CHECKSUM)?;
    }
    if current_version <= 1 {
        migrate_live_quota_v2(database, now)?;
    } else {
        validate_migration(database, 2, LIVE_QUOTA_SCHEMA_CHECKSUM)?;
    }
    if current_version <= 2 {
        migrate_quota_ledger_v3(database, now)?;
    } else {
        validate_migration(database, 3, QUOTA_LEDGER_SCHEMA_CHECKSUM)?;
    }
    if current_version <= 3 {
        migrate_immutable_iana_v4(database, now, policy_timezone)?;
    } else {
        validate_migration(database, 4, IMMUTABLE_IANA_SCHEMA_CHECKSUM)?;
    }
    if current_version <= 4 {
        migrate_daily_policy_v5(database, now, policy_timezone)?;
    } else {
        validate_migration(database, 5, DAILY_POLICY_SCHEMA_CHECKSUM)?;
    }
    if current_version <= 5 {
        migrate_reset_radar_v6(database, now)?;
    } else {
        validate_migration(database, 6, RESET_RADAR_SCHEMA_CHECKSUM)?;
    }
    if current_version <= 6 {
        migrate_atomic_radar_v7(database, now)?;
    } else {
        validate_migration(database, 7, ATOMIC_RADAR_SCHEMA_CHECKSUM)?;
    }
    if current_version <= 7 {
        migrate_atomic_settings_v8(database, now)?;
    } else {
        validate_migration(database, 8, ATOMIC_SETTINGS_SCHEMA_CHECKSUM)?;
    }
    if current_version <= 8 {
        migrate_durable_alerts_v9(database, now)?;
    } else {
        validate_migration(database, 9, DURABLE_ALERTS_SCHEMA_CHECKSUM)?;
    }
    Ok(())
}

fn migrate_settings_v1(
    database: &mut rusqlite::Connection,
    salt: &[u8; 32],
    app_instance_id: &str,
    now: i64,
) -> rusqlite::Result<()> {
    let transaction =
        database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
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
         VALUES (1, ?1, ?2, ?3)",
        rusqlite::params![now, env!("CARGO_PKG_VERSION"), SETTINGS_SCHEMA_CHECKSUM],
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
    transaction.pragma_update(None, "user_version", 1)?;
    transaction.commit()
}

fn migrate_live_quota_v2(database: &mut rusqlite::Connection, now: i64) -> rusqlite::Result<()> {
    let transaction =
        database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        "CREATE TABLE usage_observations (
           id INTEGER PRIMARY KEY,
           account_stream_id INTEGER NOT NULL REFERENCES account_streams(id),
           captured_at_ms INTEGER NOT NULL,
           used_micropoints INTEGER NOT NULL
             CHECK (used_micropoints BETWEEN 0 AND 100000000),
           window_seconds INTEGER NOT NULL CHECK (window_seconds = 604800),
           resets_at_s INTEGER NOT NULL,
           plan_type TEXT,
           allowed INTEGER,
           UNIQUE(account_stream_id, captured_at_ms)
         );
         CREATE TABLE usage_source_health (
           account_stream_id INTEGER PRIMARY KEY REFERENCES account_streams(id),
           last_attempt_at_ms INTEGER NOT NULL,
           last_success_at_ms INTEGER,
           consecutive_failures INTEGER NOT NULL CHECK (consecutive_failures >= 0),
           public_error TEXT
         );
         CREATE INDEX usage_observations_stream_capture
           ON usage_observations(account_stream_id, captured_at_ms DESC);",
    )?;
    transaction.execute(
        "INSERT INTO schema_migrations
         (version, applied_at_ms, app_version, checksum)
         VALUES (2, ?1, ?2, ?3)",
        rusqlite::params![now, env!("CARGO_PKG_VERSION"), LIVE_QUOTA_SCHEMA_CHECKSUM],
    )?;
    transaction.pragma_update(None, "user_version", 2)?;
    transaction.commit()
}

fn migrate_quota_ledger_v3(database: &mut rusqlite::Connection, now: i64) -> rusqlite::Result<()> {
    let transaction =
        database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        "CREATE TABLE quota_epochs (
           id INTEGER PRIMARY KEY,
           account_stream_id INTEGER NOT NULL REFERENCES account_streams(id),
           sequence INTEGER NOT NULL CHECK (sequence > 0),
           baseline_micropoints INTEGER NOT NULL
             CHECK (baseline_micropoints BETWEEN 0 AND 100000000),
           high_water_micropoints INTEGER NOT NULL
             CHECK (high_water_micropoints BETWEEN 0 AND 100000000),
           first_observed_at_ms INTEGER NOT NULL,
           latest_observed_at_ms INTEGER NOT NULL,
           scheduled_reset_at_s INTEGER NOT NULL,
           closed_at_ms INTEGER,
           UNIQUE(account_stream_id, sequence)
         );
         CREATE UNIQUE INDEX one_active_quota_epoch_per_stream
           ON quota_epochs(account_stream_id) WHERE closed_at_ms IS NULL;
         CREATE TABLE daily_ledgers (
           id INTEGER PRIMARY KEY,
           account_stream_id INTEGER NOT NULL REFERENCES account_streams(id),
           local_date TEXT NOT NULL,
           policy_timezone TEXT NOT NULL,
           used_micropoints INTEGER NOT NULL CHECK (used_micropoints >= 0),
           updated_at_ms INTEGER NOT NULL,
           UNIQUE(account_stream_id, local_date, policy_timezone)
         );
         ALTER TABLE usage_observations
           ADD COLUMN quota_epoch_id INTEGER REFERENCES quota_epochs(id);
         ALTER TABLE app_meta
           ADD COLUMN dashboard_revision INTEGER NOT NULL DEFAULT 0;",
    )?;
    transaction.execute(
        "INSERT INTO schema_migrations
         (version, applied_at_ms, app_version, checksum)
         VALUES (3, ?1, ?2, ?3)",
        rusqlite::params![now, env!("CARGO_PKG_VERSION"), QUOTA_LEDGER_SCHEMA_CHECKSUM],
    )?;
    transaction.pragma_update(None, "user_version", 3)?;
    transaction.commit()
}

fn migrate_immutable_iana_v4(
    database: &mut rusqlite::Connection,
    now: i64,
    policy_timezone: &str,
) -> rusqlite::Result<()> {
    let transaction =
        database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        "ALTER TABLE app_settings
           ADD COLUMN policy_timezone TEXT NOT NULL DEFAULT 'Asia/Shanghai';
         ALTER TABLE usage_observations
           ADD COLUMN ledger_eligible INTEGER NOT NULL DEFAULT 1
             CHECK (ledger_eligible IN (0, 1));
         UPDATE usage_observations
           SET ledger_eligible = CASE
             WHEN captured_at_ms >= (resets_at_s - 604800) * 1000
              AND captured_at_ms < resets_at_s * 1000
             THEN 1 ELSE 0 END;",
    )?;
    reconcile_legacy_source_health(&transaction)?;
    transaction.execute(
        "UPDATE app_settings SET policy_timezone = ?1 WHERE singleton_id = 1",
        [policy_timezone],
    )?;
    // v3 epoch and daily rows are projections. Clear them before replaying
    // their immutable observations so an already-used v3 database upgrades
    // without uniqueness conflicts or stale policy-timezone facts.
    transaction.execute_batch(
        "UPDATE usage_observations SET quota_epoch_id = NULL;
         DELETE FROM daily_ledgers;
         DELETE FROM quota_epochs;",
    )?;
    backfill_usage_observation_epochs(&transaction, policy_timezone)?;
    transaction.execute_batch(
        "INSERT INTO quota_epochs
           (account_stream_id, sequence, baseline_micropoints,
            high_water_micropoints, first_observed_at_ms,
            latest_observed_at_ms, scheduled_reset_at_s, closed_at_ms)
         SELECT account_stream_id, 9223372036854775807,
                MIN(used_micropoints), MAX(used_micropoints),
                MIN(captured_at_ms), MAX(captured_at_ms),
                MAX(resets_at_s), MAX(captured_at_ms)
         FROM usage_observations
         WHERE ledger_eligible = 0
         GROUP BY account_stream_id;
         UPDATE usage_observations
           SET quota_epoch_id = (
             SELECT epoch.id FROM quota_epochs epoch
             WHERE epoch.account_stream_id = usage_observations.account_stream_id
               AND epoch.sequence = 9223372036854775807
           )
         WHERE ledger_eligible = 0;",
    )?;
    transaction.execute_batch(
        "CREATE TABLE usage_observations_v4 (
           id INTEGER PRIMARY KEY,
           account_stream_id INTEGER NOT NULL REFERENCES account_streams(id),
           quota_epoch_id INTEGER NOT NULL REFERENCES quota_epochs(id),
           ledger_eligible INTEGER NOT NULL CHECK (ledger_eligible IN (0, 1)),
           captured_at_ms INTEGER NOT NULL,
           used_micropoints INTEGER NOT NULL
             CHECK (used_micropoints BETWEEN 0 AND 100000000),
           window_seconds INTEGER NOT NULL CHECK (window_seconds = 604800),
           resets_at_s INTEGER NOT NULL,
           plan_type TEXT,
           allowed INTEGER,
           UNIQUE(account_stream_id, captured_at_ms)
         );
         INSERT INTO usage_observations_v4
           (id, account_stream_id, quota_epoch_id, ledger_eligible, captured_at_ms,
            used_micropoints, window_seconds, resets_at_s, plan_type, allowed)
           SELECT id, account_stream_id, quota_epoch_id, ledger_eligible, captured_at_ms,
                  used_micropoints, window_seconds, resets_at_s, plan_type, allowed
           FROM usage_observations;
         DROP TABLE usage_observations;
         ALTER TABLE usage_observations_v4 RENAME TO usage_observations;
         CREATE INDEX usage_observations_stream_capture
           ON usage_observations(account_stream_id, captured_at_ms DESC);
         CREATE TRIGGER usage_observations_are_immutable_update
           BEFORE UPDATE ON usage_observations
           BEGIN
             SELECT RAISE(ABORT, 'usage observations are immutable');
           END;
         CREATE TRIGGER usage_observations_are_immutable_delete
           BEFORE DELETE ON usage_observations
           BEGIN
             SELECT RAISE(ABORT, 'usage observations are immutable');
           END;",
    )?;
    transaction.execute(
        "INSERT INTO schema_migrations
         (version, applied_at_ms, app_version, checksum)
         VALUES (4, ?1, ?2, ?3)",
        rusqlite::params![
            now,
            env!("CARGO_PKG_VERSION"),
            IMMUTABLE_IANA_SCHEMA_CHECKSUM
        ],
    )?;
    transaction.pragma_update(None, "user_version", 4)?;
    transaction.commit()
}

fn reconcile_legacy_source_health(transaction: &rusqlite::Transaction<'_>) -> rusqlite::Result<()> {
    transaction.execute_batch(
        "UPDATE usage_source_health
         SET last_success_at_ms = (
               SELECT MAX(observation.captured_at_ms)
               FROM usage_observations observation
               WHERE observation.account_stream_id =
                       usage_source_health.account_stream_id
                 AND observation.ledger_eligible = 1
             ),
             consecutive_failures = CASE
               WHEN consecutive_failures = 0 THEN 1
               ELSE consecutive_failures
             END,
             public_error = CASE
               WHEN consecutive_failures = 0 THEN 'contract_violation'
               ELSE public_error
             END
         WHERE last_success_at_ms IS NOT NULL
           AND NOT EXISTS (
             SELECT 1 FROM usage_observations observation
             WHERE observation.account_stream_id =
                     usage_source_health.account_stream_id
               AND observation.captured_at_ms =
                     usage_source_health.last_success_at_ms
               AND observation.ledger_eligible = 1
           );",
    )
}

fn migrate_daily_policy_v5(
    database: &mut rusqlite::Connection,
    now: i64,
    fallback_timezone: &str,
) -> rusqlite::Result<()> {
    let transaction =
        database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let policy_timezone: String = transaction.query_row(
        "SELECT policy_timezone FROM app_settings WHERE singleton_id = 1",
        [],
        |row| row.get(0),
    )?;
    let policy_timezone = policy_timezone
        .parse::<chrono_tz::Tz>()
        .or_else(|_| fallback_timezone.parse::<chrono_tz::Tz>())
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let today = chrono::DateTime::from_timestamp_millis(now)
        .ok_or(rusqlite::Error::InvalidQuery)?
        .with_timezone(&policy_timezone)
        .date_naive()
        .to_string();
    transaction.execute_batch(
        "CREATE TABLE policy_revisions (
           id INTEGER PRIMARY KEY,
           revision_key TEXT NOT NULL UNIQUE,
           effective_at_ms INTEGER NOT NULL,
           policy_timezone TEXT NOT NULL,
           carry_workdays_enabled INTEGER NOT NULL
             CHECK (carry_workdays_enabled IN (0, 1)),
           created_at_ms INTEGER NOT NULL
         );
         CREATE TABLE policy_day_limits (
           policy_revision_id INTEGER NOT NULL REFERENCES policy_revisions(id),
           iso_weekday INTEGER NOT NULL CHECK (iso_weekday BETWEEN 1 AND 7),
           base_micropoints INTEGER NOT NULL
             CHECK (base_micropoints BETWEEN 0 AND 100000000),
           PRIMARY KEY(policy_revision_id, iso_weekday)
         );
         CREATE TABLE daily_threshold_transitions (
           id INTEGER PRIMARY KEY,
           account_stream_id INTEGER NOT NULL REFERENCES account_streams(id),
           local_date TEXT NOT NULL,
           policy_revision_id INTEGER NOT NULL REFERENCES policy_revisions(id),
           transition_kind TEXT NOT NULL
             CHECK (transition_kind IN ('warning', 'exceeded')),
           created_at_ms INTEGER NOT NULL,
           UNIQUE(account_stream_id, local_date, transition_kind)
         );",
    )?;
    let revision_id = insert_default_policy_revision(&transaction, now, policy_timezone.name())?;
    rebuild_policy_settings_v5(&transaction, revision_id, policy_timezone.name())?;
    rebuild_daily_ledgers_v5(&transaction, revision_id, &today, now)?;
    create_policy_immutability_triggers_v5(&transaction)?;
    transaction.execute(
        "INSERT INTO schema_migrations
         (version, applied_at_ms, app_version, checksum)
         VALUES (5, ?1, ?2, ?3)",
        rusqlite::params![now, env!("CARGO_PKG_VERSION"), DAILY_POLICY_SCHEMA_CHECKSUM],
    )?;
    transaction.pragma_update(None, "user_version", 5)?;
    transaction.commit()
}

fn migrate_reset_radar_v6(database: &mut rusqlite::Connection, now: i64) -> rusqlite::Result<()> {
    let transaction =
        database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        "CREATE TABLE radar_observations (
           id INTEGER PRIMARY KEY,
           source_id TEXT NOT NULL,
           observed_at_ms INTEGER NOT NULL,
           expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms > observed_at_ms),
           chance_basis_points INTEGER NOT NULL
             CHECK (chance_basis_points BETWEEN 0 AND 10000),
           explanation TEXT NOT NULL,
           source_url TEXT NOT NULL,
           captured_at_ms INTEGER NOT NULL,
           UNIQUE(source_id, observed_at_ms)
         );
         CREATE TABLE radar_announcements (
           id INTEGER PRIMARY KEY,
           source_id TEXT NOT NULL UNIQUE,
           announced_at_ms INTEGER NOT NULL,
           text TEXT NOT NULL,
           source_url TEXT NOT NULL,
           captured_at_ms INTEGER NOT NULL
         );
         CREATE TABLE radar_source_health (
           singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
           last_attempt_at_ms INTEGER NOT NULL,
           last_success_at_ms INTEGER,
           consecutive_failures INTEGER NOT NULL CHECK (consecutive_failures >= 0),
           public_error TEXT,
           current_observation_id INTEGER REFERENCES radar_observations(id)
         );
         CREATE INDEX radar_announcements_time
           ON radar_announcements(announced_at_ms DESC);
         CREATE TRIGGER radar_observations_are_immutable_update
           BEFORE UPDATE ON radar_observations
           BEGIN
             SELECT RAISE(ABORT, 'radar observations are immutable');
           END;
         CREATE TRIGGER radar_observations_are_immutable_delete
           BEFORE DELETE ON radar_observations
           BEGIN
             SELECT RAISE(ABORT, 'radar observations are immutable');
           END;
         CREATE TRIGGER radar_announcements_are_immutable_update
           BEFORE UPDATE ON radar_announcements
           BEGIN
             SELECT RAISE(ABORT, 'radar announcements are immutable');
           END;
         CREATE TRIGGER radar_announcements_are_immutable_delete
           BEFORE DELETE ON radar_announcements
           BEGIN
             SELECT RAISE(ABORT, 'radar announcements are immutable');
           END;",
    )?;
    transaction.execute(
        "INSERT INTO schema_migrations
         (version, applied_at_ms, app_version, checksum)
         VALUES (6, ?1, ?2, ?3)",
        rusqlite::params![now, env!("CARGO_PKG_VERSION"), RESET_RADAR_SCHEMA_CHECKSUM],
    )?;
    transaction.pragma_update(None, "user_version", 6)?;
    transaction.commit()
}

fn migrate_atomic_radar_v7(database: &mut rusqlite::Connection, now: i64) -> rusqlite::Result<()> {
    let transaction =
        database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        "CREATE TABLE radar_observations_v7 (
           id INTEGER PRIMARY KEY,
           source_id TEXT NOT NULL,
           observed_at_ms INTEGER NOT NULL,
           expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms > observed_at_ms),
           chance_basis_points INTEGER NOT NULL
             CHECK (chance_basis_points BETWEEN 0 AND 10000),
           explanation TEXT NOT NULL,
           source_url TEXT NOT NULL,
           captured_at_ms INTEGER NOT NULL,
           UNIQUE(source_id, observed_at_ms, expires_at_ms, chance_basis_points,
                  explanation, source_url)
         );
         INSERT INTO radar_observations_v7
           (id, source_id, observed_at_ms, expires_at_ms, chance_basis_points,
            explanation, source_url, captured_at_ms)
           SELECT id, source_id, observed_at_ms, expires_at_ms, chance_basis_points,
                  explanation, source_url, captured_at_ms
           FROM radar_observations;
         CREATE TABLE radar_source_health_v7 (
           singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
           last_attempt_at_ms INTEGER NOT NULL,
           last_success_at_ms INTEGER,
           consecutive_failures INTEGER NOT NULL CHECK (consecutive_failures >= 0),
           public_error TEXT,
           current_observation_id INTEGER REFERENCES radar_observations_v7(id),
           account_recheck_requested_at_ms INTEGER
         );
         INSERT INTO radar_source_health_v7
           (singleton_id, last_attempt_at_ms, last_success_at_ms,
            consecutive_failures, public_error, current_observation_id,
            account_recheck_requested_at_ms)
           SELECT singleton_id, last_attempt_at_ms, last_success_at_ms,
                  consecutive_failures, public_error, current_observation_id, NULL
           FROM radar_source_health;
         DROP TABLE radar_source_health;
         DROP TRIGGER radar_observations_are_immutable_update;
         DROP TRIGGER radar_observations_are_immutable_delete;
         DROP TABLE radar_observations;
         ALTER TABLE radar_observations_v7 RENAME TO radar_observations;
         ALTER TABLE radar_source_health_v7 RENAME TO radar_source_health;
         CREATE TRIGGER radar_observations_are_immutable_update
           BEFORE UPDATE ON radar_observations
           BEGIN
             SELECT RAISE(ABORT, 'radar observations are immutable');
           END;
         CREATE TRIGGER radar_observations_are_immutable_delete
           BEFORE DELETE ON radar_observations
           BEGIN
             SELECT RAISE(ABORT, 'radar observations are immutable');
           END;",
    )?;
    transaction.execute(
        "INSERT INTO schema_migrations
         (version, applied_at_ms, app_version, checksum)
         VALUES (7, ?1, ?2, ?3)",
        rusqlite::params![now, env!("CARGO_PKG_VERSION"), ATOMIC_RADAR_SCHEMA_CHECKSUM],
    )?;
    transaction.pragma_update(None, "user_version", 7)?;
    transaction.commit()
}

fn migrate_atomic_settings_v8(
    database: &mut rusqlite::Connection,
    now: i64,
) -> rusqlite::Result<()> {
    let transaction =
        database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        "ALTER TABLE app_settings
           ADD COLUMN autostart_enabled INTEGER NOT NULL DEFAULT 0
             CHECK (autostart_enabled IN (0, 1));
         CREATE TABLE alert_preferences (
           event_kind TEXT NOT NULL CHECK (event_kind IN (
             'daily_80', 'daily_100', 'weekly_remaining_20',
             'weekly_remaining_10', 'radar_chance_70',
             'quota_reset_confirmed', 'source_failures_3'
           )),
           channel TEXT NOT NULL CHECK (channel IN ('system', 'email')),
           enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
           updated_at_ms INTEGER NOT NULL,
           PRIMARY KEY(event_kind, channel)
         );
         CREATE TABLE external_change_journal (
           id INTEGER PRIMARY KEY,
           operation_key TEXT NOT NULL UNIQUE,
           kind TEXT NOT NULL CHECK (kind = 'settings'),
           phase TEXT NOT NULL CHECK (phase IN ('prepared', 'committed')),
           old_credential_ref TEXT,
           new_credential_ref TEXT,
           old_autostart_enabled INTEGER NOT NULL
             CHECK (old_autostart_enabled IN (0, 1)),
           new_autostart_enabled INTEGER NOT NULL
             CHECK (new_autostart_enabled IN (0, 1)),
           created_at_ms INTEGER NOT NULL,
           updated_at_ms INTEGER NOT NULL
         );
         CREATE UNIQUE INDEX one_prepared_settings_change
           ON external_change_journal(kind) WHERE phase = 'prepared';",
    )?;
    for event_kind in AlertEventKind::ALL {
        for channel in AlertChannel::ALL {
            transaction.execute(
                "INSERT INTO alert_preferences
                 (event_kind, channel, enabled, updated_at_ms)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    event_kind.as_str(),
                    channel.as_str(),
                    i64::from(channel == AlertChannel::System),
                    now
                ],
            )?;
        }
    }
    transaction.execute(
        "INSERT INTO schema_migrations
         (version, applied_at_ms, app_version, checksum)
         VALUES (8, ?1, ?2, ?3)",
        rusqlite::params![
            now,
            env!("CARGO_PKG_VERSION"),
            ATOMIC_SETTINGS_SCHEMA_CHECKSUM
        ],
    )?;
    transaction.pragma_update(None, "user_version", 8)?;
    transaction.commit()
}

fn migrate_durable_alerts_v9(
    database: &mut rusqlite::Connection,
    now: i64,
) -> rusqlite::Result<()> {
    let transaction =
        database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        "ALTER TABLE app_settings
           ADD COLUMN notification_permission_status TEXT NOT NULL DEFAULT 'unknown'
             CHECK (notification_permission_status IN (
               'unknown', 'granted', 'denied', 'error'
             ));
         CREATE TABLE alert_events (
           id INTEGER PRIMARY KEY,
           event_key TEXT NOT NULL UNIQUE,
           event_kind TEXT NOT NULL CHECK (event_kind IN (
             'daily_80', 'daily_100', 'weekly_remaining_20',
             'weekly_remaining_10', 'radar_chance_70',
             'quota_reset_confirmed', 'source_failures_3'
           )),
           account_stream_id INTEGER REFERENCES account_streams(id),
           quota_epoch_id INTEGER REFERENCES quota_epochs(id),
           local_date TEXT,
           watch_key TEXT,
           source TEXT CHECK (source IS NULL OR source IN ('codex', 'radar')),
           threshold_micropoints INTEGER CHECK (
             threshold_micropoints IS NULL OR threshold_micropoints >= 0
           ),
           message_key TEXT NOT NULL,
           structured_args_json TEXT NOT NULL,
           interface_locale_snapshot TEXT NOT NULL,
           format_locale_snapshot TEXT NOT NULL,
           policy_timezone_snapshot TEXT NOT NULL,
           target TEXT NOT NULL CHECK (target IN ('today', 'radar', 'source')),
           created_at_ms INTEGER NOT NULL
         );
         CREATE TABLE alert_deliveries (
           id INTEGER PRIMARY KEY,
           delivery_key TEXT NOT NULL UNIQUE,
           alert_event_id INTEGER NOT NULL REFERENCES alert_events(id),
           channel TEXT NOT NULL CHECK (channel IN ('system', 'email')),
           recipient_key BLOB,
           state TEXT NOT NULL CHECK (state IN (
             'pending', 'leased', 'delivered', 'retry_wait',
             'paused_permission', 'paused_config', 'cancelled_by_config',
             'permanent_failure'
           )),
           attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
           next_attempt_at_ms INTEGER,
           lease_owner TEXT,
           lease_until_ms INTEGER,
           public_error_code TEXT,
           created_at_ms INTEGER NOT NULL,
           updated_at_ms INTEGER NOT NULL
         );
         CREATE INDEX alert_deliveries_due
           ON alert_deliveries(state, next_attempt_at_ms, lease_until_ms);
         CREATE TABLE delivery_attempts (
           id INTEGER PRIMARY KEY,
           delivery_id INTEGER NOT NULL REFERENCES alert_deliveries(id),
           attempted_at_ms INTEGER NOT NULL,
           outcome TEXT NOT NULL CHECK (outcome IN (
             'delivered', 'transient_failure', 'permanent_failure',
             'permission_denied'
           )),
           public_error_code TEXT,
           duration_ms INTEGER NOT NULL CHECK (duration_ms >= 0)
         );
         CREATE TRIGGER alert_events_are_immutable_update
           BEFORE UPDATE ON alert_events BEGIN
             SELECT RAISE(ABORT, 'alert events are immutable');
           END;
         CREATE TRIGGER alert_events_are_immutable_delete
           BEFORE DELETE ON alert_events BEGIN
             SELECT RAISE(ABORT, 'alert events are immutable');
           END;
         CREATE TRIGGER delivery_attempts_are_immutable_update
           BEFORE UPDATE ON delivery_attempts BEGIN
             SELECT RAISE(ABORT, 'delivery attempts are immutable');
           END;
         CREATE TRIGGER delivery_attempts_are_immutable_delete
           BEFORE DELETE ON delivery_attempts BEGIN
             SELECT RAISE(ABORT, 'delivery attempts are immutable');
           END;",
    )?;
    transaction.execute(
        "INSERT INTO schema_migrations
         (version, applied_at_ms, app_version, checksum)
         VALUES (9, ?1, ?2, ?3)",
        rusqlite::params![
            now,
            env!("CARGO_PKG_VERSION"),
            DURABLE_ALERTS_SCHEMA_CHECKSUM
        ],
    )?;
    transaction.pragma_update(None, "user_version", 9)?;
    transaction.commit()
}

fn insert_default_policy_revision(
    transaction: &rusqlite::Transaction<'_>,
    now: i64,
    policy_timezone: &str,
) -> rusqlite::Result<i64> {
    transaction.execute(
        "INSERT INTO policy_revisions
         (revision_key, effective_at_ms, policy_timezone,
          carry_workdays_enabled, created_at_ms)
         VALUES (?1, ?2, ?3, 1, ?2)",
        rusqlite::params![Uuid::now_v7().to_string(), now, policy_timezone],
    )?;
    let revision_id = transaction.last_insert_rowid();
    let policy =
        QuotaLedger::default_policy(policy_timezone).map_err(|_| rusqlite::Error::InvalidQuery)?;
    for (index, base) in policy.base_micropoints().into_iter().enumerate() {
        transaction.execute(
            "INSERT INTO policy_day_limits
             (policy_revision_id, iso_weekday, base_micropoints)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![revision_id, i64::try_from(index).unwrap_or(0) + 1, base],
        )?;
    }
    Ok(revision_id)
}

fn rebuild_policy_settings_v5(
    transaction: &rusqlite::Transaction<'_>,
    revision_id: i64,
    policy_timezone: &str,
) -> rusqlite::Result<()> {
    transaction.execute_batch(
        "CREATE TABLE app_settings_v5 (
           singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
           auth_path TEXT,
           configured_account_stream_id INTEGER REFERENCES account_streams(id),
           active_account_stream_id INTEGER REFERENCES account_streams(id),
           created_at_ms INTEGER NOT NULL,
           updated_at_ms INTEGER NOT NULL,
           policy_timezone TEXT NOT NULL,
           active_policy_revision_id INTEGER NOT NULL
             REFERENCES policy_revisions(id)
         );",
    )?;
    transaction.execute(
        "INSERT INTO app_settings_v5
         (singleton_id, auth_path, configured_account_stream_id,
          active_account_stream_id, created_at_ms, updated_at_ms,
          policy_timezone, active_policy_revision_id)
         SELECT singleton_id, auth_path, configured_account_stream_id,
                active_account_stream_id, created_at_ms, updated_at_ms,
                ?1, ?2
         FROM app_settings",
        rusqlite::params![policy_timezone, revision_id],
    )?;
    transaction.execute_batch(
        "DROP TABLE app_settings;
         ALTER TABLE app_settings_v5 RENAME TO app_settings;",
    )
}

fn rebuild_daily_ledgers_v5(
    transaction: &rusqlite::Transaction<'_>,
    revision_id: i64,
    today: &str,
    now: i64,
) -> rusqlite::Result<()> {
    transaction.execute_batch(
        "CREATE TABLE daily_ledgers_v5 (
           id INTEGER PRIMARY KEY,
           account_stream_id INTEGER NOT NULL REFERENCES account_streams(id),
           local_date TEXT NOT NULL,
           policy_timezone TEXT NOT NULL,
           used_micropoints INTEGER CHECK (
             used_micropoints IS NULL OR used_micropoints >= 0
           ),
           policy_revision_id INTEGER NOT NULL REFERENCES policy_revisions(id),
           base_micropoints INTEGER NOT NULL CHECK (base_micropoints >= 0),
           carry_micropoints INTEGER NOT NULL CHECK (carry_micropoints >= 0),
           policy_status TEXT NOT NULL CHECK (
             policy_status IN ('unknown', 'normal', 'warning', 'exceeded', 'finalized')
           ),
           finalized_at_ms INTEGER,
           updated_at_ms INTEGER NOT NULL,
           UNIQUE(account_stream_id, local_date, policy_timezone)
         );",
    )?;
    transaction.execute(
        "INSERT INTO daily_ledgers_v5
         (id, account_stream_id, local_date, policy_timezone,
          used_micropoints, policy_revision_id, base_micropoints,
          carry_micropoints, policy_status, finalized_at_ms, updated_at_ms)
         SELECT id, account_stream_id, local_date, policy_timezone,
                used_micropoints, ?1,
                CASE CAST(strftime('%w', local_date) AS INTEGER)
               WHEN 0 THEN 10000000
               WHEN 6 THEN 10000000
               ELSE 16000000
             END, 0,
             CASE
               WHEN local_date < ?2 THEN 'finalized'
               WHEN used_micropoints >=
                    CASE CAST(strftime('%w', local_date) AS INTEGER)
                      WHEN 0 THEN 10000000
                      WHEN 6 THEN 10000000
                      ELSE 16000000
                    END THEN 'exceeded'
               WHEN used_micropoints * 5 >=
                    CASE CAST(strftime('%w', local_date) AS INTEGER)
                      WHEN 0 THEN 10000000
                      WHEN 6 THEN 10000000
                      ELSE 16000000
                    END * 4 THEN 'warning'
               ELSE 'normal'
             END,
             CASE WHEN local_date < ?2 THEN ?3 ELSE NULL END,
             updated_at_ms
         FROM daily_ledgers",
        rusqlite::params![revision_id, today, now],
    )?;
    transaction.execute_batch(
        "DROP TABLE daily_ledgers;
         ALTER TABLE daily_ledgers_v5 RENAME TO daily_ledgers;",
    )
}

fn create_policy_immutability_triggers_v5(
    transaction: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<()> {
    transaction.execute_batch(
        "CREATE TRIGGER policy_revisions_are_immutable_update
           BEFORE UPDATE ON policy_revisions BEGIN
             SELECT RAISE(ABORT, 'policy revisions are immutable');
           END;
         CREATE TRIGGER policy_revisions_are_immutable_delete
           BEFORE DELETE ON policy_revisions BEGIN
             SELECT RAISE(ABORT, 'policy revisions are immutable');
           END;
         CREATE TRIGGER policy_day_limits_are_immutable_update
           BEFORE UPDATE ON policy_day_limits BEGIN
             SELECT RAISE(ABORT, 'policy day limits are immutable');
           END;
         CREATE TRIGGER policy_day_limits_are_immutable_delete
           BEFORE DELETE ON policy_day_limits BEGIN
             SELECT RAISE(ABORT, 'policy day limits are immutable');
           END;
         CREATE TRIGGER finalized_daily_ledgers_are_immutable_update
           BEFORE UPDATE ON daily_ledgers
           WHEN OLD.finalized_at_ms IS NOT NULL BEGIN
             SELECT RAISE(ABORT, 'finalized daily ledgers are immutable');
           END;
         CREATE TRIGGER finalized_daily_ledgers_are_immutable_delete
           BEFORE DELETE ON daily_ledgers
           WHEN OLD.finalized_at_ms IS NOT NULL BEGIN
             SELECT RAISE(ABORT, 'finalized daily ledgers are immutable');
           END;
         CREATE TRIGGER daily_threshold_transitions_are_immutable_update
           BEFORE UPDATE ON daily_threshold_transitions BEGIN
             SELECT RAISE(ABORT, 'daily threshold transitions are immutable');
           END;
         CREATE TRIGGER daily_threshold_transitions_are_immutable_delete
           BEFORE DELETE ON daily_threshold_transitions BEGIN
             SELECT RAISE(ABORT, 'daily threshold transitions are immutable');
           END;",
    )
}

fn validate_migration(
    database: &rusqlite::Connection,
    version: i64,
    expected_checksum: &str,
) -> rusqlite::Result<()> {
    let checksum: String = database.query_row(
        "SELECT checksum FROM schema_migrations WHERE version = ?1",
        [version],
        |row| row.get(0),
    )?;
    if checksum != expected_checksum {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(())
}

fn load_public_account_settings(
    database: &rusqlite::Connection,
) -> rusqlite::Result<PublicAccountSettings> {
    let (revision, path, account_key): (i64, Option<String>, Option<Vec<u8>>) = database
        .query_row(
            "SELECT m.settings_revision, s.auth_path, a.account_key
             FROM app_meta m
             JOIN app_settings s ON s.singleton_id = m.singleton_id
             LEFT JOIN account_streams a ON a.id = s.configured_account_stream_id
             WHERE m.singleton_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
    Ok(PublicAccountSettings {
        settings_revision: u32::try_from(revision).map_err(|_| rusqlite::Error::InvalidQuery)?,
        configured: path.is_some() && account_key.is_some(),
        path_summary: path.as_ref().map(|_| "…/auth.json".to_owned()),
        account_label: account_key.as_deref().map(account_label),
        quota_policy: load_public_quota_policy(database)?,
    })
}

fn load_public_quota_policy(
    database: &rusqlite::Connection,
) -> rusqlite::Result<PublicQuotaPolicy> {
    let (revision, timezone, carry): (i64, String, i64) = database.query_row(
        "SELECT revision.id, revision.policy_timezone,
                revision.carry_workdays_enabled
         FROM app_settings settings
         JOIN policy_revisions revision
           ON revision.id = settings.active_policy_revision_id
         WHERE settings.singleton_id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let mut statement = database.prepare(
        "SELECT base_micropoints
         FROM policy_day_limits
         WHERE policy_revision_id = ?1
         ORDER BY iso_weekday",
    )?;
    let base_micropoints = statement
        .query_map([revision], |row| row.get::<_, i64>(0))?
        .map(|value| {
            value.and_then(|value| u32::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery))
        })
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if base_micropoints.len() != 7 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(PublicQuotaPolicy {
        policy_revision: u64::try_from(revision).map_err(|_| rusqlite::Error::InvalidQuery)?,
        policy_timezone: timezone,
        carry_workdays_enabled: carry != 0,
        base_micropoints,
    })
}

fn load_public_atomic_settings(
    database: &rusqlite::Connection,
) -> rusqlite::Result<PublicSettings> {
    let account = load_public_account_settings(database)?;
    let (autostart_enabled, notification_permission_status): (i64, String) = database.query_row(
        "SELECT autostart_enabled, notification_permission_status
         FROM app_settings WHERE singleton_id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let mut statement = database.prepare(
        "SELECT event_kind, channel, enabled
         FROM alert_preferences
         ORDER BY event_kind, channel",
    )?;
    let alert_preferences = statement
        .query_map([], |row| {
            let event_kind: String = row.get(0)?;
            let channel: String = row.get(1)?;
            let enabled: i64 = row.get(2)?;
            Ok(AlertPreference {
                event_kind: AlertEventKind::parse(&event_kind)?,
                channel: AlertChannel::parse(&channel)?,
                enabled: enabled != 0,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if alert_preferences.len() != AlertEventKind::ALL.len() * AlertChannel::ALL.len() {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(PublicSettings {
        settings_revision: account.settings_revision,
        configured: account.configured,
        path_summary: account.path_summary,
        account_label: account.account_label,
        notification_permission_status: NotificationPermissionStatus::parse(
            &notification_permission_status,
        )
        .map_err(|_| rusqlite::Error::InvalidQuery)?,
        quota_policy: account.quota_policy,
        alert_preferences,
        autostart_enabled: autostart_enabled != 0,
    })
}

fn validate_quota_policy_draft(draft: QuotaPolicyDraft) -> Result<QuotaPolicy, PolicyError> {
    let base_micropoints: [i64; 7] = draft
        .base_micropoints
        .try_into()
        .map_err(|_| PolicyError::InvalidDayCount)?;
    QuotaLedger::validate_policy(
        base_micropoints,
        draft.carry_workdays_enabled,
        &draft.policy_timezone,
    )
}

fn validate_alert_preferences(
    preferences: &[AlertPreferenceDraft],
) -> Result<(), SettingsStoreError> {
    if preferences.len() != AlertEventKind::ALL.len() * AlertChannel::ALL.len() {
        return Err(SettingsStoreError::InvalidAlertPreferences);
    }
    let actual = preferences
        .iter()
        .map(|preference| (preference.event_kind, preference.channel))
        .collect::<BTreeSet<_>>();
    let expected = AlertEventKind::ALL
        .into_iter()
        .flat_map(|event_kind| {
            AlertChannel::ALL
                .into_iter()
                .map(move |channel| (event_kind, channel))
        })
        .collect::<BTreeSet<_>>();
    if actual == expected {
        Ok(())
    } else {
        Err(SettingsStoreError::InvalidAlertPreferences)
    }
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
        let policy_timezone = iana_time_zone::get_timezone()
            .ok()
            .filter(|timezone| timezone.parse::<chrono_tz::Tz>().is_ok())
            .unwrap_or_else(|| "Asia/Shanghai".to_owned());
        Self::open_with_policy_timezone(path, policy_timezone).await
    }

    /// Opens with an explicit IANA policy timezone.
    ///
    /// This seam keeps natural-day behavior deterministic in tests and is
    /// superseded by the persisted user setting once configured.
    ///
    /// # Errors
    ///
    /// Returns a database error when the timezone is invalid or initialization
    /// fails.
    pub async fn open_with_policy_timezone(
        path: impl AsRef<Path>,
        policy_timezone: impl AsRef<str>,
    ) -> Result<Self, SettingsStoreError> {
        let policy_timezone = policy_timezone.as_ref().to_owned();
        policy_timezone
            .parse::<chrono_tz::Tz>()
            .map_err(SettingsStoreError::database)?;
        let connection = Connection::open(path)
            .await
            .map_err(SettingsStoreError::database)?;
        let salt = new_salt().map_err(SettingsStoreError::database)?;
        let app_instance_id = Uuid::now_v7().to_string();
        let now = unix_time_ms();
        connection
            .call(move |database| {
                initialize_database(database, &salt, &app_instance_id, now, &policy_timezone)
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
                let transaction =
                    database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let (salt, revision): (Vec<u8>, i64) = transaction.query_row(
                    "SELECT local_hash_salt, settings_revision
                     FROM app_meta WHERE singleton_id = 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                if revision != i64::from(expected_revision) {
                    return Err(StoreCallError::Conflict);
                }
                let now = unix_time_ms();
                let (stream_id, account_key) =
                    upsert_account_stream(&transaction, &salt, &account_id, now)?;
                transaction.execute(
                    "UPDATE app_settings
                     SET auth_path = ?1, configured_account_stream_id = ?2,
                         updated_at_ms = ?3
                     WHERE singleton_id = 1",
                    rusqlite::params![path, stream_id, now],
                )?;
                transaction.execute(
                    "UPDATE app_meta
                     SET settings_revision = settings_revision + 1,
                         dashboard_revision = dashboard_revision + 1,
                         updated_at_ms = ?1
                     WHERE singleton_id = 1",
                    [now],
                )?;
                let quota_policy = load_public_quota_policy(&transaction)?;
                transaction.commit()?;
                Ok(PublicAccountSettings {
                    settings_revision: expected_revision.saturating_add(1),
                    configured: true,
                    path_summary: Some("…/auth.json".to_owned()),
                    account_label: Some(account_label(&account_key)),
                    quota_policy,
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
            .call(|database| load_public_account_settings(database))
            .await
            .map_err(SettingsStoreError::database)
    }

    /// Reads every current non-secret setting as one revisioned projection.
    ///
    /// # Errors
    ///
    /// Returns a database error when the projection cannot be read.
    pub async fn public_atomic_settings(&self) -> Result<PublicSettings, SettingsStoreError> {
        self.connection
            .call(|database| load_public_atomic_settings(database))
            .await
            .map_err(SettingsStoreError::database)
    }

    async fn prepare_external_settings_change(
        &self,
        expected_revision: u32,
        old_autostart_enabled: bool,
        new_autostart_enabled: bool,
    ) -> Result<String, SettingsStoreError> {
        let operation_key = Uuid::now_v7().to_string();
        let returned_key = operation_key.clone();
        self.connection
            .call(move |database| {
                let transaction =
                    database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let current_revision: i64 = transaction.query_row(
                    "SELECT settings_revision FROM app_meta WHERE singleton_id = 1",
                    [],
                    |row| row.get(0),
                )?;
                if current_revision != i64::from(expected_revision) {
                    return Err(StoreCallError::Conflict);
                }
                let now = unix_time_ms();
                transaction.execute(
                    "INSERT INTO external_change_journal
                     (operation_key, kind, phase, old_credential_ref,
                      new_credential_ref, old_autostart_enabled,
                      new_autostart_enabled, created_at_ms, updated_at_ms)
                     VALUES (?1, 'settings', 'prepared', NULL, NULL, ?2, ?3, ?4, ?4)",
                    rusqlite::params![
                        operation_key,
                        i64::from(old_autostart_enabled),
                        i64::from(new_autostart_enabled),
                        now
                    ],
                )?;
                transaction.commit()?;
                Ok::<(), StoreCallError>(())
            })
            .await
            .map_err(|error| match error {
                tokio_rusqlite::Error::Error(StoreCallError::Conflict) => {
                    SettingsStoreError::Conflict
                }
                other => SettingsStoreError::database(other),
            })?;
        Ok(returned_key)
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    async fn commit_atomic_settings(
        &self,
        operation_key: &str,
        expected_revision: u32,
        account_candidate: Option<ValidatedAccountCandidate>,
        policy: QuotaPolicy,
        alert_preferences: Vec<AlertPreferenceDraft>,
        autostart_enabled: bool,
    ) -> Result<PublicSettings, SettingsStoreError> {
        let operation_key = operation_key.to_owned();
        let policy_timezone = policy.policy_timezone().name().to_owned();
        let carry_workdays_enabled = policy.carry_workdays_enabled();
        let base_micropoints = policy.base_micropoints();
        self.connection
            .call(move |database| {
                let transaction =
                    database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let (salt, current_revision): (Vec<u8>, i64) = transaction.query_row(
                    "SELECT local_hash_salt, settings_revision
                     FROM app_meta WHERE singleton_id = 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                if current_revision != i64::from(expected_revision) {
                    return Err(StoreCallError::Conflict);
                }
                let prepared: i64 = transaction.query_row(
                    "SELECT COUNT(*) FROM external_change_journal
                     WHERE operation_key = ?1 AND kind = 'settings' AND phase = 'prepared'",
                    [&operation_key],
                    |row| row.get(0),
                )?;
                if prepared != 1 {
                    return Err(rusqlite::Error::InvalidQuery.into());
                }

                let now = unix_time_ms();
                let configured_stream_id: Option<i64> = transaction.query_row(
                    "SELECT configured_account_stream_id
                     FROM app_settings WHERE singleton_id = 1",
                    [],
                    |row| row.get(0),
                )?;
                if let Some(stream_id) = configured_stream_id {
                    persist_daily_policy_snapshots(&transaction, stream_id, now)?;
                }

                let new_account = account_candidate
                    .map(|candidate| {
                        let (stream_id, _) = upsert_account_stream(
                            &transaction,
                            &salt,
                            &candidate.canonical_account_id,
                            now,
                        )?;
                        Ok::<_, rusqlite::Error>((stream_id, candidate.canonical_path))
                    })
                    .transpose()?;

                transaction.execute(
                    "INSERT INTO policy_revisions
                     (revision_key, effective_at_ms, policy_timezone,
                      carry_workdays_enabled, created_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?2)",
                    rusqlite::params![
                        Uuid::now_v7().to_string(),
                        now,
                        policy_timezone,
                        i64::from(carry_workdays_enabled)
                    ],
                )?;
                let policy_revision_id = transaction.last_insert_rowid();
                for (index, base) in base_micropoints.into_iter().enumerate() {
                    transaction.execute(
                        "INSERT INTO policy_day_limits
                         (policy_revision_id, iso_weekday, base_micropoints)
                         VALUES (?1, ?2, ?3)",
                        rusqlite::params![
                            policy_revision_id,
                            i64::try_from(index).map_err(|_| rusqlite::Error::InvalidQuery)? + 1,
                            base
                        ],
                    )?;
                }
                if let Some((stream_id, path)) = new_account {
                    transaction.execute(
                        "UPDATE app_settings
                         SET auth_path = ?1, configured_account_stream_id = ?2,
                             active_policy_revision_id = ?3, policy_timezone = ?4,
                             autostart_enabled = ?5, updated_at_ms = ?6
                         WHERE singleton_id = 1",
                        rusqlite::params![
                            path,
                            stream_id,
                            policy_revision_id,
                            policy_timezone,
                            i64::from(autostart_enabled),
                            now
                        ],
                    )?;
                } else {
                    transaction.execute(
                        "UPDATE app_settings
                         SET active_policy_revision_id = ?1, policy_timezone = ?2,
                             autostart_enabled = ?3, updated_at_ms = ?4
                         WHERE singleton_id = 1",
                        rusqlite::params![
                            policy_revision_id,
                            policy_timezone,
                            i64::from(autostart_enabled),
                            now
                        ],
                    )?;
                }
                for preference in alert_preferences {
                    transaction.execute(
                        "UPDATE alert_preferences
                         SET enabled = ?1, updated_at_ms = ?2
                         WHERE event_kind = ?3 AND channel = ?4",
                        rusqlite::params![
                            i64::from(preference.enabled),
                            now,
                            preference.event_kind.as_str(),
                            preference.channel.as_str()
                        ],
                    )?;
                }
                if let Some(stream_id) = configured_stream_id {
                    persist_daily_policy_snapshots(&transaction, stream_id, now)?;
                }
                transaction.execute(
                    "UPDATE app_meta
                     SET settings_revision = settings_revision + 1,
                         dashboard_revision = dashboard_revision + 1,
                         updated_at_ms = ?1
                     WHERE singleton_id = 1",
                    [now],
                )?;
                transaction.execute(
                    "UPDATE external_change_journal
                     SET phase = 'committed', updated_at_ms = ?1
                     WHERE operation_key = ?2",
                    rusqlite::params![now, operation_key],
                )?;
                let settings = load_public_atomic_settings(&transaction)?;
                transaction.commit()?;
                Ok(settings)
            })
            .await
            .map_err(|error| match error {
                tokio_rusqlite::Error::Error(StoreCallError::Conflict) => {
                    SettingsStoreError::Conflict
                }
                other => SettingsStoreError::database(other),
            })
    }

    async fn clear_external_change(&self, operation_key: &str) -> Result<(), SettingsStoreError> {
        let operation_key = operation_key.to_owned();
        self.connection
            .call(move |database| {
                database.execute(
                    "DELETE FROM external_change_journal WHERE operation_key = ?1",
                    [operation_key],
                )?;
                Ok::<(), rusqlite::Error>(())
            })
            .await
            .map_err(SettingsStoreError::database)
    }

    async fn external_changes(&self) -> Result<Vec<ExternalSettingsChange>, SettingsStoreError> {
        self.connection
            .call(|database| {
                let mut statement = database.prepare(
                    "SELECT operation_key, phase, old_autostart_enabled,
                            new_autostart_enabled
                     FROM external_change_journal
                     WHERE kind = 'settings'
                     ORDER BY id",
                )?;
                statement
                    .query_map([], |row| {
                        let phase: String = row.get(1)?;
                        let phase = match phase.as_str() {
                            "prepared" => ExternalChangePhase::Prepared,
                            "committed" => ExternalChangePhase::Committed,
                            _ => return Err(rusqlite::Error::InvalidQuery),
                        };
                        Ok(ExternalSettingsChange {
                            operation_key: row.get(0)?,
                            phase,
                            old_autostart_enabled: row.get::<_, i64>(2)? != 0,
                            new_autostart_enabled: row.get::<_, i64>(3)? != 0,
                        })
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .await
            .map_err(SettingsStoreError::database)
    }

    /// Appends and activates one complete, validated daily-policy revision.
    ///
    /// # Errors
    ///
    /// Returns a validation error without writing anything, a conflict for a
    /// stale settings revision, or a storage error if the transaction fails.
    pub async fn update_quota_policy(
        &self,
        expected_revision: u32,
        draft: QuotaPolicyDraft,
    ) -> Result<PublicAccountSettings, SettingsStoreError> {
        self.update_quota_policy_at(expected_revision, draft, unix_time_ms())
            .await
    }

    /// Deterministic clock seam for migration and policy timeline tests.
    #[doc(hidden)]
    pub async fn update_quota_policy_at(
        &self,
        expected_revision: u32,
        draft: QuotaPolicyDraft,
        now_unix_ms: i64,
    ) -> Result<PublicAccountSettings, SettingsStoreError> {
        let base_micropoints: [i64; 7] = draft
            .base_micropoints
            .try_into()
            .map_err(|_| PolicyError::InvalidDayCount)?;
        let policy = QuotaLedger::validate_policy(
            base_micropoints,
            draft.carry_workdays_enabled,
            &draft.policy_timezone,
        )?;
        let policy_timezone = policy.policy_timezone().name().to_owned();
        let carry_workdays_enabled = policy.carry_workdays_enabled();
        let base_micropoints = policy.base_micropoints();
        let now = now_unix_ms;
        self.connection
            .call(move |database| {
                let transaction =
                    database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let current_revision: i64 = transaction.query_row(
                    "SELECT settings_revision FROM app_meta WHERE singleton_id = 1",
                    [],
                    |row| row.get(0),
                )?;
                if current_revision != i64::from(expected_revision) {
                    return Err(StoreCallError::Conflict);
                }
                let configured_stream_id: Option<i64> = transaction.query_row(
                    "SELECT configured_account_stream_id
                     FROM app_settings WHERE singleton_id = 1",
                    [],
                    |row| row.get(0),
                )?;
                if let Some(stream_id) = configured_stream_id {
                    // Freeze completed dates under the revision that governed
                    // them before activating the replacement.
                    persist_daily_policy_snapshots(&transaction, stream_id, now)?;
                }
                transaction.execute(
                    "INSERT INTO policy_revisions
                     (revision_key, effective_at_ms, policy_timezone,
                      carry_workdays_enabled, created_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?2)",
                    rusqlite::params![
                        Uuid::now_v7().to_string(),
                        now,
                        policy_timezone,
                        i64::from(carry_workdays_enabled)
                    ],
                )?;
                let policy_revision_id = transaction.last_insert_rowid();
                for (index, base) in base_micropoints.into_iter().enumerate() {
                    transaction.execute(
                        "INSERT INTO policy_day_limits
                         (policy_revision_id, iso_weekday, base_micropoints)
                         VALUES (?1, ?2, ?3)",
                        rusqlite::params![
                            policy_revision_id,
                            i64::try_from(index).map_err(|_| rusqlite::Error::InvalidQuery)? + 1,
                            base
                        ],
                    )?;
                }
                transaction.execute(
                    "UPDATE app_settings
                     SET active_policy_revision_id = ?1,
                         policy_timezone = ?2,
                         updated_at_ms = ?3
                     WHERE singleton_id = 1",
                    rusqlite::params![policy_revision_id, policy_timezone, now],
                )?;
                if let Some(stream_id) = configured_stream_id {
                    persist_daily_policy_snapshots(&transaction, stream_id, now)?;
                }
                transaction.execute(
                    "UPDATE app_meta
                     SET settings_revision = settings_revision + 1,
                         dashboard_revision = dashboard_revision + 1,
                         updated_at_ms = ?1
                     WHERE singleton_id = 1",
                    [now],
                )?;
                let settings = load_public_account_settings(&transaction)?;
                transaction.commit()?;
                Ok(settings)
            })
            .await
            .map_err(|error| match error {
                tokio_rusqlite::Error::Error(StoreCallError::Conflict) => {
                    SettingsStoreError::Conflict
                }
                other => SettingsStoreError::database(other),
            })
    }

    /// Captures the selected path and settings revision for one refresh round.
    ///
    /// # Errors
    ///
    /// Returns a database error when settings cannot be read.
    pub async fn configured_refresh_binding(
        &self,
    ) -> Result<Option<RefreshAccountBinding>, SettingsStoreError> {
        self.connection
            .call(|database| {
                database.query_row(
                    "SELECT m.settings_revision, s.auth_path
                     FROM app_meta m
                     JOIN app_settings s ON s.singleton_id = m.singleton_id
                     WHERE m.singleton_id = 1",
                    [],
                    |row| {
                        let revision: i64 = row.get(0)?;
                        let path: Option<String> = row.get(1)?;
                        let Some(path) = path else {
                            return Ok(None);
                        };
                        let revision =
                            u32::try_from(revision).map_err(|_| rusqlite::Error::InvalidQuery)?;
                        Ok(Some(RefreshAccountBinding::selected(
                            revision,
                            std::path::PathBuf::from(path),
                        )))
                    },
                )
            })
            .await
            .map_err(SettingsStoreError::database)
    }

    /// Commits both source results from one refresh flight in one transaction
    /// and publishes at most one dashboard revision.
    ///
    /// # Errors
    ///
    /// Returns a database error when the complete transaction cannot commit.
    pub(crate) async fn record_refresh_attempt(
        &self,
        usage_attempt: UsageRefreshAttempt,
        radar_attempt: Option<Result<RadarSnapshot, RadarSourceError>>,
        attempted_at_unix_ms: i64,
    ) -> Result<CombinedRefreshDisposition, SettingsStoreError> {
        self.connection
            .call(move |database| {
                let transaction =
                    database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let (outcome, account_checked, settings_increment, usage_changed) =
                    match (usage_attempt.binding.as_ref(), usage_attempt.result) {
                        (Some(binding), Ok(observation)) => {
                            let (disposition, settings_increment, changed) =
                                apply_usage_success(&transaction, binding, &observation)?;
                            (
                                match disposition {
                                    UsageCommitDisposition::Committed => RefreshOutcome::Updated,
                                    UsageCommitDisposition::Superseded => {
                                        RefreshOutcome::Superseded
                                    }
                                },
                                disposition == UsageCommitDisposition::Committed,
                                settings_increment,
                                changed,
                            )
                        }
                        (None, Ok(_)) => (RefreshOutcome::Superseded, false, 0, false),
                        (Some(binding), Err(error)) => {
                            let code = error.code();
                            let (disposition, settings_increment) = apply_usage_failure(
                                &transaction,
                                binding,
                                attempted_at_unix_ms,
                                code,
                            )?;
                            (
                                match disposition {
                                    UsageCommitDisposition::Committed => {
                                        RefreshOutcome::Failed(code)
                                    }
                                    UsageCommitDisposition::Superseded => {
                                        RefreshOutcome::Superseded
                                    }
                                },
                                disposition == UsageCommitDisposition::Committed,
                                settings_increment,
                                disposition == UsageCommitDisposition::Committed,
                            )
                        }
                        (None, Err(error)) => {
                            (RefreshOutcome::Failed(error.code()), false, 0, false)
                        }
                    };
                if account_checked {
                    transaction.execute(
                        "UPDATE radar_source_health
                         SET account_recheck_requested_at_ms = NULL
                         WHERE singleton_id = 1
                           AND account_recheck_requested_at_ms < ?1",
                        [attempted_at_unix_ms],
                    )?;
                }
                let radar_changed = match radar_attempt {
                    Some(Ok(snapshot)) => {
                        let disposition =
                            apply_radar_success(&transaction, attempted_at_unix_ms, &snapshot)?;
                        if disposition.new_announcement {
                            transaction.execute(
                                "UPDATE radar_source_health
                                 SET account_recheck_requested_at_ms = ?1
                                 WHERE singleton_id = 1",
                                [attempted_at_unix_ms],
                            )?;
                        }
                        true
                    }
                    Some(Err(error)) => {
                        apply_radar_failure(&transaction, attempted_at_unix_ms, error.code())?;
                        true
                    }
                    None => false,
                };
                if usage_changed || radar_changed {
                    update_dashboard_meta(&transaction, settings_increment, attempted_at_unix_ms)?;
                }
                transaction.commit()?;
                Ok::<_, rusqlite::Error>(CombinedRefreshDisposition { outcome })
            })
            .await
            .map_err(SettingsStoreError::database)
    }

    /// Returns whether an announcement is waiting for a later account refresh.
    ///
    /// # Errors
    ///
    /// Returns a database error when operational state cannot be read.
    pub async fn radar_account_recheck_pending(&self) -> Result<bool, SettingsStoreError> {
        self.connection
            .call(|database| {
                use rusqlite::OptionalExtension as _;
                database
                    .query_row(
                        "SELECT account_recheck_requested_at_ms
                         FROM radar_source_health WHERE singleton_id = 1",
                        [],
                        |row| row.get::<_, Option<i64>>(0),
                    )
                    .optional()
                    .map(|value| value.flatten().is_some())
            })
            .await
            .map_err(SettingsStoreError::database)
    }

    /// Atomically appends a valid observation, resets source health, and
    /// activates the configured account stream.
    ///
    /// # Errors
    ///
    /// Returns a database error when no account is configured or the whole
    /// transaction cannot be committed.
    pub async fn record_usage_success(
        &self,
        binding: &RefreshAccountBinding,
        observation: WeeklyUsageObservation,
    ) -> Result<UsageCommitDisposition, SettingsStoreError> {
        let binding = binding.clone();
        self.connection
            .call(move |database| {
                record_usage_success_transaction(database, &binding, &observation)
            })
            .await
            .map_err(SettingsStoreError::database)
    }

    /// Records one failed attempt while retaining every last-known-good fact.
    ///
    /// # Errors
    ///
    /// Returns a database error when no account is configured or the health
    /// transaction cannot be committed.
    pub async fn record_usage_failure(
        &self,
        binding: &RefreshAccountBinding,
        attempted_at_unix_ms: i64,
        public_error: UsageSourceErrorCode,
    ) -> Result<UsageCommitDisposition, SettingsStoreError> {
        let binding = binding.clone();
        self.connection
            .call(move |database| {
                let transaction =
                    database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let (disposition, settings_increment) = apply_usage_failure(
                    &transaction,
                    &binding,
                    attempted_at_unix_ms,
                    public_error,
                )?;
                if disposition == UsageCommitDisposition::Committed {
                    update_dashboard_meta(&transaction, settings_increment, attempted_at_unix_ms)?;
                }
                transaction.commit().map(|()| disposition)
            })
            .await
            .map_err(SettingsStoreError::database)
    }

    /// Atomically commits Radar facts and resets only Radar source health.
    ///
    /// This transaction has no account-stream or quota-epoch foreign keys.
    ///
    /// # Errors
    ///
    /// Returns a database error if the independent Radar transaction fails.
    pub async fn record_radar_success(
        &self,
        attempted_at_unix_ms: i64,
        snapshot: RadarSnapshot,
    ) -> Result<RadarCommitDisposition, SettingsStoreError> {
        self.connection
            .call(move |database| {
                let transaction =
                    database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let disposition =
                    apply_radar_success(&transaction, attempted_at_unix_ms, &snapshot)?;
                update_dashboard_meta(&transaction, 0, attempted_at_unix_ms)?;
                transaction.commit()?;
                Ok::<_, rusqlite::Error>(disposition)
            })
            .await
            .map_err(SettingsStoreError::database)
    }

    /// Records one Radar failure while retaining its current observation link.
    ///
    /// # Errors
    ///
    /// Returns a database error if the independent health transaction fails.
    pub async fn record_radar_failure(
        &self,
        attempted_at_unix_ms: i64,
        public_error: RadarSourceErrorCode,
    ) -> Result<(), SettingsStoreError> {
        self.connection
            .call(move |database| {
                let transaction =
                    database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                apply_radar_failure(&transaction, attempted_at_unix_ms, public_error)?;
                update_dashboard_meta(&transaction, 0, attempted_at_unix_ms)?;
                transaction.commit()
            })
            .await
            .map_err(SettingsStoreError::database)
    }

    /// Returns the independent Radar projection, hiding expired predictions.
    ///
    /// # Errors
    ///
    /// Returns a database error when persisted state is corrupt.
    pub async fn public_reset_radar(
        &self,
        now_unix_ms: i64,
    ) -> Result<PublicResetRadar, SettingsStoreError> {
        self.connection
            .call(move |database| query_public_reset_radar(database, now_unix_ms))
            .await
            .map_err(SettingsStoreError::database)
    }

    /// Returns the last-known-good quota and current source health for only the
    /// configured account stream.
    ///
    /// # Errors
    ///
    /// Returns a database error when persisted state is corrupt or unavailable.
    pub async fn public_live_quota(
        &self,
        now_unix_ms: i64,
    ) -> Result<Option<PublicLiveQuota>, SettingsStoreError> {
        self.public_live_quota_snapshot(now_unix_ms)
            .await
            .map(|(_, quota)| quota)
    }

    /// Returns the persisted dashboard revision and quota from one read
    /// transaction.
    ///
    /// # Errors
    ///
    /// Returns a database error when persisted state is corrupt or unavailable.
    pub async fn public_live_quota_snapshot(
        &self,
        now_unix_ms: i64,
    ) -> Result<(u64, Option<PublicLiveQuota>), SettingsStoreError> {
        self.connection
            .call(move |database| {
                let transaction = database.unchecked_transaction()?;
                let revision: i64 = transaction.query_row(
                    "SELECT dashboard_revision FROM app_meta WHERE singleton_id = 1",
                    [],
                    |row| row.get(0),
                )?;
                let revision =
                    u64::try_from(revision).map_err(|_| rusqlite::Error::InvalidQuery)?;
                let Some(row) = query_live_quota_row(&transaction)? else {
                    return Ok((revision, None));
                };
                let ledger_days = project_public_ledger_days(
                    &transaction,
                    row.stream_id,
                    row.policy_timezone,
                    now_unix_ms,
                )?;
                build_public_live_quota(row, ledger_days, now_unix_ms)
                    .map(|quota| (revision, Some(quota)))
            })
            .await
            .map_err(SettingsStoreError::database)
    }

    /// Returns one transactionally consistent quota + Radar dashboard snapshot.
    ///
    /// # Errors
    ///
    /// Returns a database error when either independent projection is corrupt.
    pub async fn public_dashboard_snapshot(
        &self,
        now_unix_ms: i64,
    ) -> Result<(u64, Option<PublicLiveQuota>, PublicResetRadar), SettingsStoreError> {
        self.connection
            .call(move |database| {
                let transaction = database.unchecked_transaction()?;
                let revision: i64 = transaction.query_row(
                    "SELECT dashboard_revision FROM app_meta WHERE singleton_id = 1",
                    [],
                    |row| row.get(0),
                )?;
                let revision =
                    u64::try_from(revision).map_err(|_| rusqlite::Error::InvalidQuery)?;
                let quota = if let Some(row) = query_live_quota_row(&transaction)? {
                    let ledger_days = project_public_ledger_days(
                        &transaction,
                        row.stream_id,
                        row.policy_timezone,
                        now_unix_ms,
                    )?;
                    Some(build_public_live_quota(row, ledger_days, now_unix_ms)?)
                } else {
                    None
                };
                let radar = query_public_reset_radar(&transaction, now_unix_ms)?;
                Ok::<_, rusqlite::Error>((revision, quota, radar))
            })
            .await
            .map_err(SettingsStoreError::database)
    }

    /// Returns the newest persisted in-app reminders and system delivery states.
    ///
    /// # Errors
    ///
    /// Returns a storage error when persisted alert state is unavailable.
    pub async fn public_alerts(&self, limit: u32) -> Result<PublicAlertInbox, SettingsStoreError> {
        self.connection
            .call(move |database| {
                let permission: String = database.query_row(
                    "SELECT notification_permission_status
                     FROM app_settings WHERE singleton_id = 1",
                    [],
                    |row| row.get(0),
                )?;
                let mut statement = database.prepare(
                    "SELECT e.id, e.event_kind, e.local_date, e.source, e.target,
                            d.state, e.created_at_ms
                     FROM alert_events e
                     LEFT JOIN alert_deliveries d
                       ON d.alert_event_id = e.id AND d.channel = 'system'
                     ORDER BY e.created_at_ms DESC, e.id DESC LIMIT ?1",
                )?;
                let events = statement
                    .query_map([i64::from(limit.min(100))], |row| {
                        let id: i64 = row.get(0)?;
                        let event_kind: String = row.get(1)?;
                        let target: String = row.get(4)?;
                        let delivery_state: Option<String> = row.get(5)?;
                        Ok(PublicAlertEvent {
                            event_id: u64::try_from(id)
                                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                            event_kind: AlertEventKind::parse(&event_kind)?,
                            local_date: row.get(2)?,
                            source: row.get(3)?,
                            target: AlertTarget::parse(&target)
                                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                            system_delivery_state: delivery_state
                                .map(|state| {
                                    PublicDeliveryState::parse(&state)
                                        .map_err(|_| rusqlite::Error::InvalidQuery)
                                })
                                .transpose()?,
                            created_at_unix_ms: row.get(6)?,
                        })
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok::<_, rusqlite::Error>((permission, events))
            })
            .await
            .map_err(SettingsStoreError::database)
            .and_then(|(permission, events)| {
                Ok(PublicAlertInbox {
                    notification_permission_status: NotificationPermissionStatus::parse(
                        &permission,
                    )?,
                    events,
                })
            })
    }

    /// Persists a platform-reported notification permission state.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the state cannot be committed.
    pub async fn set_notification_permission_status(
        &self,
        status: NotificationPermissionStatus,
        now_unix_ms: i64,
    ) -> Result<(), SettingsStoreError> {
        let status = status.as_str();
        self.connection
            .call(move |database| {
                let transaction =
                    database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let changed = transaction.execute(
                    "UPDATE app_settings
                     SET notification_permission_status = ?1, updated_at_ms = ?2
                     WHERE singleton_id = 1
                       AND notification_permission_status <> ?1",
                    rusqlite::params![status, now_unix_ms],
                )?;
                if changed == 1 {
                    transaction.execute(
                        "UPDATE app_meta
                         SET dashboard_revision = dashboard_revision + 1,
                             updated_at_ms = ?1
                         WHERE singleton_id = 1",
                        [now_unix_ms],
                    )?;
                }
                transaction.commit()
            })
            .await
            .map_err(SettingsStoreError::database)
    }

    pub(crate) async fn pause_system_deliveries(
        &self,
        permission: NotificationPermissionStatus,
        now_unix_ms: i64,
    ) -> Result<u32, SettingsStoreError> {
        let public_error = match permission {
            NotificationPermissionStatus::Denied => "notification_permission_denied",
            NotificationPermissionStatus::Error => "notification_permission_error",
            NotificationPermissionStatus::Unknown => "notification_permission_unknown",
            NotificationPermissionStatus::Granted => return Ok(0),
        };
        self.connection
            .call(move |database| {
                let transaction =
                    database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                if permission == NotificationPermissionStatus::Denied {
                    transaction.execute(
                        "INSERT INTO delivery_attempts
                         (delivery_id, attempted_at_ms, outcome, public_error_code, duration_ms)
                         SELECT id, ?1, 'permission_denied', ?2, 0
                         FROM alert_deliveries
                         WHERE channel = 'system' AND state IN ('pending', 'retry_wait')",
                        rusqlite::params![now_unix_ms, public_error],
                    )?;
                }
                let changed = transaction.execute(
                    "UPDATE alert_deliveries
                     SET state = 'paused_permission', next_attempt_at_ms = NULL,
                         lease_owner = NULL, lease_until_ms = NULL,
                         public_error_code = ?1, updated_at_ms = ?2
                     WHERE channel = 'system'
                       AND state IN ('pending', 'retry_wait')",
                    rusqlite::params![public_error, now_unix_ms],
                )?;
                transaction.commit()?;
                Ok::<_, rusqlite::Error>(changed)
            })
            .await
            .map(|count| u32::try_from(count).unwrap_or(u32::MAX))
            .map_err(SettingsStoreError::database)
    }

    pub(crate) async fn resume_permission_deliveries(
        &self,
        now_unix_ms: i64,
    ) -> Result<(), SettingsStoreError> {
        self.connection
            .call(move |database| {
                database.execute(
                    "UPDATE alert_deliveries
                     SET state = 'pending', next_attempt_at_ms = ?1,
                         public_error_code = NULL, updated_at_ms = ?1
                     WHERE channel = 'system' AND state = 'paused_permission'",
                    [now_unix_ms],
                )?;
                Ok::<_, rusqlite::Error>(())
            })
            .await
            .map_err(SettingsStoreError::database)
    }

    pub(crate) async fn claim_system_deliveries(
        &self,
        worker_id: &str,
        now_unix_ms: i64,
        lease_ms: i64,
        limit: u32,
    ) -> Result<Vec<ClaimedSystemDelivery>, SettingsStoreError> {
        let worker_id = worker_id.to_owned();
        let lease_until_ms = now_unix_ms.saturating_add(lease_ms);
        self.connection
            .call(move |database| {
                let transaction =
                    database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let deliveries = {
                    let mut statement = transaction.prepare(
                        "SELECT d.id, d.delivery_key, e.event_kind, e.target
                         FROM alert_deliveries d
                         JOIN alert_events e ON e.id = d.alert_event_id
                         WHERE d.channel = 'system' AND (
                           d.state = 'pending'
                           OR (d.state = 'retry_wait' AND d.next_attempt_at_ms <= ?1)
                           OR (d.state = 'leased' AND d.lease_until_ms <= ?1)
                         )
                         ORDER BY d.created_at_ms, d.id LIMIT ?2",
                    )?;
                    statement
                        .query_map(
                            rusqlite::params![now_unix_ms, i64::from(limit.min(100))],
                            |row| {
                                let event_kind: String = row.get(2)?;
                                let target: String = row.get(3)?;
                                Ok(ClaimedSystemDelivery {
                                    id: row.get(0)?,
                                    delivery_key: row.get(1)?,
                                    event_kind: AlertEventKind::parse(&event_kind)?,
                                    target: AlertTarget::parse(&target)
                                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                                })
                            },
                        )?
                        .collect::<rusqlite::Result<Vec<_>>>()?
                };
                let mut claimed = Vec::with_capacity(deliveries.len());
                for delivery in deliveries {
                    let changed = transaction.execute(
                        "UPDATE alert_deliveries
                             SET state = 'leased', lease_owner = ?1,
                                 lease_until_ms = ?2, updated_at_ms = ?3
                             WHERE id = ?4 AND (
                               state = 'pending'
                               OR (state = 'retry_wait' AND next_attempt_at_ms <= ?3)
                               OR (state = 'leased' AND lease_until_ms <= ?3)
                             )",
                        rusqlite::params![worker_id, lease_until_ms, now_unix_ms, delivery.id],
                    )?;
                    if changed == 1 {
                        claimed.push(delivery);
                    }
                }
                transaction.commit()?;
                Ok::<_, rusqlite::Error>(claimed)
            })
            .await
            .map_err(SettingsStoreError::database)
    }

    pub(crate) async fn complete_system_delivery(
        &self,
        delivery_id: i64,
        worker_id: &str,
        attempted_at_ms: i64,
        duration_ms: i64,
    ) -> Result<(), SettingsStoreError> {
        self.finish_system_delivery(
            delivery_id,
            worker_id,
            attempted_at_ms,
            duration_ms,
            true,
            false,
        )
        .await
    }

    pub(crate) async fn fail_system_delivery(
        &self,
        delivery_id: i64,
        worker_id: &str,
        attempted_at_ms: i64,
        duration_ms: i64,
        transient: bool,
    ) -> Result<(), SettingsStoreError> {
        self.finish_system_delivery(
            delivery_id,
            worker_id,
            attempted_at_ms,
            duration_ms,
            false,
            transient,
        )
        .await
    }

    async fn finish_system_delivery(
        &self,
        delivery_id: i64,
        worker_id: &str,
        attempted_at_ms: i64,
        duration_ms: i64,
        delivered: bool,
        transient: bool,
    ) -> Result<(), SettingsStoreError> {
        let worker_id = worker_id.to_owned();
        self.connection
            .call(move |database| {
                let transaction =
                    database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let attempt_count: i64 = transaction.query_row(
                    "SELECT attempt_count FROM alert_deliveries
                     WHERE id = ?1 AND state = 'leased' AND lease_owner = ?2",
                    rusqlite::params![delivery_id, worker_id],
                    |row| row.get(0),
                )?;
                let next_count = attempt_count.saturating_add(1);
                let (state, outcome, public_error, next_attempt_at) = if delivered {
                    ("delivered", "delivered", None, None)
                } else if transient {
                    let exponent = u32::try_from(attempt_count.min(6)).unwrap_or(6);
                    let backoff = 60_000_i64.saturating_mul(2_i64.saturating_pow(exponent));
                    (
                        "retry_wait",
                        "transient_failure",
                        Some("notification_send_failed"),
                        Some(attempted_at_ms.saturating_add(backoff.min(3_600_000))),
                    )
                } else {
                    (
                        "permanent_failure",
                        "permanent_failure",
                        Some("notification_send_failed"),
                        None,
                    )
                };
                transaction.execute(
                    "INSERT INTO delivery_attempts
                     (delivery_id, attempted_at_ms, outcome, public_error_code, duration_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![
                        delivery_id,
                        attempted_at_ms,
                        outcome,
                        public_error,
                        duration_ms
                    ],
                )?;
                let changed = transaction.execute(
                    "UPDATE alert_deliveries
                     SET state = ?1, attempt_count = ?2, next_attempt_at_ms = ?3,
                         lease_owner = NULL, lease_until_ms = NULL,
                         public_error_code = ?4, updated_at_ms = ?5
                     WHERE id = ?6 AND state = 'leased' AND lease_owner = ?7",
                    rusqlite::params![
                        state,
                        next_count,
                        next_attempt_at,
                        public_error,
                        attempted_at_ms,
                        delivery_id,
                        worker_id
                    ],
                )?;
                if changed != 1 {
                    return Err(rusqlite::Error::InvalidQuery);
                }
                transaction.commit()
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

fn apply_usage_success(
    transaction: &rusqlite::Transaction<'_>,
    binding: &RefreshAccountBinding,
    observation: &WeeklyUsageObservation,
) -> rusqlite::Result<(UsageCommitDisposition, i64, bool)> {
    let Some(context) = validated_binding_context(transaction, binding)? else {
        return Ok((UsageCommitDisposition::Superseded, 0, false));
    };
    let account_id = binding
        .canonical_account_id
        .as_deref()
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let now = observation.captured_at_unix_ms;
    let (stream_id, _) = upsert_account_stream(transaction, &context.salt, account_id, now)?;
    let previous_state = load_ledger_state(transaction, stream_id, context.policy_timezone)?;
    let previous_epoch = QuotaLedger::persisted_epoch(&previous_state);
    let transition = QuotaLedger::apply(previous_state, observation, context.policy_timezone)
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    if transition.kind == LedgerApplyKind::DroppedOutOfOrder {
        return Ok((UsageCommitDisposition::Committed, 0, false));
    }
    persist_success_observation(transaction, stream_id, observation, &transition)?;
    persist_usage_transition_alerts(
        transaction,
        stream_id,
        previous_epoch.as_ref(),
        &transition,
        observation.captured_at_unix_ms,
    )?;
    persist_daily_policy_snapshots(transaction, stream_id, observation.captured_at_unix_ms)?;
    transaction.execute(
        "INSERT INTO usage_source_health
         (account_stream_id, last_attempt_at_ms, last_success_at_ms,
          consecutive_failures, public_error)
         VALUES (?1, ?2, ?2, 0, NULL)
         ON CONFLICT(account_stream_id) DO UPDATE SET
           last_attempt_at_ms = excluded.last_attempt_at_ms,
           last_success_at_ms = excluded.last_success_at_ms,
           consecutive_failures = 0,
           public_error = NULL",
        rusqlite::params![stream_id, now],
    )?;
    transaction.execute(
        "UPDATE app_settings
         SET configured_account_stream_id = ?1,
             active_account_stream_id = ?1, updated_at_ms = ?2
         WHERE singleton_id = 1",
        rusqlite::params![stream_id, now],
    )?;
    Ok((
        UsageCommitDisposition::Committed,
        i64::from(context.configured_stream_id != Some(stream_id)),
        true,
    ))
}

fn apply_usage_failure(
    transaction: &rusqlite::Transaction<'_>,
    binding: &RefreshAccountBinding,
    attempted_at_unix_ms: i64,
    public_error: UsageSourceErrorCode,
) -> rusqlite::Result<(UsageCommitDisposition, i64)> {
    let (salt, revision, configured_path, configured_stream_id): (
        Vec<u8>,
        i64,
        Option<String>,
        Option<i64>,
    ) = transaction.query_row(
        "SELECT m.local_hash_salt, m.settings_revision, s.auth_path,
                s.configured_account_stream_id
         FROM app_meta m
         JOIN app_settings s ON s.singleton_id = m.singleton_id
         WHERE m.singleton_id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    if revision != i64::from(binding.settings_revision)
        || configured_path.as_deref() != binding.canonical_path.to_str()
    {
        return Ok((UsageCommitDisposition::Superseded, 0));
    }
    let stream_id = if let Some(account_id) = binding.canonical_account_id.as_deref() {
        upsert_account_stream(transaction, &salt, account_id, attempted_at_unix_ms)?.0
    } else {
        configured_stream_id.ok_or(rusqlite::Error::QueryReturnedNoRows)?
    };
    let previous_health: Option<(Option<i64>, i64)> = {
        use rusqlite::OptionalExtension as _;
        transaction
            .query_row(
                "SELECT last_success_at_ms, consecutive_failures
                 FROM usage_source_health WHERE account_stream_id = ?1",
                [stream_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
    };
    transaction.execute(
        "INSERT INTO usage_source_health
         (account_stream_id, last_attempt_at_ms, last_success_at_ms,
          consecutive_failures, public_error)
         VALUES (?1, ?2, NULL, 1, ?3)
         ON CONFLICT(account_stream_id) DO UPDATE SET
           last_attempt_at_ms = excluded.last_attempt_at_ms,
           consecutive_failures = consecutive_failures + 1,
           public_error = excluded.public_error",
        rusqlite::params![
            stream_id,
            attempted_at_unix_ms,
            public_error.as_storage_key()
        ],
    )?;
    if previous_health.as_ref().is_some_and(|health| health.1 == 2) {
        persist_source_failure_alert(
            transaction,
            "codex",
            Some(stream_id),
            previous_health.and_then(|health| health.0),
            attempted_at_unix_ms,
        )?;
    }
    if configured_stream_id != Some(stream_id) {
        transaction.execute(
            "UPDATE app_settings
             SET configured_account_stream_id = ?1,
                 active_account_stream_id = NULL,
                 updated_at_ms = ?2
             WHERE singleton_id = 1",
            rusqlite::params![stream_id, attempted_at_unix_ms],
        )?;
    }
    Ok((
        UsageCommitDisposition::Committed,
        i64::from(configured_stream_id != Some(stream_id)),
    ))
}

fn apply_radar_success(
    transaction: &rusqlite::Transaction<'_>,
    attempted_at_unix_ms: i64,
    snapshot: &RadarSnapshot,
) -> rusqlite::Result<RadarCommitDisposition> {
    let previous_watch: Option<(String, i64, i64, i64)> = {
        use rusqlite::OptionalExtension as _;
        transaction
            .query_row(
                "SELECT o.source_id, o.observed_at_ms, o.expires_at_ms,
                        o.chance_basis_points
                 FROM radar_source_health h
                 JOIN radar_observations o ON o.id = h.current_observation_id
                 WHERE h.singleton_id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?
    };
    let current_observation_id =
        upsert_radar_observation(transaction, snapshot.observation(), attempted_at_unix_ms)?;
    if let Some(observation) = snapshot.observation() {
        persist_radar_threshold_alert(
            transaction,
            previous_watch.as_ref(),
            observation,
            attempted_at_unix_ms,
        )?;
    }
    let new_announcement = if let Some(announcement) = snapshot.latest_announcement() {
        transaction.execute(
            "INSERT INTO radar_announcements
                 (source_id, announced_at_ms, text, source_url, captured_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(source_id) DO NOTHING",
            rusqlite::params![
                announcement.source_id(),
                announcement.announced_at_unix_ms(),
                announcement.text(),
                announcement.source_url(),
                attempted_at_unix_ms,
            ],
        )? == 1
    } else {
        false
    };
    transaction.execute(
        "INSERT INTO radar_source_health
         (singleton_id, last_attempt_at_ms, last_success_at_ms,
          consecutive_failures, public_error, current_observation_id,
          account_recheck_requested_at_ms)
         VALUES (1, ?1, ?1, 0, NULL, ?2, NULL)
         ON CONFLICT(singleton_id) DO UPDATE SET
           last_attempt_at_ms = excluded.last_attempt_at_ms,
           last_success_at_ms = excluded.last_success_at_ms,
           consecutive_failures = 0,
           public_error = NULL,
           current_observation_id = excluded.current_observation_id",
        rusqlite::params![attempted_at_unix_ms, current_observation_id],
    )?;
    Ok(RadarCommitDisposition { new_announcement })
}

fn upsert_radar_observation(
    transaction: &rusqlite::Transaction<'_>,
    observation: Option<&crate::RadarObservation>,
    attempted_at_unix_ms: i64,
) -> rusqlite::Result<Option<i64>> {
    let Some(observation) = observation else {
        return Ok(None);
    };
    transaction.execute(
        "INSERT INTO radar_observations
         (source_id, observed_at_ms, expires_at_ms, chance_basis_points,
          explanation, source_url, captured_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(source_id, observed_at_ms, expires_at_ms,
                     chance_basis_points, explanation, source_url) DO NOTHING",
        rusqlite::params![
            observation.source_id(),
            observation.observed_at_unix_ms(),
            observation.expires_at_unix_ms(),
            observation.chance().basis_points(),
            observation.explanation(),
            observation.source_url(),
            attempted_at_unix_ms,
        ],
    )?;
    transaction
        .query_row(
            "SELECT id FROM radar_observations
             WHERE source_id = ?1 AND observed_at_ms = ?2 AND expires_at_ms = ?3
               AND chance_basis_points = ?4 AND explanation = ?5 AND source_url = ?6",
            rusqlite::params![
                observation.source_id(),
                observation.observed_at_unix_ms(),
                observation.expires_at_unix_ms(),
                observation.chance().basis_points(),
                observation.explanation(),
                observation.source_url(),
            ],
            |row| row.get::<_, i64>(0),
        )
        .map(Some)
}

fn persist_radar_threshold_alert(
    transaction: &rusqlite::Transaction<'_>,
    previous_watch: Option<&(String, i64, i64, i64)>,
    observation: &crate::RadarObservation,
    attempted_at_unix_ms: i64,
) -> rusqlite::Result<()> {
    let watch_key = format!(
        "{}:{}:{}",
        observation.source_id(),
        observation.observed_at_unix_ms(),
        observation.expires_at_unix_ms()
    );
    let previous_above_same_watch =
        previous_watch.is_some_and(|(source_id, observed_at, expires_at, chance)| {
            source_id == observation.source_id()
                && *observed_at == observation.observed_at_unix_ms()
                && *expires_at == observation.expires_at_unix_ms()
                && *chance >= 7_000
        });
    if observation.chance().basis_points() < 7_000 || previous_above_same_watch {
        return Ok(());
    }
    let policy_timezone: String = transaction.query_row(
        "SELECT policy_timezone FROM app_settings WHERE singleton_id = 1",
        [],
        |row| row.get(0),
    )?;
    persist_alert_event(
        transaction,
        &NewAlertEvent {
            event_key: format!(
                "radar:{watch_key}:{}",
                AlertEventKind::RadarChance70.as_str()
            ),
            event_kind: AlertEventKind::RadarChance70,
            account_stream_id: None,
            quota_epoch_id: None,
            local_date: None,
            watch_key: Some(watch_key),
            source: None,
            threshold_micropoints: Some(7_000),
            message_key: "alerts.radar_chance_70",
            structured_args_json: serde_json::json!({
                "chanceBasisPoints": observation.chance().basis_points(),
            })
            .to_string(),
            policy_timezone,
            target: "radar",
            created_at_ms: attempted_at_unix_ms,
        },
    )?;
    Ok(())
}

fn apply_radar_failure(
    transaction: &rusqlite::Transaction<'_>,
    attempted_at_unix_ms: i64,
    public_error: RadarSourceErrorCode,
) -> rusqlite::Result<()> {
    let previous_health: Option<(Option<i64>, i64)> = {
        use rusqlite::OptionalExtension as _;
        transaction
            .query_row(
                "SELECT last_success_at_ms, consecutive_failures
                 FROM radar_source_health WHERE singleton_id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
    };
    transaction.execute(
        "INSERT INTO radar_source_health
         (singleton_id, last_attempt_at_ms, last_success_at_ms,
          consecutive_failures, public_error, current_observation_id,
          account_recheck_requested_at_ms)
         VALUES (1, ?1, NULL, 1, ?2, NULL, NULL)
         ON CONFLICT(singleton_id) DO UPDATE SET
           last_attempt_at_ms = excluded.last_attempt_at_ms,
           consecutive_failures = consecutive_failures + 1,
           public_error = excluded.public_error",
        rusqlite::params![attempted_at_unix_ms, public_error.as_storage_key()],
    )?;
    if previous_health.as_ref().is_some_and(|health| health.1 == 2) {
        persist_source_failure_alert(
            transaction,
            "radar",
            None,
            previous_health.and_then(|health| health.0),
            attempted_at_unix_ms,
        )?;
    }
    Ok(())
}

fn update_dashboard_meta(
    transaction: &rusqlite::Transaction<'_>,
    settings_increment: i64,
    updated_at_unix_ms: i64,
) -> rusqlite::Result<()> {
    transaction.execute(
        "UPDATE app_meta
         SET settings_revision = settings_revision + ?1,
             dashboard_revision = dashboard_revision + 1,
             updated_at_ms = ?2
         WHERE singleton_id = 1",
        rusqlite::params![settings_increment, updated_at_unix_ms],
    )?;
    Ok(())
}

fn record_usage_success_transaction(
    database: &mut rusqlite::Connection,
    binding: &RefreshAccountBinding,
    observation: &WeeklyUsageObservation,
) -> rusqlite::Result<UsageCommitDisposition> {
    let transaction =
        database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let Some(context) = validated_binding_context(&transaction, binding)? else {
        return Ok(UsageCommitDisposition::Superseded);
    };
    let account_id = binding
        .canonical_account_id
        .as_deref()
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let now = observation.captured_at_unix_ms;
    let (stream_id, _) = upsert_account_stream(&transaction, &context.salt, account_id, now)?;
    let previous_state = load_ledger_state(&transaction, stream_id, context.policy_timezone)?;
    let previous_epoch = QuotaLedger::persisted_epoch(&previous_state);
    let transition = QuotaLedger::apply(previous_state, observation, context.policy_timezone)
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    if transition.kind == LedgerApplyKind::DroppedOutOfOrder {
        transaction.commit()?;
        return Ok(UsageCommitDisposition::Committed);
    }
    persist_success_observation(&transaction, stream_id, observation, &transition)?;
    persist_usage_transition_alerts(
        &transaction,
        stream_id,
        previous_epoch.as_ref(),
        &transition,
        observation.captured_at_unix_ms,
    )?;
    persist_daily_policy_snapshots(&transaction, stream_id, observation.captured_at_unix_ms)?;
    persist_success_projection(
        &transaction,
        stream_id,
        context.configured_stream_id,
        observation.captured_at_unix_ms,
    )?;
    transaction
        .commit()
        .map(|()| UsageCommitDisposition::Committed)
}

fn query_public_reset_radar(
    database: &rusqlite::Connection,
    now_unix_ms: i64,
) -> rusqlite::Result<PublicResetRadar> {
    let health = query_radar_health(database)?;
    let latest_announcement = query_latest_radar_announcement(database)?;
    let Some(health) = health else {
        return Ok(PublicResetRadar {
            last_attempt_at_unix_ms: None,
            last_success_at_unix_ms: None,
            consecutive_failures: 0,
            source_status: SourceStatus::Unavailable,
            public_error: None,
            prediction: None,
            latest_announcement,
        });
    };
    let public_error = match health.error_key.as_deref() {
        Some(key) => {
            Some(RadarSourceErrorCode::from_storage_key(key).ok_or(rusqlite::Error::InvalidQuery)?)
        }
        None => None,
    };
    let source_status =
        project_radar_source_status(public_error, health.last_success_at_unix_ms, now_unix_ms);
    let prediction = query_radar_prediction(database, health.current_observation_id, now_unix_ms)?;
    let consecutive_failures =
        u32::try_from(health.consecutive_failures).map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(PublicResetRadar {
        last_attempt_at_unix_ms: Some(health.last_attempt_at_unix_ms),
        last_success_at_unix_ms: health.last_success_at_unix_ms,
        consecutive_failures,
        source_status,
        public_error,
        prediction,
        latest_announcement,
    })
}

struct RadarHealthRow {
    last_attempt_at_unix_ms: i64,
    last_success_at_unix_ms: Option<i64>,
    consecutive_failures: i64,
    error_key: Option<String>,
    current_observation_id: Option<i64>,
}

fn query_radar_health(database: &rusqlite::Connection) -> rusqlite::Result<Option<RadarHealthRow>> {
    use rusqlite::OptionalExtension as _;

    database
        .query_row(
            "SELECT last_attempt_at_ms, last_success_at_ms,
                    consecutive_failures, public_error, current_observation_id
             FROM radar_source_health WHERE singleton_id = 1",
            [],
            |row| {
                Ok(RadarHealthRow {
                    last_attempt_at_unix_ms: row.get(0)?,
                    last_success_at_unix_ms: row.get(1)?,
                    consecutive_failures: row.get(2)?,
                    error_key: row.get(3)?,
                    current_observation_id: row.get(4)?,
                })
            },
        )
        .optional()
}

fn query_latest_radar_announcement(
    database: &rusqlite::Connection,
) -> rusqlite::Result<Option<PublicRadarAnnouncement>> {
    use rusqlite::OptionalExtension as _;

    database
        .query_row(
            "SELECT announced_at_ms, text, source_url
             FROM radar_announcements
             ORDER BY announced_at_ms DESC, id DESC LIMIT 1",
            [],
            |row| {
                Ok(PublicRadarAnnouncement {
                    announced_at_unix_ms: row.get(0)?,
                    text: row.get(1)?,
                    source_url: row.get(2)?,
                })
            },
        )
        .optional()
}

fn project_radar_source_status(
    public_error: Option<RadarSourceErrorCode>,
    last_success_at_unix_ms: Option<i64>,
    now_unix_ms: i64,
) -> SourceStatus {
    if public_error.is_some() {
        if last_success_at_unix_ms.is_some() {
            SourceStatus::StaleAfterFailure
        } else {
            SourceStatus::Unavailable
        }
    } else if last_success_at_unix_ms
        .is_some_and(|success| now_unix_ms.saturating_sub(success) > FRESH_FOR_MS)
    {
        SourceStatus::StaleByAge
    } else if last_success_at_unix_ms.is_some() {
        SourceStatus::Fresh
    } else {
        SourceStatus::Unavailable
    }
}

fn query_radar_prediction(
    database: &rusqlite::Connection,
    current_observation_id: Option<i64>,
    now_unix_ms: i64,
) -> rusqlite::Result<Option<PublicRadarPrediction>> {
    let prediction = current_observation_id
        .map(|observation_id| {
            database.query_row(
                "SELECT chance_basis_points, observed_at_ms, expires_at_ms,
                        explanation, source_url
                 FROM radar_observations WHERE id = ?1",
                [observation_id],
                |row| {
                    let chance: i64 = row.get(0)?;
                    let chance = u16::try_from(chance)
                        .ok()
                        .and_then(RadarChance::from_basis_points)
                        .ok_or(rusqlite::Error::InvalidQuery)?;
                    let observed_at_unix_ms: i64 = row.get(1)?;
                    let expires_at_unix_ms: i64 = row.get(2)?;
                    Ok(PublicRadarPrediction {
                        chance_basis_points: chance.basis_points(),
                        display_chance: radar_bucket_label(chance).to_owned(),
                        observed_at_unix_ms,
                        expires_at_unix_ms,
                        explanation: row.get(3)?,
                        source_url: row.get(4)?,
                    })
                },
            )
        })
        .transpose()?
        .filter(|prediction| {
            prediction.observed_at_unix_ms <= now_unix_ms
                && now_unix_ms < prediction.expires_at_unix_ms
        });
    Ok(prediction)
}

struct NewAlertEvent {
    event_key: String,
    event_kind: AlertEventKind,
    account_stream_id: Option<i64>,
    quota_epoch_id: Option<i64>,
    local_date: Option<String>,
    watch_key: Option<String>,
    source: Option<&'static str>,
    threshold_micropoints: Option<i64>,
    message_key: &'static str,
    structured_args_json: String,
    policy_timezone: String,
    target: &'static str,
    created_at_ms: i64,
}

fn persist_alert_event(
    transaction: &rusqlite::Transaction<'_>,
    event: &NewAlertEvent,
) -> rusqlite::Result<bool> {
    let inserted = transaction.execute(
        "INSERT INTO alert_events
         (event_key, event_kind, account_stream_id, quota_epoch_id,
          local_date, watch_key, source, threshold_micropoints,
          message_key, structured_args_json, interface_locale_snapshot,
          format_locale_snapshot, policy_timezone_snapshot, target, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                 'zh-CN', 'zh-CN', ?11, ?12, ?13)
         ON CONFLICT(event_key) DO NOTHING",
        rusqlite::params![
            event.event_key,
            event.event_kind.as_str(),
            event.account_stream_id,
            event.quota_epoch_id,
            event.local_date,
            event.watch_key,
            event.source,
            event.threshold_micropoints,
            event.message_key,
            event.structured_args_json,
            event.policy_timezone,
            event.target,
            event.created_at_ms,
        ],
    )?;
    if inserted == 0 {
        return Ok(false);
    }
    let alert_event_id = transaction.last_insert_rowid();
    let preferences = {
        let mut statement = transaction.prepare(
            "SELECT channel FROM alert_preferences
             WHERE event_kind = ?1 AND enabled = 1 ORDER BY channel",
        )?;
        statement
            .query_map([event.event_kind.as_str()], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    for channel in preferences {
        let state = if channel == AlertChannel::System.as_str() {
            "pending"
        } else {
            "paused_config"
        };
        transaction.execute(
            "INSERT INTO alert_deliveries
             (delivery_key, alert_event_id, channel, recipient_key, state,
              attempt_count, next_attempt_at_ms, lease_owner, lease_until_ms,
              public_error_code, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, NULL, ?4, 0, ?5, NULL, NULL, NULL, ?5, ?5)",
            rusqlite::params![
                format!("{}:{channel}", event.event_key),
                alert_event_id,
                channel,
                state,
                event.created_at_ms,
            ],
        )?;
    }
    Ok(true)
}

fn persist_source_failure_alert(
    transaction: &rusqlite::Transaction<'_>,
    source: &'static str,
    account_stream_id: Option<i64>,
    last_success_at_ms: Option<i64>,
    created_at_ms: i64,
) -> rusqlite::Result<()> {
    let policy_timezone: String = transaction.query_row(
        "SELECT policy_timezone FROM app_settings WHERE singleton_id = 1",
        [],
        |row| row.get(0),
    )?;
    let generation = last_success_at_ms.unwrap_or(0);
    persist_alert_event(
        transaction,
        &NewAlertEvent {
            event_key: format!(
                "source:{source}:stream:{}:generation:{generation}:{}",
                account_stream_id.unwrap_or(0),
                AlertEventKind::SourceFailures3.as_str()
            ),
            event_kind: AlertEventKind::SourceFailures3,
            account_stream_id,
            quota_epoch_id: None,
            local_date: None,
            watch_key: None,
            source: Some(source),
            threshold_micropoints: Some(3),
            message_key: "alerts.source_failures_3",
            structured_args_json: serde_json::json!({
                "source": source,
                "consecutiveFailures": 3,
            })
            .to_string(),
            policy_timezone,
            target: "source",
            created_at_ms,
        },
    )?;
    Ok(())
}

fn persist_usage_transition_alerts(
    transaction: &rusqlite::Transaction<'_>,
    stream_id: i64,
    previous_epoch: Option<&PersistedLedgerEpoch>,
    transition: &crate::LedgerTransition,
    created_at_ms: i64,
) -> rusqlite::Result<()> {
    let current_epoch =
        QuotaLedger::persisted_epoch(&transition.state).ok_or(rusqlite::Error::InvalidQuery)?;
    let quota_epoch_id: i64 = transaction.query_row(
        "SELECT id FROM quota_epochs
         WHERE account_stream_id = ?1 AND closed_at_ms IS NULL",
        [stream_id],
        |row| row.get(0),
    )?;
    let policy_timezone: String = transaction.query_row(
        "SELECT policy_timezone FROM app_settings WHERE singleton_id = 1",
        [],
        |row| row.get(0),
    )?;

    if let Some(previous) =
        previous_epoch.filter(|previous| previous.sequence == current_epoch.sequence)
    {
        for (used_boundary, remaining_threshold, event_kind, message_key) in [
            (
                80_000_000,
                20_000_000,
                AlertEventKind::WeeklyRemaining20,
                "alerts.weekly_remaining_20",
            ),
            (
                90_000_000,
                10_000_000,
                AlertEventKind::WeeklyRemaining10,
                "alerts.weekly_remaining_10",
            ),
        ] {
            if previous.high_water_micropoints < used_boundary
                && current_epoch.high_water_micropoints >= used_boundary
            {
                persist_alert_event(
                    transaction,
                    &NewAlertEvent {
                        event_key: format!(
                            "stream:{stream_id}:epoch:{quota_epoch_id}:{}",
                            event_kind.as_str()
                        ),
                        event_kind,
                        account_stream_id: Some(stream_id),
                        quota_epoch_id: Some(quota_epoch_id),
                        local_date: None,
                        watch_key: None,
                        source: None,
                        threshold_micropoints: Some(remaining_threshold),
                        message_key,
                        structured_args_json: serde_json::json!({
                            "remainingMicropoints": remaining_threshold,
                        })
                        .to_string(),
                        policy_timezone: policy_timezone.clone(),
                        target: "today",
                        created_at_ms,
                    },
                )?;
            }
        }
    }

    if transition.kind == LedgerApplyKind::ConfirmedReset {
        persist_alert_event(
            transaction,
            &NewAlertEvent {
                event_key: format!(
                    "stream:{stream_id}:epoch:{quota_epoch_id}:{}",
                    AlertEventKind::QuotaResetConfirmed.as_str()
                ),
                event_kind: AlertEventKind::QuotaResetConfirmed,
                account_stream_id: Some(stream_id),
                quota_epoch_id: Some(quota_epoch_id),
                local_date: None,
                watch_key: None,
                source: None,
                threshold_micropoints: None,
                message_key: "alerts.quota_reset_confirmed",
                structured_args_json: serde_json::json!({
                    "epochSequence": current_epoch.sequence,
                })
                .to_string(),
                policy_timezone,
                target: "today",
                created_at_ms,
            },
        )?;
    }
    Ok(())
}

fn persist_daily_policy_snapshots(
    transaction: &rusqlite::Transaction<'_>,
    stream_id: i64,
    now_unix_ms: i64,
) -> rusqlite::Result<()> {
    let timezone: String = transaction.query_row(
        "SELECT policy_timezone FROM app_settings WHERE singleton_id = 1",
        [],
        |row| row.get(0),
    )?;
    let timezone = parse_policy_timezone(&timezone)?;
    for day in project_policy_days(transaction, stream_id, timezone, now_unix_ms)? {
        let status = match day.status {
            DailyPolicyStatus::Unknown => "unknown",
            DailyPolicyStatus::Normal => "normal",
            DailyPolicyStatus::Warning => "warning",
            DailyPolicyStatus::Exceeded => "exceeded",
            DailyPolicyStatus::Finalized => "finalized",
        };
        let finalized_at = day.finalized.then_some(now_unix_ms);
        transaction.execute(
            "INSERT INTO daily_ledgers
             (account_stream_id, local_date, policy_timezone,
              used_micropoints, policy_revision_id, base_micropoints,
              carry_micropoints, policy_status, finalized_at_ms, updated_at_ms)
             VALUES (?8, ?9, ?2, ?10, ?1, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(account_stream_id, local_date, policy_timezone)
             DO UPDATE SET
                 used_micropoints = COALESCE(
                   excluded.used_micropoints, daily_ledgers.used_micropoints
                 ),
                 policy_revision_id = excluded.policy_revision_id,
                 base_micropoints = excluded.base_micropoints,
                 carry_micropoints = excluded.carry_micropoints,
                 policy_status = excluded.policy_status,
                 finalized_at_ms = excluded.finalized_at_ms,
                 updated_at_ms = MAX(daily_ledgers.updated_at_ms, excluded.updated_at_ms)
             WHERE daily_ledgers.finalized_at_ms IS NULL",
            rusqlite::params![
                i64::try_from(day.policy_revision_id).map_err(|_| rusqlite::Error::InvalidQuery)?,
                day.policy_timezone.name(),
                day.base_micropoints,
                day.carry_micropoints,
                status,
                finalized_at,
                now_unix_ms,
                stream_id,
                day.local_date.to_string(),
                day.used_micropoints,
            ],
        )?;
        if let Some(transition) = day.threshold_transition {
            persist_daily_threshold_alert(transaction, stream_id, now_unix_ms, &day, transition)?;
        }
    }
    Ok(())
}

fn persist_daily_threshold_alert(
    transaction: &rusqlite::Transaction<'_>,
    stream_id: i64,
    now_unix_ms: i64,
    day: &PolicyDayProjection,
    transition: ThresholdTransition,
) -> rusqlite::Result<()> {
    transaction.execute(
        "INSERT OR IGNORE INTO daily_threshold_transitions
         (account_stream_id, local_date, policy_revision_id,
          transition_kind, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            stream_id,
            day.local_date.to_string(),
            i64::try_from(day.policy_revision_id).map_err(|_| rusqlite::Error::InvalidQuery)?,
            threshold_transition_key(transition),
            now_unix_ms,
        ],
    )?;
    let quota_epoch_id: i64 = transaction.query_row(
        "SELECT id FROM quota_epochs
         WHERE account_stream_id = ?1 AND closed_at_ms IS NULL",
        [stream_id],
        |row| row.get(0),
    )?;
    let (event_kind, threshold_micropoints, message_key) = match transition {
        ThresholdTransition::Warning => (
            AlertEventKind::Daily80,
            day.limit_micropoints.saturating_mul(4) / 5,
            "alerts.daily_80",
        ),
        ThresholdTransition::Exceeded => (
            AlertEventKind::Daily100,
            day.limit_micropoints,
            "alerts.daily_100",
        ),
    };
    let local_date = day.local_date.to_string();
    persist_alert_event(
        transaction,
        &NewAlertEvent {
            event_key: format!(
                "stream:{stream_id}:epoch:{quota_epoch_id}:date:{local_date}:{}",
                event_kind.as_str()
            ),
            event_kind,
            account_stream_id: Some(stream_id),
            quota_epoch_id: Some(quota_epoch_id),
            local_date: Some(local_date.clone()),
            watch_key: None,
            source: None,
            threshold_micropoints: Some(threshold_micropoints),
            message_key,
            structured_args_json: serde_json::json!({
                "localDate": local_date,
                "thresholdMicropoints": threshold_micropoints,
            })
            .to_string(),
            policy_timezone: day.policy_timezone.name().to_owned(),
            target: "today",
            created_at_ms: now_unix_ms,
        },
    )?;
    Ok(())
}

struct BindingContext {
    salt: Vec<u8>,
    configured_stream_id: Option<i64>,
    policy_timezone: chrono_tz::Tz,
}

fn validated_binding_context(
    transaction: &rusqlite::Transaction<'_>,
    binding: &RefreshAccountBinding,
) -> rusqlite::Result<Option<BindingContext>> {
    let (salt, revision, configured_path, configured_stream_id, policy_timezone): (
        Vec<u8>,
        i64,
        Option<String>,
        Option<i64>,
        String,
    ) = transaction.query_row(
        "SELECT m.local_hash_salt, m.settings_revision, s.auth_path,
                s.configured_account_stream_id, s.policy_timezone
         FROM app_meta m
         JOIN app_settings s ON s.singleton_id = m.singleton_id
         WHERE m.singleton_id = 1",
        [],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        },
    )?;
    if revision != i64::from(binding.settings_revision)
        || configured_path.as_deref() != binding.canonical_path.to_str()
    {
        return Ok(None);
    }
    Ok(Some(BindingContext {
        salt,
        configured_stream_id,
        policy_timezone: parse_policy_timezone(&policy_timezone)?,
    }))
}

fn persist_success_observation(
    transaction: &rusqlite::Transaction<'_>,
    stream_id: i64,
    observation: &WeeklyUsageObservation,
    transition: &crate::LedgerTransition,
) -> rusqlite::Result<()> {
    let persisted_epoch =
        QuotaLedger::persisted_epoch(&transition.state).ok_or(rusqlite::Error::InvalidQuery)?;
    let quota_epoch_id = persist_ledger_epoch(
        transaction,
        stream_id,
        &persisted_epoch,
        transition.kind,
        observation.captured_at_unix_ms,
    )?;
    transaction.execute(
        "INSERT INTO usage_observations
         (account_stream_id, quota_epoch_id, ledger_eligible, captured_at_ms,
          used_micropoints, window_seconds, resets_at_s, plan_type, allowed)
         VALUES (?1, ?8, 1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            stream_id,
            observation.captured_at_unix_ms,
            observation.used.micropoints(),
            observation.window_seconds,
            observation.resets_at_unix_s,
            observation.plan_type,
            observation.allowed.map(i64::from),
            quota_epoch_id,
        ],
    )?;
    Ok(())
}

fn persist_success_projection(
    transaction: &rusqlite::Transaction<'_>,
    stream_id: i64,
    configured_stream_id: Option<i64>,
    observed_at_unix_ms: i64,
) -> rusqlite::Result<()> {
    transaction.execute(
        "INSERT INTO usage_source_health
         (account_stream_id, last_attempt_at_ms, last_success_at_ms,
          consecutive_failures, public_error)
         VALUES (?1, ?2, ?2, 0, NULL)
         ON CONFLICT(account_stream_id) DO UPDATE SET
           last_attempt_at_ms = excluded.last_attempt_at_ms,
           last_success_at_ms = excluded.last_success_at_ms,
           consecutive_failures = 0,
           public_error = NULL",
        rusqlite::params![stream_id, observed_at_unix_ms],
    )?;
    transaction.execute(
        "UPDATE app_settings
         SET configured_account_stream_id = ?1,
             active_account_stream_id = ?1, updated_at_ms = ?2
         WHERE singleton_id = 1",
        rusqlite::params![stream_id, observed_at_unix_ms],
    )?;
    let settings_increment = i64::from(configured_stream_id != Some(stream_id));
    transaction.execute(
        "UPDATE app_meta
         SET settings_revision = settings_revision + ?1,
             dashboard_revision = dashboard_revision + 1,
             updated_at_ms = ?2
         WHERE singleton_id = 1",
        rusqlite::params![settings_increment, observed_at_unix_ms],
    )?;
    Ok(())
}

struct LiveQuotaRow {
    stream_id: i64,
    policy_timezone: chrono_tz::Tz,
    used_micropoints: Option<i64>,
    captured_at_unix_ms: Option<i64>,
    resets_at_unix_s: Option<i64>,
    plan_type: Option<String>,
    allowed: Option<i64>,
    last_attempt_at_unix_ms: i64,
    last_success_at_unix_ms: Option<i64>,
    consecutive_failures: i64,
    error_key: Option<String>,
}

fn query_live_quota_row(
    transaction: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<Option<LiveQuotaRow>> {
    use rusqlite::OptionalExtension as _;

    transaction
        .query_row(
            "SELECT s.configured_account_stream_id, s.policy_timezone,
                    o.used_micropoints, o.captured_at_ms,
                    COALESCE(e.scheduled_reset_at_s, o.resets_at_s),
                    o.plan_type, o.allowed, h.last_attempt_at_ms,
                    h.last_success_at_ms, h.consecutive_failures, h.public_error
             FROM app_settings s
             JOIN usage_source_health h
               ON h.account_stream_id = s.configured_account_stream_id
             LEFT JOIN usage_observations o
               ON o.id = (
                 SELECT latest.id FROM usage_observations latest
                 WHERE latest.account_stream_id = s.configured_account_stream_id
                   AND latest.ledger_eligible = 1
                 ORDER BY latest.captured_at_ms DESC LIMIT 1
               )
             LEFT JOIN quota_epochs e
               ON e.account_stream_id = s.configured_account_stream_id
              AND e.closed_at_ms IS NULL
             WHERE s.singleton_id = 1
             LIMIT 1",
            [],
            |row| {
                Ok(LiveQuotaRow {
                    stream_id: row.get(0)?,
                    policy_timezone: parse_policy_timezone(&row.get::<_, String>(1)?)?,
                    used_micropoints: row.get(2)?,
                    captured_at_unix_ms: row.get(3)?,
                    resets_at_unix_s: row.get(4)?,
                    plan_type: row.get(5)?,
                    allowed: row.get(6)?,
                    last_attempt_at_unix_ms: row.get(7)?,
                    last_success_at_unix_ms: row.get(8)?,
                    consecutive_failures: row.get(9)?,
                    error_key: row.get(10)?,
                })
            },
        )
        .optional()
}

fn project_policy_days(
    transaction: &rusqlite::Transaction<'_>,
    stream_id: i64,
    policy_timezone: chrono_tz::Tz,
    now_unix_ms: i64,
) -> rusqlite::Result<Vec<PolicyDayProjection>> {
    let Some(projection) = QuotaLedger::project(
        &load_ledger_state(transaction, stream_id, policy_timezone)?,
        policy_timezone,
    )
    .map_err(|_| rusqlite::Error::InvalidQuery)?
    else {
        return Ok(Vec::new());
    };
    let (policy_revision_id, policy) = load_active_quota_policy(transaction)?;
    let first = projection
        .days
        .first()
        .and_then(|day| chrono::NaiveDate::parse_from_str(&day.local_date, "%Y-%m-%d").ok())
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let last = projection
        .days
        .last()
        .and_then(|day| chrono::NaiveDate::parse_from_str(&day.local_date, "%Y-%m-%d").ok())
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let anchor = first
        .checked_sub_signed(chrono::Duration::days(i64::from(
            chrono::Datelike::weekday(&first).num_days_from_monday(),
        )))
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let current_dates = projection
        .days
        .iter()
        .map(|day| day.local_date.clone())
        .collect::<BTreeSet<_>>();
    let mut usage = projection
        .days
        .into_iter()
        .map(|day| (day.local_date, day.used_micropoints))
        .collect::<BTreeMap<_, _>>();
    let today = chrono::DateTime::from_timestamp_millis(now_unix_ms)
        .ok_or(rusqlite::Error::InvalidQuery)?
        .with_timezone(&policy.policy_timezone())
        .date_naive();
    let (mut snapshots, mut previous_statuses) = load_stored_policy_facts(
        transaction,
        stream_id,
        anchor,
        last,
        today,
        policy.policy_timezone(),
        &mut usage,
    )?;
    Ok(QuotaLedger::project_policy_days(
        &policy,
        &mut PolicyWindowFacts {
            anchor,
            last,
            today,
            policy_revision_id,
            usage: &usage,
            snapshots: &mut snapshots,
            previous_statuses: &mut previous_statuses,
        },
    )
    .map_err(|_| rusqlite::Error::InvalidQuery)?
    .into_iter()
    .filter(|day| current_dates.contains(&day.local_date.to_string()))
    .collect())
}

fn project_public_ledger_days(
    transaction: &rusqlite::Transaction<'_>,
    stream_id: i64,
    policy_timezone: chrono_tz::Tz,
    now_unix_ms: i64,
) -> rusqlite::Result<Vec<PublicLedgerDay>> {
    let today = chrono::DateTime::from_timestamp_millis(now_unix_ms)
        .ok_or(rusqlite::Error::InvalidQuery)?
        .with_timezone(&policy_timezone)
        .date_naive();
    project_policy_days(transaction, stream_id, policy_timezone, now_unix_ms)?
        .iter()
        .map(|day| public_ledger_day(day, today))
        .collect()
}

fn load_stored_policy_facts(
    transaction: &rusqlite::Transaction<'_>,
    stream_id: i64,
    anchor: chrono::NaiveDate,
    last: chrono::NaiveDate,
    today: chrono::NaiveDate,
    policy_timezone: chrono_tz::Tz,
    usage: &mut BTreeMap<String, Option<i64>>,
) -> rusqlite::Result<(
    BTreeMap<String, DailyLimitSnapshot>,
    BTreeMap<String, DailyPolicyStatus>,
)> {
    let mut snapshots = BTreeMap::new();
    let mut previous_statuses = BTreeMap::new();
    let mut statement = transaction.prepare(
        "SELECT local_date, used_micropoints, policy_revision_id,
                policy_timezone, base_micropoints, carry_micropoints,
                finalized_at_ms, policy_status
         FROM daily_ledgers
         WHERE account_stream_id = ?1
           AND local_date BETWEEN ?2 AND ?3
           AND (finalized_at_ms IS NOT NULL OR policy_timezone = ?4)",
    )?;
    let rows = statement
        .query_map(
            rusqlite::params![
                stream_id,
                anchor.to_string(),
                last.to_string(),
                policy_timezone.name()
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                    row.get::<_, String>(7)?,
                ))
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if rows
        .iter()
        .any(|row| row.6.is_some() && row.3 != policy_timezone.name())
    {
        usage.retain(|date, _| {
            chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok_and(|date| date >= today)
        });
    }
    for (date, used, revision, timezone, base, carry, finalized_at, previous_status) in rows {
        if finalized_at.is_some() {
            usage.insert(date.clone(), used);
        }
        previous_statuses.insert(date.clone(), parse_daily_policy_status(&previous_status)?);
        if finalized_at.is_some() {
            snapshots.insert(
                date,
                DailyLimitSnapshot::new(
                    u64::try_from(revision).map_err(|_| rusqlite::Error::InvalidQuery)?,
                    &timezone,
                    base,
                    carry,
                )
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            );
        }
    }
    Ok((snapshots, previous_statuses))
}

fn public_ledger_day(
    day: &PolicyDayProjection,
    today: chrono::NaiveDate,
) -> rusqlite::Result<PublicLedgerDay> {
    Ok(PublicLedgerDay {
        local_date: day.local_date.to_string(),
        used_micropoints: day.used_micropoints,
        policy_revision: day.policy_revision_id,
        policy_timezone: day.policy_timezone.name().to_owned(),
        base_micropoints: u32::try_from(day.base_micropoints)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        carry_micropoints: u32::try_from(day.carry_micropoints)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        limit_micropoints: u32::try_from(day.limit_micropoints)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        is_today: day.local_date == today,
        finalized: day.finalized,
        status: match day.status {
            DailyPolicyStatus::Unknown => LedgerDayStatus::Unknown,
            DailyPolicyStatus::Normal => LedgerDayStatus::Normal,
            DailyPolicyStatus::Warning => LedgerDayStatus::Warning,
            DailyPolicyStatus::Exceeded => LedgerDayStatus::Exceeded,
            DailyPolicyStatus::Finalized => LedgerDayStatus::Finalized,
        },
    })
}

fn parse_daily_policy_status(value: &str) -> rusqlite::Result<DailyPolicyStatus> {
    match value {
        "unknown" => Ok(DailyPolicyStatus::Unknown),
        "normal" => Ok(DailyPolicyStatus::Normal),
        "warning" => Ok(DailyPolicyStatus::Warning),
        "exceeded" => Ok(DailyPolicyStatus::Exceeded),
        "finalized" => Ok(DailyPolicyStatus::Finalized),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

const fn threshold_transition_key(transition: ThresholdTransition) -> &'static str {
    match transition {
        ThresholdTransition::Warning => "warning",
        ThresholdTransition::Exceeded => "exceeded",
    }
}

fn load_active_quota_policy(
    database: &rusqlite::Connection,
) -> rusqlite::Result<(u64, QuotaPolicy)> {
    let public = load_public_quota_policy(database)?;
    let bases = public
        .base_micropoints
        .into_iter()
        .map(i64::from)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let policy = QuotaLedger::validate_policy(
        bases,
        public.carry_workdays_enabled,
        &public.policy_timezone,
    )
    .map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok((public.policy_revision, policy))
}

fn build_public_live_quota(
    row: LiveQuotaRow,
    ledger_days: Vec<PublicLedgerDay>,
    now_unix_ms: i64,
) -> rusqlite::Result<PublicLiveQuota> {
    let used = row
        .used_micropoints
        .map(u32::try_from)
        .transpose()
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let failures =
        u32::try_from(row.consecutive_failures).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let public_error = match row.error_key.as_deref() {
        Some(key) => {
            Some(UsageSourceErrorCode::from_storage_key(key).ok_or(rusqlite::Error::InvalidQuery)?)
        }
        None => None,
    };
    let source_status = if row.last_success_at_unix_ms.is_none() {
        SourceStatus::Unavailable
    } else if failures > 0 {
        SourceStatus::StaleAfterFailure
    } else if row
        .last_success_at_unix_ms
        .is_some_and(|success| now_unix_ms.saturating_sub(success) <= FRESH_FOR_MS)
    {
        SourceStatus::Fresh
    } else {
        SourceStatus::StaleByAge
    };
    let today = ledger_days.iter().find(|day| day.is_today);
    let today_base_micropoints = today.map(|day| day.base_micropoints);
    let today_carry_micropoints = today.map(|day| day.carry_micropoints);
    let today_limit_micropoints = today.map(|day| day.limit_micropoints);
    let today_available_micropoints = today.and_then(|day| {
        day.used_micropoints.and_then(|used| {
            u32::try_from(used)
                .ok()
                .map(|used| day.limit_micropoints.saturating_sub(used))
        })
    });
    Ok(PublicLiveQuota {
        used_micropoints: used,
        remaining_micropoints: used.map(|value| 100_000_000_u32.saturating_sub(value)),
        captured_at_unix_ms: row.captured_at_unix_ms,
        resets_at_unix_s: row.resets_at_unix_s,
        window_starts_at_unix_s: row
            .resets_at_unix_s
            .map(|reset| reset.saturating_sub(604_800)),
        window_ends_at_unix_s: row.resets_at_unix_s.map(|reset| reset.saturating_sub(1)),
        plan_type: row.plan_type,
        allowed: row.allowed.map(|value| value != 0),
        last_attempt_at_unix_ms: row.last_attempt_at_unix_ms,
        last_success_at_unix_ms: row.last_success_at_unix_ms,
        consecutive_failures: failures,
        source_status,
        public_error,
        today_base_micropoints,
        today_carry_micropoints,
        today_limit_micropoints,
        today_available_micropoints,
        ledger_days,
    })
}

fn parse_policy_timezone(value: &str) -> rusqlite::Result<chrono_tz::Tz> {
    value.parse().map_err(|_| rusqlite::Error::InvalidQuery)
}

fn backfill_usage_observation_epochs(
    transaction: &rusqlite::Transaction<'_>,
    policy_timezone: &str,
) -> rusqlite::Result<()> {
    let policy_timezone = parse_policy_timezone(policy_timezone)?;
    let stream_ids = {
        let mut statement = transaction.prepare(
            "SELECT DISTINCT account_stream_id
             FROM usage_observations
             WHERE ledger_eligible = 1
             ORDER BY account_stream_id",
        )?;
        statement
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<i64>>>()?
    };
    for stream_id in stream_ids {
        backfill_stream_observations(transaction, stream_id, policy_timezone)?;
    }
    Ok(())
}

fn backfill_stream_observations(
    transaction: &rusqlite::Transaction<'_>,
    stream_id: i64,
    policy_timezone: chrono_tz::Tz,
) -> rusqlite::Result<()> {
    let observations = {
        let mut statement = transaction.prepare(
            "SELECT id, captured_at_ms, used_micropoints, window_seconds,
                    resets_at_s, plan_type, allowed
             FROM usage_observations
             WHERE account_stream_id = ?1
               AND ledger_eligible = 1
             ORDER BY captured_at_ms, id",
        )?;
        statement
            .query_map([stream_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    WeeklyUsageObservation {
                        captured_at_unix_ms: row.get(1)?,
                        used: crate::QuotaUnits::from_micropoints(row.get(2)?)
                            .ok_or(rusqlite::Error::InvalidQuery)?,
                        window_seconds: u32::try_from(row.get::<_, i64>(3)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        resets_at_unix_s: row.get(4)?,
                        plan_type: row.get(5)?,
                        allowed: row.get::<_, Option<i64>>(6)?.map(|value| value != 0),
                    },
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut state = crate::LedgerState::default();
    for (observation_id, observation) in observations {
        let transition = QuotaLedger::apply(state, &observation, policy_timezone)
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let kind = transition.kind;
        let assigned_local_date = transition.assigned_local_date;
        state = transition.state;
        let epoch = QuotaLedger::persisted_epoch(&state).ok_or(rusqlite::Error::InvalidQuery)?;
        let epoch_id = persist_ledger_epoch(
            transaction,
            stream_id,
            &epoch,
            kind,
            observation.captured_at_unix_ms,
        )?;
        transaction.execute(
            "UPDATE usage_observations SET quota_epoch_id = ?1 WHERE id = ?2",
            rusqlite::params![epoch_id, observation_id],
        )?;
        if let Some(local_date) = assigned_local_date {
            let used = QuotaLedger::daily_used(&state, &local_date)
                .ok_or(rusqlite::Error::InvalidQuery)?;
            transaction.execute(
                "INSERT INTO daily_ledgers
                 (account_stream_id, local_date, policy_timezone,
                  used_micropoints, updated_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(account_stream_id, local_date, policy_timezone)
                 DO UPDATE SET used_micropoints = excluded.used_micropoints,
                               updated_at_ms = excluded.updated_at_ms",
                rusqlite::params![
                    stream_id,
                    local_date,
                    policy_timezone.name(),
                    used,
                    observation.captured_at_unix_ms
                ],
            )?;
        }
    }
    Ok(())
}

fn load_ledger_state(
    transaction: &rusqlite::Transaction<'_>,
    stream_id: i64,
    policy_timezone: chrono_tz::Tz,
) -> rusqlite::Result<crate::LedgerState> {
    let mut statement = transaction.prepare(
        "SELECT captured_at_ms, used_micropoints, window_seconds, resets_at_s,
                plan_type, allowed
         FROM usage_observations
         WHERE account_stream_id = ?1
           AND ledger_eligible = 1
         ORDER BY captured_at_ms, id",
    )?;
    let observations = statement
        .query_map([stream_id], |row| {
            let used: i64 = row.get(1)?;
            let window_seconds: i64 = row.get(2)?;
            Ok(WeeklyUsageObservation {
                captured_at_unix_ms: row.get(0)?,
                used: crate::QuotaUnits::from_micropoints(used)
                    .ok_or(rusqlite::Error::InvalidQuery)?,
                window_seconds: u32::try_from(window_seconds)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                resets_at_unix_s: row.get(3)?,
                plan_type: row.get(4)?,
                allowed: row.get::<_, Option<i64>>(5)?.map(|value| value != 0),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut state = crate::LedgerState::default();
    for observation in observations {
        state = QuotaLedger::apply(state, &observation, policy_timezone)
            .map_err(|_| rusqlite::Error::InvalidQuery)?
            .state;
    }
    Ok(state)
}

fn persist_ledger_epoch(
    transaction: &rusqlite::Transaction<'_>,
    stream_id: i64,
    epoch: &PersistedLedgerEpoch,
    kind: LedgerApplyKind,
    captured_at_unix_ms: i64,
) -> rusqlite::Result<i64> {
    match kind {
        LedgerApplyKind::Baseline => {
            transaction.execute(
                "INSERT INTO quota_epochs
                 (account_stream_id, sequence, baseline_micropoints,
                  high_water_micropoints, first_observed_at_ms,
                  latest_observed_at_ms, scheduled_reset_at_s, closed_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
                rusqlite::params![
                    stream_id,
                    i64::try_from(epoch.sequence).map_err(|_| rusqlite::Error::InvalidQuery)?,
                    epoch.baseline_micropoints,
                    epoch.high_water_micropoints,
                    epoch.first_observed_at_unix_ms,
                    epoch.latest_observed_at_unix_ms,
                    epoch.scheduled_reset_at_unix_s,
                ],
            )?;
            Ok(transaction.last_insert_rowid())
        }
        LedgerApplyKind::SameEpoch => {
            let updated = transaction.execute(
                "UPDATE quota_epochs
                 SET high_water_micropoints = ?1, latest_observed_at_ms = ?2,
                     scheduled_reset_at_s = ?3
                 WHERE account_stream_id = ?4 AND closed_at_ms IS NULL",
                rusqlite::params![
                    epoch.high_water_micropoints,
                    epoch.latest_observed_at_unix_ms,
                    epoch.scheduled_reset_at_unix_s,
                    stream_id,
                ],
            )?;
            if updated == 0 {
                transaction.execute(
                    "INSERT INTO quota_epochs
                     (account_stream_id, sequence, baseline_micropoints,
                      high_water_micropoints, first_observed_at_ms,
                      latest_observed_at_ms, scheduled_reset_at_s, closed_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
                    rusqlite::params![
                        stream_id,
                        i64::try_from(epoch.sequence).map_err(|_| rusqlite::Error::InvalidQuery)?,
                        epoch.baseline_micropoints,
                        epoch.high_water_micropoints,
                        epoch.first_observed_at_unix_ms,
                        epoch.latest_observed_at_unix_ms,
                        epoch.scheduled_reset_at_unix_s,
                    ],
                )?;
            }
            transaction.query_row(
                "SELECT id FROM quota_epochs
                 WHERE account_stream_id = ?1 AND closed_at_ms IS NULL",
                [stream_id],
                |row| row.get(0),
            )
        }
        LedgerApplyKind::ConfirmedReset => {
            transaction.execute(
                "UPDATE quota_epochs SET closed_at_ms = ?1
                 WHERE account_stream_id = ?2 AND closed_at_ms IS NULL",
                rusqlite::params![captured_at_unix_ms, stream_id],
            )?;
            transaction.execute(
                "INSERT INTO quota_epochs
                 (account_stream_id, sequence, baseline_micropoints,
                  high_water_micropoints, first_observed_at_ms,
                  latest_observed_at_ms, scheduled_reset_at_s, closed_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
                rusqlite::params![
                    stream_id,
                    i64::try_from(epoch.sequence).map_err(|_| rusqlite::Error::InvalidQuery)?,
                    epoch.baseline_micropoints,
                    epoch.high_water_micropoints,
                    epoch.first_observed_at_unix_ms,
                    epoch.latest_observed_at_unix_ms,
                    epoch.scheduled_reset_at_unix_s,
                ],
            )?;
            Ok(transaction.last_insert_rowid())
        }
        LedgerApplyKind::DroppedOutOfOrder => Err(rusqlite::Error::InvalidQuery),
    }
}

fn upsert_account_stream(
    transaction: &rusqlite::Transaction<'_>,
    salt: &[u8],
    account_id: &str,
    observed_at_unix_ms: i64,
) -> rusqlite::Result<(i64, [u8; 32])> {
    let account_key = account_key(salt, account_id);
    transaction.execute(
        "INSERT INTO account_streams
         (stream_key, account_key, first_seen_at_ms, last_seen_at_ms)
         VALUES (?1, ?2, ?3, ?3)
         ON CONFLICT(account_key) DO UPDATE
           SET last_seen_at_ms = excluded.last_seen_at_ms",
        rusqlite::params![
            Uuid::now_v7().to_string(),
            account_key.as_slice(),
            observed_at_unix_ms
        ],
    )?;
    let stream_id = transaction.query_row(
        "SELECT id FROM account_streams WHERE account_key = ?1",
        [account_key.as_slice()],
        |row| row.get(0),
    )?;
    Ok((stream_id, account_key))
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
