//! Framework-independent `QuotaTide` domain core.

use serde::Serialize;
use ts_rs::TS;

mod account_settings;
mod alerts;
mod email;
mod live_quota;
mod local_data;
mod quota_ledger;
mod reset_radar;
mod tray_shell;

pub use account_settings::{
    AccountApplication, AccountConfigError, AccountSettingsStore, AlertChannel, AlertEventKind,
    AlertPreference, AlertPreferenceDraft, AtomicSettingsError, AtomicSettingsManager,
    AuthCandidateValidator, AutostartControl, CredentialVault, InterfaceLocalePreference,
    PublicAccountSettings, PublicError, PublicErrorCode, PublicQuotaPolicy, PublicSettings,
    PublicSmtpRecipient, PublicSmtpSettings, QuotaPolicyDraft, SafeErrorContext, SecretUpdate,
    SettingsDraft, SettingsManager, SettingsStoreError, SmtpCredentialStatus, SmtpRecipientDraft,
    SmtpSettingsDraft, SmtpTlsMode, StoryTheme, TrayDisplayMode, UsageCommitDisposition,
    ValidatedAccountCandidate,
};
pub use alerts::{
    AlertTarget, DeliverySweep, DeliveryWorker, NotificationPermissionStatus, PublicAlertEvent,
    PublicAlertInbox, PublicDeliveryState, SafeNotification, SystemNotifier,
};
pub use email::{EmailDeliveryWorker, MailTransport, SafeMail, SmtpConnection, TestEmailError};
pub use live_quota::{
    Application, Clock, CurrentUsageAuth, DashboardChanged, LedgerDayStatus, PublicBurnProjection,
    PublicLedgerDay, PublicLiveQuota, PublicLiveQuotaState, PublicResetCredit, PublicResetCredits,
    QuotaPressure, QuotaUnits, RefreshAccountBinding, RefreshCoordinator, RefreshCoordinatorError,
    RefreshOutcome, RefreshReceipt, RefreshTrigger, SettingsChanged, SourceStatus,
    UsageAuthReadFailure, UsageRefreshAttempt, UsageRefreshSource, UsageSourceError,
    UsageSourceErrorCode, WeeklyUsageObservation,
};
pub use quota_ledger::{
    DailyUsageFact, LedgerApplyKind, LedgerError, LedgerProjection, LedgerState, LedgerTransition,
    QuotaLedger,
};
pub use reset_radar::{
    PublicRadarAnnouncement, PublicRadarPrediction, PublicResetRadar, RadarAnnouncement,
    RadarChance, RadarCommitDisposition, RadarContractError, RadarObservation, RadarSnapshot,
    RadarSourceError, RadarSourceErrorCode, ResetRadarSource, radar_bucket_label,
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
