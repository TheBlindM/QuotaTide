use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, watch};
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

/// Freshness state projected alongside the last-known-good observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum SourceFreshness {
    Fresh,
    Stale,
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
    pub plan_type: Option<String>,
    pub allowed: Option<bool>,
    #[ts(type = "number")]
    pub last_attempt_at_unix_ms: i64,
    #[ts(type = "number | null")]
    pub last_success_at_unix_ms: Option<i64>,
    pub consecutive_failures: u32,
    pub freshness: SourceFreshness,
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
    ) -> impl std::future::Future<Output = Result<WeeklyUsageObservation, UsageSourceErrorCode>> + Send;
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
}

impl<V, S, C> Application<V, S, C>
where
    V: AuthCandidateValidator,
    S: UsageRefreshSource,
    C: Clock,
{
    #[must_use]
    pub const fn new(account: AccountApplication<V>, refresh: RefreshCoordinator<S, C>) -> Self {
        Self { account, refresh }
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

    /// Returns the selected path for native adapter composition only.
    ///
    /// # Errors
    ///
    /// Returns a storage error when settings cannot be read.
    pub async fn configured_auth_path(
        &self,
    ) -> Result<Option<std::path::PathBuf>, AccountConfigError<V::Error>> {
        self.account.configured_auth_path().await
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RefreshCoordinatorError {
    #[error("live quota storage unavailable")]
    StorageUnavailable,
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
                if let Some(result) = *receiver.borrow() {
                    return result;
                }
                receiver
                    .changed()
                    .await
                    .map_err(|_| RefreshCoordinatorError::StorageUnavailable)?;
            },
            Role::Leader(sender) => {
                let result = self.run_once(now).await;
                let _ = sender.send(Some(result));
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
        let outcome = match self.source.fetch(attempted_at_unix_ms).await {
            Ok(observation) => {
                self.store
                    .record_usage_success(observation)
                    .await
                    .map_err(map_store)?;
                RefreshOutcome::Updated
            }
            Err(error) => {
                self.store
                    .record_usage_failure(attempted_at_unix_ms, error)
                    .await
                    .map_err(map_store)?;
                RefreshOutcome::Failed(error)
            }
        };
        Ok(RefreshReceipt {
            attempted_at_unix_ms,
            outcome,
        })
    }
}

fn map_store(_error: SettingsStoreError) -> RefreshCoordinatorError {
    RefreshCoordinatorError::StorageUnavailable
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
