//! Framework-independent `QuotaTide` domain core.

use serde::Serialize;
use ts_rs::TS;

mod account_settings;
mod live_quota;
mod quota_ledger;
mod quota_policy;
mod tray_shell;

pub use account_settings::{
    AccountApplication, AccountConfigError, AccountSettingsStore, AuthCandidateValidator,
    PublicAccountSettings, PublicError, PublicErrorCode, PublicQuotaPolicy, QuotaPolicyDraft,
    SafeErrorContext, SettingsManager, SettingsStoreError, UsageCommitDisposition,
    ValidatedAccountCandidate,
};
pub use live_quota::{
    Application, Clock, CurrentUsageAuth, DashboardChanged, LedgerDayStatus, PublicLedgerDay,
    PublicLiveQuota, PublicLiveQuotaState, QuotaUnits, RefreshAccountBinding, RefreshCoordinator,
    RefreshCoordinatorError, RefreshOutcome, RefreshReceipt, RefreshTrigger, SourceStatus,
    UsageAuthReadFailure, UsageRefreshAttempt, UsageRefreshSource, UsageSourceError,
    UsageSourceErrorCode, WeeklyUsageObservation,
};
pub use quota_ledger::{
    DailyUsageFact, LedgerApplyKind, LedgerError, LedgerProjection, LedgerState, LedgerTransition,
    QuotaLedger,
};
pub use quota_policy::{
    DailyLimitSnapshot, DailyPolicyStatus, PolicyDayFact, PolicyDayProjection, PolicyError,
    QuotaPolicy, ThresholdTransition,
};
pub use tray_shell::{
    PhysicalPoint, PhysicalRect, PhysicalSize, ShellEffect, ShellEvent, TrayShell,
    place_tray_window,
};

/// Public, non-secret metadata exposed by the desktop shell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct BuildInfo {
    pub product_name: String,
    pub version: String,
    pub author: String,
    pub identifier: String,
    pub stage: String,
}

/// Returns the public metadata for this `QuotaTide` build.
#[must_use]
pub fn build_info() -> BuildInfo {
    BuildInfo {
        product_name: "QuotaTide".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        author: "TheBlind".to_owned(),
        identifier: "dev.theblind.quotatide".to_owned(),
        stage: "skeleton".to_owned(),
    }
}
