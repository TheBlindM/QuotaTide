use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, watch};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::{
    AccountApplication, AccountConfigError, AccountSettingsStore, AuthCandidateValidator,
    PublicAccountSettings, SettingsStoreError,
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
}

impl UsageSourceErrorCode {
    pub(crate) const fn as_storage_key(self) -> &'static str {
        match self {
            Self::AuthPathUnavailable => "auth_path_unavailable",
            Self::AuthenticationStale => "authentication_stale",
            Self::PermissionDenied => "permission_denied",
            Self::RateLimited => "rate_limited",
            Self::Timeout => "timeout",
            Self::UpstreamUnavailable => "upstream_unavailable",
            Self::ResponseTooLarge => "response_too_large",
            Self::InvalidJson => "invalid_json",
            Self::ContractViolation => "contract_violation",
            Self::WeeklyWindowUnavailable => "weekly_window_unavailable",
        }
    }

    pub(crate) fn from_storage_key(value: &str) -> Option<Self> {
        match value {
            "auth_path_unavailable" => Some(Self::AuthPathUnavailable),
            "authentication_stale" => Some(Self::AuthenticationStale),
            "permission_denied" => Some(Self::PermissionDenied),
            "rate_limited" => Some(Self::RateLimited),
            "timeout" => Some(Self::Timeout),
            "upstream_unavailable" => Some(Self::UpstreamUnavailable),
            "response_too_large" => Some(Self::ResponseTooLarge),
            "invalid_json" => Some(Self::InvalidJson),
            "contract_violation" => Some(Self::ContractViolation),
            "weekly_window_unavailable" => Some(Self::WeeklyWindowUnavailable),
            _ => None,
        }
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
}

/// Clock seam used by scheduling and deterministic coordinator tests.
pub trait Clock: Clone + Send + Sync + 'static {
    fn now_unix_ms(&self) -> i64;
}

/// One complete current-account collection attempt. Production adapters own
/// auth re-reading and the conditional token-rotation retry.
pub trait UsageRefreshSource: Clone + Send + Sync + 'static {
    fn fetch(
        &self,
        captured_at_unix_ms: i64,
    ) -> impl std::future::Future<Output = UsageRefreshAttempt> + Send;
}

#[derive(Default)]
struct RefreshState {
    in_flight: Option<watch::Receiver<Option<Result<RefreshReceipt, RefreshCoordinatorError>>>>,
    last_manual_started_at_unix_ms: Option<i64>,
}

/// Core-owned single-flight refresh use case.
#[derive(Clone)]
pub struct RefreshCoordinator<S, C> {
    source: S,
    clock: C,
    store: AccountSettingsStore,
    state: std::sync::Arc<Mutex<RefreshState>>,
}

/// The sole application facade consumed by the native shell.
#[derive(Clone)]
pub struct Application<V, S, C> {
    account: AccountApplication<V>,
    refresh: RefreshCoordinator<S, C>,
    scheduler: Arc<SchedulerControl>,
}

struct SchedulerControl {
    cancellation: CancellationToken,
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
        Self {
            account,
            refresh,
            scheduler: Arc::new(SchedulerControl {
                cancellation: CancellationToken::new(),
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

    /// Returns the secret-free live quota projection.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the projection cannot be read.
    pub async fn live_quota(
        &self,
        now_unix_ms: i64,
    ) -> Result<Option<PublicLiveQuota>, AccountConfigError<V::Error>> {
        self.account.live_quota(now_unix_ms).await
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
    /// A resume signal performs at most one overdue refresh and resets the next
    /// hourly deadline to one hour after resume.
    pub async fn run_hourly_scheduler<F>(&self, refresh_on_startup: bool, on_refresh: F)
    where
        F: Fn() + Send + Sync,
    {
        const HOUR_MS: i64 = 60 * 60 * 1000;
        let cancellation = self.scheduler.cancellation.clone();
        let mut resumes = self.scheduler.resume_sender.subscribe();
        let mut interval = tokio::time::interval(Duration::from_secs(60 * 60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await;

        if refresh_on_startup
            && run_scheduled_refresh(&cancellation, self.refresh(RefreshTrigger::Startup)).await
        {
            on_refresh();
        }

        loop {
            tokio::select! {
                () = cancellation.cancelled() => break,
                _ = interval.tick() => {
                    if run_scheduled_refresh(
                        &cancellation,
                        self.refresh(RefreshTrigger::Hourly),
                    ).await {
                        on_refresh();
                    } else {
                        break;
                    }
                }
                changed = resumes.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    if run_scheduled_refresh(
                        &cancellation,
                        self.refresh_if_due(RefreshTrigger::Resume, HOUR_MS),
                    ).await {
                        on_refresh();
                    } else {
                        break;
                    }
                    interval.reset();
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
}

async fn run_scheduled_refresh(
    cancellation: &CancellationToken,
    refresh: impl std::future::Future<Output = Result<RefreshReceipt, RefreshCoordinatorError>>,
) -> bool {
    tokio::select! {
        () = cancellation.cancelled() => false,
        _ = refresh => true,
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
    #[must_use]
    pub fn new(store: AccountSettingsStore, source: S, clock: C) -> Self {
        Self {
            source,
            clock,
            store,
            state: std::sync::Arc::new(Mutex::new(RefreshState::default())),
        }
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
            Cooldown(i64),
        }

        let now = self.clock.now_unix_ms();
        let role = {
            let mut state = self.state.lock().await;
            if let Some(receiver) = state.in_flight.clone() {
                if trigger == RefreshTrigger::Manual
                    && state
                        .last_manual_started_at_unix_ms
                        .is_none_or(|last| now.saturating_sub(last) >= 30_000)
                {
                    state.last_manual_started_at_unix_ms = Some(now);
                }
                Role::Follower(receiver)
            } else if trigger == RefreshTrigger::Manual
                && state
                    .last_manual_started_at_unix_ms
                    .is_some_and(|last| now.saturating_sub(last) < 30_000)
            {
                Role::Cooldown(now)
            } else {
                if trigger == RefreshTrigger::Manual {
                    state.last_manual_started_at_unix_ms = Some(now);
                }
                let (sender, receiver) = watch::channel(None);
                state.in_flight = Some(receiver);
                Role::Leader(sender)
            }
        };

        match role {
            Role::Cooldown(attempted_at_unix_ms) => Ok(RefreshReceipt {
                attempted_at_unix_ms,
                outcome: RefreshOutcome::ManualCooldown,
            }),
            Role::Follower(mut receiver) => loop {
                if let Some(result) = receiver.borrow().clone() {
                    return result;
                }
                receiver
                    .changed()
                    .await
                    .map_err(|_| RefreshCoordinatorError::FlightUnavailable)?;
            },
            Role::Leader(sender) => {
                let result = self.run_once(now).await;
                let _ = sender.send(Some(result.clone()));
                self.state.lock().await.in_flight = None;
                result
            }
        }
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
        let current = self.store.public_live_quota(now).await.map_err(map_store)?;
        if current
            .is_some_and(|quota| now.saturating_sub(quota.last_attempt_at_unix_ms) < interval_ms)
        {
            return Ok(RefreshReceipt {
                attempted_at_unix_ms: now,
                outcome: RefreshOutcome::NotDue,
            });
        }
        self.refresh(trigger).await
    }

    async fn run_once(
        &self,
        attempted_at_unix_ms: i64,
    ) -> Result<RefreshReceipt, RefreshCoordinatorError> {
        let attempt = self.source.fetch(attempted_at_unix_ms).await;
        let outcome = match attempt.result {
            Ok(observation) => {
                let Some(binding) = attempt.binding.as_ref() else {
                    return Ok(RefreshReceipt {
                        attempted_at_unix_ms,
                        outcome: RefreshOutcome::Superseded,
                    });
                };
                match self
                    .store
                    .record_usage_success(binding, observation)
                    .await
                    .map_err(map_store)?
                {
                    crate::UsageCommitDisposition::Committed => RefreshOutcome::Updated,
                    crate::UsageCommitDisposition::Superseded => RefreshOutcome::Superseded,
                }
            }
            Err(error) => {
                let code = error.code();
                if let Some(binding) = attempt.binding.as_ref()
                    && self
                        .store
                        .record_usage_failure(binding, attempted_at_unix_ms, code)
                        .await
                        .map_err(map_store)?
                        == crate::UsageCommitDisposition::Superseded
                {
                    return Ok(RefreshReceipt {
                        attempted_at_unix_ms,
                        outcome: RefreshOutcome::Superseded,
                    });
                }
                RefreshOutcome::Failed(code)
            }
        };
        Ok(RefreshReceipt {
            attempted_at_unix_ms,
            outcome,
        })
    }
}

fn map_store(error: SettingsStoreError) -> RefreshCoordinatorError {
    RefreshCoordinatorError::StorageUnavailable(Arc::new(error))
}

#[cfg(test)]
mod tests {
    use super::QuotaUnits;

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
}
