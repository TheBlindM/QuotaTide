use std::error::Error;
use std::future::Future;

use serde::Serialize;
use ts_rs::TS;

use crate::{AccountSettingsStore, AlertEventKind, InterfaceLocalePreference, SettingsStoreError};

/// Current operating-system notification authorization state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum NotificationPermissionStatus {
    Unknown,
    Granted,
    Denied,
    Error,
}

impl NotificationPermissionStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Granted => "granted",
            Self::Denied => "denied",
            Self::Error => "error",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, SettingsStoreError> {
        match value {
            "unknown" => Ok(Self::Unknown),
            "granted" => Ok(Self::Granted),
            "denied" => Ok(Self::Denied),
            "error" => Ok(Self::Error),
            _ => Err(SettingsStoreError::InvalidNotificationState),
        }
    }
}

/// Overview section associated with an alert or notification click.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum AlertTarget {
    Today,
    Radar,
    Source,
}

impl AlertTarget {
    pub(crate) fn parse(value: &str) -> Result<Self, SettingsStoreError> {
        match value {
            "today" => Ok(Self::Today),
            "radar" => Ok(Self::Radar),
            "source" => Ok(Self::Source),
            _ => Err(SettingsStoreError::InvalidNotificationState),
        }
    }
}

/// Current system-channel state displayed with one in-app reminder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum PublicDeliveryState {
    Pending,
    Delivered,
    RetryWait,
    PausedPermission,
    Failed,
}

impl PublicDeliveryState {
    pub(crate) fn parse(value: &str) -> Result<Self, SettingsStoreError> {
        match value {
            "pending" | "leased" => Ok(Self::Pending),
            "delivered" => Ok(Self::Delivered),
            "retry_wait" => Ok(Self::RetryWait),
            "paused_permission" => Ok(Self::PausedPermission),
            "paused_config" | "cancelled_by_config" | "permanent_failure" => Ok(Self::Failed),
            _ => Err(SettingsStoreError::InvalidNotificationState),
        }
    }
}

/// Secret-free persisted reminder rendered by the current UI locale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct PublicAlertEvent {
    #[ts(type = "number")]
    pub event_id: u64,
    pub event_kind: AlertEventKind,
    pub local_date: Option<String>,
    pub source: Option<String>,
    pub target: AlertTarget,
    pub system_delivery_state: Option<PublicDeliveryState>,
    #[ts(type = "number")]
    pub created_at_unix_ms: i64,
}

/// In-app reminder projection and current notification authorization state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct PublicAlertInbox {
    pub notification_permission_status: NotificationPermissionStatus,
    pub events: Vec<PublicAlertEvent>,
}

/// Fully rendered, secret-free system notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeNotification {
    pub delivery_key: String,
    pub title: String,
    pub body: String,
    pub target: AlertTarget,
}

/// Rust-side operating-system notification seam.
pub trait SystemNotifier: Clone + Send + Sync + 'static {
    type Error: Error + Send + Sync + 'static;

    fn permission_state(
        &self,
    ) -> impl Future<Output = Result<NotificationPermissionStatus, Self::Error>> + Send;

    fn notify(
        &self,
        notification: SafeNotification,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn is_transient(error: &Self::Error) -> bool;
}

/// Counts from one bounded outbox pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeliverySweep {
    pub claimed: u32,
    pub delivered: u32,
    pub retrying: u32,
    pub failed: u32,
    pub paused: u32,
}

/// Lease-based system notification worker.
#[derive(Clone)]
pub struct DeliveryWorker<N> {
    store: AccountSettingsStore,
    notifier: N,
    worker_id: String,
}

impl<N: SystemNotifier> DeliveryWorker<N> {
    #[must_use]
    pub fn new(store: AccountSettingsStore, notifier: N, worker_id: impl Into<String>) -> Self {
        Self {
            store,
            notifier,
            worker_id: worker_id.into(),
        }
    }

    /// Delivers one bounded batch without ever requesting permission itself.
    ///
    /// # Errors
    ///
    /// Returns only persistence failures. Permission and adapter failures are
    /// recorded on the corresponding delivery and remain visible in-app.
    pub async fn deliver_pending(
        &self,
        now_unix_ms: i64,
    ) -> Result<DeliverySweep, SettingsStoreError> {
        let permission = match self.notifier.permission_state().await {
            Ok(permission) => permission,
            Err(_) => NotificationPermissionStatus::Error,
        };
        self.store
            .set_notification_permission_status(permission, now_unix_ms)
            .await?;
        if permission != NotificationPermissionStatus::Granted {
            let paused = self
                .store
                .pause_system_deliveries(permission, now_unix_ms)
                .await?;
            return Ok(DeliverySweep {
                paused,
                ..DeliverySweep::default()
            });
        }
        self.store.resume_permission_deliveries(now_unix_ms).await?;
        let deliveries = self
            .store
            .claim_system_deliveries(&self.worker_id, now_unix_ms, 60_000, 16)
            .await?;
        let mut sweep = DeliverySweep {
            claimed: u32::try_from(deliveries.len()).unwrap_or(u32::MAX),
            ..DeliverySweep::default()
        };
        for delivery in deliveries {
            let notification = render_notification(&delivery);
            let started = std::time::Instant::now();
            match self.notifier.notify(notification).await {
                Ok(()) => {
                    let delivered = self
                        .store
                        .complete_system_delivery(
                            delivery.id,
                            &self.worker_id,
                            now_unix_ms,
                            elapsed_ms(started),
                        )
                        .await?;
                    if delivered {
                        sweep.delivered = sweep.delivered.saturating_add(1);
                    } else {
                        sweep.failed = sweep.failed.saturating_add(1);
                    }
                }
                Err(error) => {
                    let transient = N::is_transient(&error);
                    self.store
                        .fail_system_delivery(
                            delivery.id,
                            &self.worker_id,
                            now_unix_ms,
                            elapsed_ms(started),
                            transient,
                        )
                        .await?;
                    if transient {
                        sweep.retrying = sweep.retrying.saturating_add(1);
                    } else {
                        sweep.failed = sweep.failed.saturating_add(1);
                    }
                }
            }
        }
        Ok(sweep)
    }
}

pub(crate) struct ClaimedSystemDelivery {
    pub id: i64,
    pub delivery_key: String,
    pub event_kind: AlertEventKind,
    pub target: AlertTarget,
    pub interface_locale: InterfaceLocalePreference,
}

fn render_notification(delivery: &ClaimedSystemDelivery) -> SafeNotification {
    let (title, body) = match delivery.interface_locale {
        InterfaceLocalePreference::ZhCn => (
            "QuotaTide 提醒",
            match delivery.event_kind {
                AlertEventKind::Daily80 => "今日使用已达到实际额度的 80%。",
                AlertEventKind::Daily100 => "今日使用已达到实际额度。",
                AlertEventKind::WeeklyRemaining20 => "本周额度剩余已降至 20%。",
                AlertEventKind::WeeklyRemaining10 => "本周额度剩余已降至 10%。",
                AlertEventKind::RadarChance70 => "第三方重置预测信号的置信度已达到 70% 档位。",
                AlertEventKind::QuotaResetConfirmed => "Codex 当前七日额度窗口已确认重置。",
                AlertEventKind::SourceFailures3 => "额度来源已连续采集失败 3 次。",
            },
        ),
        InterfaceLocalePreference::En | InterfaceLocalePreference::System => (
            "QuotaTide Alert",
            match delivery.event_kind {
                AlertEventKind::Daily80 => "Today's usage has reached 80% of its adjusted limit.",
                AlertEventKind::Daily100 => "Today's usage has reached its adjusted limit.",
                AlertEventKind::WeeklyRemaining20 => "Weekly quota remaining has fallen to 20%.",
                AlertEventKind::WeeklyRemaining10 => "Weekly quota remaining has fallen to 10%.",
                AlertEventKind::RadarChance70 => {
                    "The third-party reset prediction signal's confidence has reached the 70% tier."
                }
                AlertEventKind::QuotaResetConfirmed => {
                    "The current Codex seven-day quota window has reset."
                }
                AlertEventKind::SourceFailures3 => {
                    "A quota source has failed three consecutive refreshes."
                }
            },
        ),
    };
    SafeNotification {
        delivery_key: delivery.delivery_key.clone(),
        title: title.to_owned(),
        body: body.to_owned(),
        target: delivery.target,
    }
}

fn elapsed_ms(started: std::time::Instant) -> i64 {
    i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX)
}
