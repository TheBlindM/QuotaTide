use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, watch};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::{
    AccountApplication, AccountConfigError, AccountSettingsStore, AuthCandidateValidator,
    PublicAccountSettings, PublicResetRadar, ResetRadarSource, SettingsStoreError,
};

/// Millionths of one percentage point. `100% == 100_000_000`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct QuotaUnits(i64);

impl QuotaUnits {
    pub const FULL: Self = Self(100_000_000);

    #[must_use]
    pub const fn from_micropoints(value: i64) -> Option<Self> {
        if value >= 0 && value <= Self::FULL.0 {
            Some(Self(value))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn micropoints(self) -> i64 {
        self.0
    }
}

/// Strictly normalized current seven-day quota observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeeklyUsageObservation {
    pub captured_at_unix_ms: i64,
    pub used: QuotaUnits,
    pub window_seconds: u32,
    pub resets_at_unix_s: i64,
    pub plan_type: Option<String>,
    pub allowed: Option<bool>,
}

/// In-memory binding between one refresh attempt and the selected account.
///
/// Its path and account identity are intentionally redacted from `Debug` and
/// are never serialized or persisted in raw form.
#[derive(Clone, PartialEq, Eq)]
pub struct RefreshAccountBinding {
    pub(crate) settings_revision: u32,
    pub(crate) canonical_path: PathBuf,
    pub(crate) canonical_account_id: Option<String>,
}

impl std::fmt::Debug for RefreshAccountBinding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RefreshAccountBinding")
            .field("settings_revision", &self.settings_revision)
            .field("canonical_path", &"<redacted>")
            .field("canonical_account_id", &"<redacted>")
            .finish()
    }
}

impl RefreshAccountBinding {
    #[must_use]
    pub fn selected(settings_revision: u32, canonical_path: PathBuf) -> Self {
        Self {
            settings_revision,
            canonical_path,
            canonical_account_id: None,
        }
    }

    #[must_use]
    pub fn with_account_id(mut self, canonical_account_id: String) -> Self {
        self.canonical_account_id = Some(canonical_account_id);
        self
    }

    #[must_use]
    pub fn canonical_path(&self) -> &std::path::Path {
        &self.canonical_path
    }
}

/// Stable source failure categories; raw upstream content never crosses this boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum UsageSourceErrorCode {
    AuthPathUnavailable,
    AuthenticationStale,
    PermissionDenied,
    RateLimited,
    Timeout,
    UpstreamUnavailable,
    ResponseTooLarge,
    InvalidJson,
    ContractViolation,
    WeeklyWindowUnavailable,
}

/// Internal source failure with a stable public category and retained cause.
#[derive(Debug, Clone, thiserror::Error)]
#[error("live quota source failed: {code:?}")]
pub struct UsageSourceError {
    code: UsageSourceErrorCode,
    #[source]
    source: Option<Arc<dyn Error + Send + Sync>>,
}

impl UsageSourceError {
    #[must_use]
    pub const fn new(code: UsageSourceErrorCode) -> Self {
        Self { code, source: None }
    }

    #[must_use]
    pub fn with_source(
        code: UsageSourceErrorCode,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            code,
            source: Some(Arc::new(source)),
        }
    }

    #[must_use]
    pub const fn code(&self) -> UsageSourceErrorCode {
        self.code
    }
}

/// Complete source result, including the account selection observed at read time.
pub struct UsageRefreshAttempt {
    pub(crate) binding: Option<RefreshAccountBinding>,
    pub(crate) result: Result<WeeklyUsageObservation, UsageSourceError>,
}

impl UsageRefreshAttempt {
    #[must_use]
    pub fn success(binding: RefreshAccountBinding, observation: WeeklyUsageObservation) -> Self {
        Self {
            binding: Some(binding),
            result: Ok(observation),
        }
    }

    #[must_use]
    pub fn failure(binding: Option<RefreshAccountBinding>, error: UsageSourceError) -> Self {
        Self {
            binding,
            result: Err(error),
        }
    }

    /// Returns the source result while keeping the account binding internal.
    ///
    /// # Errors
    ///
    /// Returns the categorized source failure with its internal source chain.
    pub fn into_result(self) -> Result<WeeklyUsageObservation, UsageSourceError> {
        self.result
    }
}

/// Complete source state; the UI only localizes this Rust-owned decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum SourceStatus {
    Fresh,
    StaleAfterFailure,
    StaleByAge,
    Unavailable,
}

/// Secret-free live quota projection returned by the application query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct PublicLiveQuota {
    pub used_micropoints: Option<u32>,
    pub remaining_micropoints: Option<u32>,
    #[ts(type = "number | null")]
    pub captured_at_unix_ms: Option<i64>,
    #[ts(type = "number | null")]
    pub resets_at_unix_s: Option<i64>,
    #[ts(type = "number | null")]
    pub window_starts_at_unix_s: Option<i64>,
    #[ts(type = "number | null")]
    pub window_ends_at_unix_s: Option<i64>,
    pub plan_type: Option<String>,
    pub allowed: Option<bool>,
    #[ts(type = "number")]
    pub last_attempt_at_unix_ms: i64,
    #[ts(type = "number | null")]
    pub last_success_at_unix_ms: Option<i64>,
    pub consecutive_failures: u32,
    pub source_status: SourceStatus,
    pub public_error: Option<UsageSourceErrorCode>,
    pub today_base_micropoints: Option<u32>,
    pub today_carry_micropoints: Option<u32>,
    pub today_limit_micropoints: Option<u32>,
    pub today_available_micropoints: Option<u32>,
    pub ledger_days: Vec<PublicLedgerDay>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum LedgerDayStatus {
    Unknown,
    Normal,
    Warning,
    Exceeded,
    Finalized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct PublicLedgerDay {
    pub local_date: String,
    #[ts(type = "number | null")]
    pub used_micropoints: Option<i64>,
    #[ts(type = "number")]
    pub policy_revision: u64,
    pub policy_timezone: String,
    pub base_micropoints: u32,
    pub carry_micropoints: u32,
    pub limit_micropoints: u32,
    pub is_today: bool,
    pub finalized: bool,
    pub status: LedgerDayStatus,
}

/// Complete query result for the current dashboard projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct PublicLiveQuotaState {
    #[ts(type = "number")]
    pub dashboard_revision: u64,
    pub refreshing: bool,
    pub quota: Option<PublicLiveQuota>,
    pub radar: PublicResetRadar,
}

/// Small native event that tells the UI to re-query the dashboard projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct DashboardChanged {
    #[ts(type = "number")]
    pub revision: u64,
}

/// Small native event that tells the UI to re-query public settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct SettingsChanged {
    pub revision: u32,
}

impl UsageSourceErrorCode {
    const STORAGE_KEYS: &'static [(Self, &'static str)] = &[
        (Self::AuthPathUnavailable, "auth_path_unavailable"),
        (Self::AuthenticationStale, "authentication_stale"),
        (Self::PermissionDenied, "permission_denied"),
        (Self::RateLimited, "rate_limited"),
        (Self::Timeout, "timeout"),
        (Self::UpstreamUnavailable, "upstream_unavailable"),
        (Self::ResponseTooLarge, "response_too_large"),
        (Self::InvalidJson, "invalid_json"),
        (Self::ContractViolation, "contract_violation"),
        (Self::WeeklyWindowUnavailable, "weekly_window_unavailable"),
    ];

    pub(crate) fn as_storage_key(self) -> &'static str {
        Self::STORAGE_KEYS
            .iter()
            .find_map(|(candidate, key)| (*candidate == self).then_some(*key))
            .expect("every usage source error code has one storage key")
    }

    pub(crate) fn from_storage_key(value: &str) -> Option<Self> {
        Self::STORAGE_KEYS
            .iter()
            .find_map(|(candidate, key)| (*key == value).then_some(*candidate))
    }
}

/// Why one refresh request entered the coordinator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshTrigger {
    Startup,
    Hourly,
    Manual,
    Resume,
    Settings,
}

/// Result shared by every waiter attached to the same refresh flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshOutcome {
    Updated,
    Failed(UsageSourceErrorCode),
    Superseded,
    ManualCooldown,
    NotDue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshReceipt {
    pub attempted_at_unix_ms: i64,
    pub outcome: RefreshOutcome,
    /// Remaining core-owned manual cooldown after this call completes.
    pub retry_after_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CombinedRefreshDisposition {
    pub(crate) outcome: RefreshOutcome,
}

/// Clock seam used by scheduling and deterministic coordinator tests.
pub trait Clock: Clone + Send + Sync + 'static {
    fn now_unix_ms(&self) -> i64;
}

/// Current auth material returned by a native adapter.
///
/// The material and fingerprint are intentionally opaque and never implement
/// `Debug` or serialization.
pub struct CurrentUsageAuth<M> {
    binding: RefreshAccountBinding,
    material: M,
    credential_fingerprint: [u8; 32],
}

impl<M> CurrentUsageAuth<M> {
    #[must_use]
    pub const fn new(
        binding: RefreshAccountBinding,
        material: M,
        credential_fingerprint: [u8; 32],
    ) -> Self {
        Self {
            binding,
            material,
            credential_fingerprint,
        }
    }
}

/// Safe auth-read failure returned by a native adapter.
pub struct UsageAuthReadFailure {
    binding: Option<RefreshAccountBinding>,
    error: UsageSourceError,
}

impl UsageAuthReadFailure {
    #[must_use]
    pub const fn new(binding: Option<RefreshAccountBinding>, error: UsageSourceError) -> Self {
        Self { binding, error }
    }
}

/// Native seam for reading current auth and making one fixed-contract request.
///
/// The core coordinator owns every-round auth re-reading, credential rotation
/// comparison, and the single conditional retry.
pub trait UsageRefreshSource: Clone + Send + Sync + 'static {
    type AuthMaterial: Send + Sync + 'static;

    fn read_current_auth(
        &self,
    ) -> impl std::future::Future<
        Output = Result<CurrentUsageAuth<Self::AuthMaterial>, UsageAuthReadFailure>,
    > + Send;

    fn fetch_usage<'a>(
        &'a self,
        auth: &'a Self::AuthMaterial,
        captured_at_unix_ms: i64,
    ) -> impl std::future::Future<Output = Result<WeeklyUsageObservation, UsageSourceError>> + Send + 'a;
}

#[derive(Default)]
struct RefreshState {
    in_flight: Option<watch::Receiver<Option<Result<RefreshReceipt, RefreshCoordinatorError>>>>,
    last_manual_started_at_unix_ms: Option<i64>,
    refreshing: bool,
}

/// Core-owned single-flight refresh use case.
#[derive(Clone)]
pub struct RefreshCoordinator<S, C> {
    source: S,
    radar_source: Option<Arc<dyn ResetRadarSource>>,
    clock: C,
    store: AccountSettingsStore,
    state: std::sync::Arc<Mutex<RefreshState>>,
    dashboard_changes: watch::Sender<DashboardChanged>,
}

/// The sole application facade consumed by the native shell.
#[derive(Clone)]
pub struct Application<V, S, C> {
    account: AccountApplication<V>,
    refresh: RefreshCoordinator<S, C>,
    query: AppQuery,
    scheduler: Arc<SchedulerControl>,
}

#[derive(Clone)]
struct AppQuery {
    store: AccountSettingsStore,
}

impl AppQuery {
    const fn new(store: AccountSettingsStore) -> Self {
        Self { store }
    }

    async fn dashboard(
        &self,
        now_unix_ms: i64,
    ) -> Result<(u64, Option<PublicLiveQuota>, PublicResetRadar), SettingsStoreError> {
        self.store.public_dashboard_snapshot(now_unix_ms).await
    }
}

struct SchedulerControl {
    cancellation: CancellationToken,
    started: AtomicBool,
    resume_sequence: AtomicU64,
    resume_sender: watch::Sender<u64>,
}

impl<V, S, C> Application<V, S, C>
where
    V: AuthCandidateValidator,
    S: UsageRefreshSource,
    C: Clock,
{
    #[must_use]
    pub fn new(account: AccountApplication<V>, refresh: RefreshCoordinator<S, C>) -> Self {
        let (resume_sender, _) = watch::channel(0);
        let query = AppQuery::new(refresh.store.clone());
        Self {
            account,
            refresh,
            query,
            scheduler: Arc::new(SchedulerControl {
                cancellation: CancellationToken::new(),
                started: AtomicBool::new(false),
                resume_sequence: AtomicU64::new(0),
                resume_sender,
            }),
        }
    }

    /// Reads the current secret-free account settings.
    ///
    /// # Errors
    ///
    /// Returns a storage error when settings cannot be read.
    pub async fn account_settings(
        &self,
    ) -> Result<PublicAccountSettings, AccountConfigError<V::Error>> {
        self.account.account_settings().await
    }

    /// Validates and selects the current account.
    ///
    /// # Errors
    ///
    /// Returns validation, conflict, or storage errors.
    pub async fn select_account(
        &self,
        expected_revision: u32,
        path: &std::path::Path,
    ) -> Result<PublicAccountSettings, AccountConfigError<V::Error>> {
        self.account.select_account(expected_revision, path).await
    }

    /// Validates and activates a complete daily quota policy.
    ///
    /// # Errors
    ///
    /// Returns validation, conflict, or storage errors.
    pub async fn update_quota_policy(
        &self,
        expected_revision: u32,
        draft: crate::QuotaPolicyDraft,
    ) -> Result<PublicAccountSettings, AccountConfigError<V::Error>> {
        self.account
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
    ) -> Result<PublicLiveQuotaState, AccountConfigError<V::Error>> {
        loop {
            let before = self.refresh.refreshing().await;
            let (dashboard_revision, quota, radar) = self
                .query
                .dashboard(now_unix_ms)
                .await
                .map_err(AccountConfigError::Storage)?;
            let after = self.refresh.refreshing().await;
            if before == after {
                return Ok(PublicLiveQuotaState {
                    dashboard_revision,
                    refreshing: after,
                    quota,
                    radar,
                });
            }
        }
    }

    /// Runs or joins one refresh attempt.
    ///
    /// # Errors
    ///
    /// Returns an error only when health or observation state cannot be committed.
    pub async fn refresh(
        &self,
        trigger: RefreshTrigger,
    ) -> Result<RefreshReceipt, RefreshCoordinatorError> {
        self.refresh.refresh(trigger).await
    }

    /// Refreshes the account selected after a settings commit.
    ///
    /// If this call initially joins a flight bound to the previous settings
    /// revision, the superseded result is discarded and one new flight is
    /// started for the latest selection.
    ///
    /// # Errors
    ///
    /// Returns an error only when refresh state cannot be committed.
    pub async fn refresh_selected_account(
        &self,
    ) -> Result<RefreshReceipt, RefreshCoordinatorError> {
        loop {
            let receipt = self.refresh.refresh(RefreshTrigger::Settings).await?;
            if receipt.outcome != RefreshOutcome::Superseded {
                return Ok(receipt);
            }
        }
    }

    /// Runs one overdue refresh without replaying skipped scheduler ticks.
    ///
    /// # Errors
    ///
    /// Returns an error when due-state or refresh state cannot be read or committed.
    pub async fn refresh_if_due(
        &self,
        trigger: RefreshTrigger,
        interval_ms: i64,
    ) -> Result<RefreshReceipt, RefreshCoordinatorError> {
        self.refresh.refresh_if_due(trigger, interval_ms).await
    }

    /// Runs the cancellable startup/hourly scheduler until application shutdown.
    ///
    /// A resume signal performs at most one overdue refresh. Only an actual
    /// refresh resets the next hourly deadline; an early resume leaves the
    /// existing deadline intact.
    pub async fn run_hourly_scheduler(&self, refresh_on_startup: bool) {
        const HOUR_MS: i64 = 60 * 60 * 1000;
        if self
            .scheduler
            .started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let cancellation = self.scheduler.cancellation.clone();
        let mut resumes = self.scheduler.resume_sender.subscribe();
        let mut interval = tokio::time::interval(Duration::from_secs(60 * 60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await;

        if refresh_on_startup {
            let completed =
                run_scheduled_refresh(&cancellation, self.refresh(RefreshTrigger::Startup)).await;
            if completed.is_none() {
                return;
            }
        }

        loop {
            tokio::select! {
                () = cancellation.cancelled() => break,
                _ = interval.tick() => {
                    let completed = run_scheduled_refresh(
                        &cancellation,
                        self.refresh(RefreshTrigger::Hourly),
                    ).await;
                    if completed.is_none() {
                        break;
                    }
                }
                changed = resumes.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    let completed = run_scheduled_refresh(
                        &cancellation,
                        self.refresh_if_due(RefreshTrigger::Resume, HOUR_MS),
                    ).await;
                    let Some(result) = completed else {
                        break;
                    };
                    if result.is_ok_and(|receipt| receipt.outcome != RefreshOutcome::NotDue) {
                        interval.reset();
                    }
                }
            }
        }
    }

    /// Notifies the scheduler that the operating system resumed the app.
    pub fn notify_resume(&self) {
        let sequence = self
            .scheduler
            .resume_sequence
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        self.scheduler.resume_sender.send_replace(sequence);
    }

    /// Cancels scheduler work so it cannot hold application shutdown open.
    pub fn cancel_scheduler(&self) {
        self.scheduler.cancellation.cancel();
    }

    /// Subscribes to small revision-only dashboard invalidation events.
    #[must_use]
    pub fn subscribe_dashboard_changes(&self) -> watch::Receiver<DashboardChanged> {
        self.refresh.dashboard_changes.subscribe()
    }
}

async fn run_scheduled_refresh(
    cancellation: &CancellationToken,
    refresh: impl std::future::Future<Output = Result<RefreshReceipt, RefreshCoordinatorError>>,
) -> Option<Result<RefreshReceipt, RefreshCoordinatorError>> {
    tokio::select! {
        () = cancellation.cancelled() => None,
        result = refresh => Some(result),
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum RefreshCoordinatorError {
    #[error("live quota storage unavailable")]
    StorageUnavailable(#[source] Arc<SettingsStoreError>),
    #[error("live quota refresh flight ended before publishing a result")]
    FlightUnavailable,
}

impl<S: UsageRefreshSource, C: Clock> RefreshCoordinator<S, C> {
    const MANUAL_COOLDOWN_MS: i64 = 30_000;

    #[must_use]
    pub fn new(store: AccountSettingsStore, source: S, clock: C) -> Self {
        let (dashboard_changes, _) = watch::channel(DashboardChanged { revision: 0 });
        Self {
            source,
            radar_source: None,
            clock,
            store,
            state: std::sync::Arc::new(Mutex::new(RefreshState::default())),
            dashboard_changes,
        }
    }

    /// Adds the anonymous Reset Radar source to the same refresh flight.
    #[must_use]
    pub fn with_reset_radar_source<R: ResetRadarSource>(mut self, source: R) -> Self {
        self.radar_source = Some(Arc::new(source));
        self
    }

    async fn refreshing(&self) -> bool {
        let state = self.state.lock().await;
        state.refreshing
    }

    /// Runs or joins one refresh flight.
    ///
    /// # Errors
    ///
    /// Returns a storage error only when the observation/health transaction
    /// cannot be committed. Source failures are committed and returned in the
    /// receipt.
    pub async fn refresh(
        &self,
        trigger: RefreshTrigger,
    ) -> Result<RefreshReceipt, RefreshCoordinatorError> {
        enum Role {
            Leader(watch::Sender<Option<Result<RefreshReceipt, RefreshCoordinatorError>>>),
            Follower(watch::Receiver<Option<Result<RefreshReceipt, RefreshCoordinatorError>>>),
            Cooldown,
        }

        let now = self.clock.now_unix_ms();
        let (role, started) = {
            let mut state = self.state.lock().await;
            if let Some(receiver) = state.in_flight.clone() {
                if trigger == RefreshTrigger::Manual
                    && state
                        .last_manual_started_at_unix_ms
                        .is_none_or(|last| now.saturating_sub(last) >= Self::MANUAL_COOLDOWN_MS)
                {
                    state.last_manual_started_at_unix_ms = Some(now);
                }
                (Role::Follower(receiver), false)
            } else if trigger == RefreshTrigger::Manual
                && state
                    .last_manual_started_at_unix_ms
                    .is_some_and(|last| now.saturating_sub(last) < Self::MANUAL_COOLDOWN_MS)
            {
                (Role::Cooldown, false)
            } else {
                if trigger == RefreshTrigger::Manual {
                    state.last_manual_started_at_unix_ms = Some(now);
                }
                let (sender, receiver) = watch::channel(None);
                state.in_flight = Some(receiver);
                state.refreshing = true;
                (Role::Leader(sender), true)
            }
        };
        if started {
            if let Ok(revision) = self.persisted_dashboard_revision(now).await {
                self.dashboard_changes
                    .send_replace(DashboardChanged { revision });
            }
        }

        match role {
            Role::Cooldown => Ok(RefreshReceipt {
                attempted_at_unix_ms: now,
                outcome: RefreshOutcome::ManualCooldown,
                retry_after_ms: self.manual_retry_after_ms().await,
            }),
            Role::Follower(mut receiver) => loop {
                let published = receiver.borrow().clone();
                if let Some(result) = published {
                    return self.with_trigger_retry_after(result, trigger).await;
                }
                receiver
                    .changed()
                    .await
                    .map_err(|_| RefreshCoordinatorError::FlightUnavailable)?;
            },
            Role::Leader(sender) => {
                let refresh_result = self.run_once(now).await;
                {
                    let mut state = self.state.lock().await;
                    state.in_flight = None;
                    state.refreshing = false;
                }
                let revision_result = self.persisted_dashboard_revision(now).await;
                if let Ok(revision) = revision_result {
                    self.dashboard_changes
                        .send_replace(DashboardChanged { revision });
                }
                let result = match (refresh_result, revision_result) {
                    (Err(error), _) | (Ok(_), Err(error)) => Err(error),
                    (Ok(receipt), Ok(_)) => Ok(receipt),
                };
                let _ = sender.send(Some(result.clone()));
                self.with_trigger_retry_after(result, trigger).await
            }
        }
    }

    async fn persisted_dashboard_revision(
        &self,
        now_unix_ms: i64,
    ) -> Result<u64, RefreshCoordinatorError> {
        self.store
            .public_live_quota_snapshot(now_unix_ms)
            .await
            .map(|(revision, _)| revision)
            .map_err(map_store)
    }

    /// Refreshes once only when the latest attempt is at least `interval_ms`
    /// old. Used by resume handling so missed sleep ticks are never replayed.
    ///
    /// # Errors
    ///
    /// Returns a storage error when due-state or the refresh transaction cannot
    /// be read or committed.
    pub async fn refresh_if_due(
        &self,
        trigger: RefreshTrigger,
        interval_ms: i64,
    ) -> Result<RefreshReceipt, RefreshCoordinatorError> {
        let now = self.clock.now_unix_ms();
        let account_recheck_pending = self
            .store
            .radar_account_recheck_pending()
            .await
            .map_err(map_store)?;
        let (_, quota, radar) = self
            .store
            .public_dashboard_snapshot(now)
            .await
            .map_err(map_store)?;
        let last_attempt_at_unix_ms = quota
            .map(|quota| quota.last_attempt_at_unix_ms)
            .into_iter()
            .chain(radar.last_attempt_at_unix_ms)
            .max();
        if !account_recheck_pending
            && last_attempt_at_unix_ms
                .is_some_and(|attempt| now.saturating_sub(attempt) < interval_ms)
        {
            return Ok(RefreshReceipt {
                attempted_at_unix_ms: now,
                outcome: RefreshOutcome::NotDue,
                retry_after_ms: 0,
            });
        }
        self.refresh(trigger).await
    }

    async fn run_once(
        &self,
        attempted_at_unix_ms: i64,
    ) -> Result<RefreshReceipt, RefreshCoordinatorError> {
        let (attempt, radar_attempt) = if let Some(radar_source) = self.radar_source.as_ref() {
            let (usage, radar) = tokio::join!(
                collect_current_usage(&self.source, attempted_at_unix_ms),
                radar_source.fetch_radar(attempted_at_unix_ms),
            );
            (usage, Some(radar))
        } else {
            (
                collect_current_usage(&self.source, attempted_at_unix_ms).await,
                None,
            )
        };
        let disposition = self
            .store
            .record_refresh_attempt(attempt, radar_attempt, attempted_at_unix_ms)
            .await
            .map_err(map_store)?;
        Ok(RefreshReceipt {
            attempted_at_unix_ms,
            outcome: disposition.outcome,
            retry_after_ms: 0,
        })
    }

    async fn with_trigger_retry_after(
        &self,
        result: Result<RefreshReceipt, RefreshCoordinatorError>,
        trigger: RefreshTrigger,
    ) -> Result<RefreshReceipt, RefreshCoordinatorError> {
        let mut receipt = result?;
        if trigger == RefreshTrigger::Manual {
            receipt.retry_after_ms = self.manual_retry_after_ms().await;
        }
        Ok(receipt)
    }

    async fn manual_retry_after_ms(&self) -> u32 {
        let now = self.clock.now_unix_ms();
        let state = self.state.lock().await;
        let Some(started_at) = state.last_manual_started_at_unix_ms else {
            return 0;
        };
        let remaining = Self::MANUAL_COOLDOWN_MS
            .saturating_sub(now.saturating_sub(started_at))
            .max(0);
        u32::try_from(remaining).unwrap_or(u32::MAX)
    }
}

async fn collect_current_usage<S: UsageRefreshSource>(
    source: &S,
    captured_at_unix_ms: i64,
) -> UsageRefreshAttempt {
    let CurrentUsageAuth {
        binding,
        material: first,
        credential_fingerprint: first_fingerprint,
    } = match source.read_current_auth().await {
        Ok(current) => current,
        Err(failure) => {
            return UsageRefreshAttempt::failure(failure.binding, failure.error);
        }
    };

    match source.fetch_usage(&first, captured_at_unix_ms).await {
        Ok(observation) => UsageRefreshAttempt::success(binding, observation),
        Err(error)
            if matches!(
                error.code(),
                UsageSourceErrorCode::AuthenticationStale | UsageSourceErrorCode::PermissionDenied
            ) =>
        {
            let CurrentUsageAuth {
                binding: refreshed_binding,
                material: refreshed,
                credential_fingerprint: refreshed_fingerprint,
            } = match source.read_current_auth().await {
                Ok(current) => current,
                Err(failure) => {
                    return UsageRefreshAttempt::failure(
                        failure.binding.or(Some(binding)),
                        UsageSourceError::with_source(
                            UsageSourceErrorCode::AuthenticationStale,
                            failure.error,
                        ),
                    );
                }
            };
            if refreshed_fingerprint == first_fingerprint {
                return UsageRefreshAttempt::failure(
                    Some(binding),
                    UsageSourceError::with_source(UsageSourceErrorCode::AuthenticationStale, error),
                );
            }
            match source.fetch_usage(&refreshed, captured_at_unix_ms).await {
                Ok(observation) => UsageRefreshAttempt::success(refreshed_binding, observation),
                Err(retry_error) => UsageRefreshAttempt::failure(
                    Some(refreshed_binding),
                    UsageSourceError::with_source(
                        UsageSourceErrorCode::AuthenticationStale,
                        retry_error,
                    ),
                ),
            }
        }
        Err(error) => UsageRefreshAttempt::failure(Some(binding), error),
    }
}

fn map_store(error: SettingsStoreError) -> RefreshCoordinatorError {
    RefreshCoordinatorError::StorageUnavailable(Arc::new(error))
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use super::{
        CurrentUsageAuth, QuotaUnits, RefreshAccountBinding, UsageAuthReadFailure,
        UsageRefreshSource, UsageSourceError, UsageSourceErrorCode, WeeklyUsageObservation,
        collect_current_usage,
    };

    #[test]
    fn quota_units_are_bounded_and_exact() {
        assert_eq!(
            QuotaUnits::from_micropoints(41_250_000)
                .expect("valid units")
                .micropoints(),
            41_250_000
        );
        assert!(QuotaUnits::from_micropoints(-1).is_none());
        assert!(QuotaUnits::from_micropoints(100_000_001).is_none());
    }

    #[derive(Clone)]
    struct RotationSource {
        reads: Arc<AtomicUsize>,
        calls: Arc<AtomicUsize>,
        auth: Arc<Mutex<VecDeque<u8>>>,
        results: Arc<Mutex<VecDeque<Result<WeeklyUsageObservation, UsageSourceError>>>>,
    }

    impl UsageRefreshSource for RotationSource {
        type AuthMaterial = u8;

        async fn read_current_auth(
            &self,
        ) -> Result<CurrentUsageAuth<Self::AuthMaterial>, UsageAuthReadFailure> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            let auth = self
                .auth
                .lock()
                .expect("auth queue")
                .pop_front()
                .ok_or_else(|| {
                    UsageAuthReadFailure::new(
                        None,
                        UsageSourceError::new(UsageSourceErrorCode::AuthPathUnavailable),
                    )
                })?;
            Ok(CurrentUsageAuth::new(
                RefreshAccountBinding::selected(1, std::path::PathBuf::from("/chosen/auth.json"))
                    .with_account_id("account-ticket17".to_owned()),
                auth,
                [auth; 32],
            ))
        }

        async fn fetch_usage<'a>(
            &'a self,
            _auth: &'a Self::AuthMaterial,
            _captured_at_unix_ms: i64,
        ) -> Result<WeeklyUsageObservation, UsageSourceError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.results
                .lock()
                .expect("result queue")
                .pop_front()
                .expect("queued result")
        }
    }

    #[tokio::test]
    async fn auth_failure_retries_once_only_after_core_detects_rotation() {
        let source = rotation_source(
            [1, 2],
            [
                Err(UsageSourceError::new(
                    UsageSourceErrorCode::AuthenticationStale,
                )),
                Ok(test_observation()),
            ],
        );

        let result = collect_current_usage(&source, 1_785_000_000_000)
            .await
            .into_result();

        assert!(result.is_ok());
        assert_eq!(source.reads.load(Ordering::SeqCst), 2);
        assert_eq!(source.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn unchanged_credentials_normalize_permission_failure_as_stale_authentication() {
        let source = rotation_source(
            [1, 1],
            [Err(UsageSourceError::new(
                UsageSourceErrorCode::PermissionDenied,
            ))],
        );

        let error = collect_current_usage(&source, 1_785_000_000_000)
            .await
            .into_result()
            .expect_err("stale authentication");

        assert_eq!(error.code(), UsageSourceErrorCode::AuthenticationStale);
        assert_eq!(source.reads.load(Ordering::SeqCst), 2);
        assert_eq!(source.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn failed_rotation_retry_is_normalized_and_retains_source_chain() {
        let source = rotation_source(
            [1, 2],
            [
                Err(UsageSourceError::new(
                    UsageSourceErrorCode::AuthenticationStale,
                )),
                Err(UsageSourceError::new(
                    UsageSourceErrorCode::PermissionDenied,
                )),
            ],
        );

        let error = collect_current_usage(&source, 1_785_000_000_000)
            .await
            .into_result()
            .expect_err("retry remains stale");

        assert_eq!(error.code(), UsageSourceErrorCode::AuthenticationStale);
        assert!(std::error::Error::source(&error).is_some());
        assert_eq!(source.reads.load(Ordering::SeqCst), 2);
        assert_eq!(source.calls.load(Ordering::SeqCst), 2);
    }

    fn rotation_source(
        auth: impl IntoIterator<Item = u8>,
        results: impl IntoIterator<Item = Result<WeeklyUsageObservation, UsageSourceError>>,
    ) -> RotationSource {
        RotationSource {
            reads: Arc::new(AtomicUsize::new(0)),
            calls: Arc::new(AtomicUsize::new(0)),
            auth: Arc::new(Mutex::new(auth.into_iter().collect())),
            results: Arc::new(Mutex::new(results.into_iter().collect())),
        }
    }

    fn test_observation() -> WeeklyUsageObservation {
        WeeklyUsageObservation {
            captured_at_unix_ms: 1_785_000_000_000,
            used: QuotaUnits::from_micropoints(20_000_000).expect("valid quota"),
            window_seconds: 604_800,
            resets_at_unix_s: 1_786_000_000,
            plan_type: Some("plus".to_owned()),
            allowed: Some(true),
        }
    }
}
