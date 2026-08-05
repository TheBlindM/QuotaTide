use std::error::Error;
use std::future::Future;
use std::time::Instant;

use secrecy::SecretString;

use crate::account_settings::{
    AccountSettingsStore, ClaimedEmailDelivery, CredentialVault, SettingsStoreError,
};
use crate::{AlertEventKind, DeliverySweep, InterfaceLocalePreference, SmtpTlsMode};

/// Stable, secret-free result categories for the explicit SMTP test action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TestEmailError {
    #[error("SMTP is not completely configured")]
    NotConfigured,
    #[error("SMTP credential is missing")]
    CredentialMissing,
    #[error("SMTP credential store is unavailable")]
    CredentialUnavailable,
    #[error("SMTP test delivery failed")]
    DeliveryFailed,
    #[error("settings storage is unavailable")]
    StorageUnavailable,
}

/// Connection data passed only to the trusted Rust SMTP adapter.
#[derive(Clone, PartialEq, Eq)]
pub struct SmtpConnection {
    pub host: String,
    pub port: u16,
    pub tls_mode: SmtpTlsMode,
    pub username: String,
    pub from_address: String,
    pub from_name: String,
}

/// One recipient-scoped message. It is intentionally neither serializable nor
/// debuggable because email addresses are private local configuration.
#[derive(Clone, PartialEq, Eq)]
pub struct SafeMail {
    pub delivery_key: String,
    pub recipient: String,
    pub subject: String,
    pub body: String,
}

/// Narrow SMTP boundary. Implementations must require TLS and return only typed
/// errors; raw server responses never cross into the core.
pub trait MailTransport: Clone + Send + Sync + 'static {
    type Error: Error + Send + Sync + 'static;

    fn send(
        &self,
        connection: SmtpConnection,
        password: SecretString,
        mail: SafeMail,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn is_transient(error: &Self::Error) -> bool;
}

/// Lease-based, recipient-scoped email delivery worker.
#[derive(Clone)]
pub struct EmailDeliveryWorker<V, M> {
    store: AccountSettingsStore,
    vault: V,
    transport: M,
    worker_id: String,
}

impl<V: CredentialVault, M: MailTransport> EmailDeliveryWorker<V, M> {
    #[must_use]
    pub fn new(
        store: AccountSettingsStore,
        vault: V,
        transport: M,
        worker_id: impl Into<String>,
    ) -> Self {
        Self {
            store,
            vault,
            transport,
            worker_id: worker_id.into(),
        }
    }

    /// Delivers one bounded batch and records each recipient independently.
    ///
    /// # Errors
    ///
    /// Returns only persistence failures. Vault and SMTP failures are converted
    /// into durable, secret-free delivery states.
    pub async fn deliver_pending(
        &self,
        now_unix_ms: i64,
    ) -> Result<DeliverySweep, SettingsStoreError> {
        let Some(configuration) = self.store.smtp_delivery_configuration().await? else {
            let paused = self
                .store
                .pause_email_deliveries("email_not_configured", now_unix_ms)
                .await?;
            return Ok(DeliverySweep {
                paused,
                ..DeliverySweep::default()
            });
        };
        let password = match self.vault.get(configuration.credential_slot).await {
            Ok(Some(secret)) => secret,
            Ok(None) => {
                let paused = self
                    .store
                    .pause_email_deliveries("email_credential_missing", now_unix_ms)
                    .await?;
                return Ok(DeliverySweep {
                    paused,
                    ..DeliverySweep::default()
                });
            }
            Err(_) => {
                let paused = self
                    .store
                    .pause_email_deliveries("email_credential_unavailable", now_unix_ms)
                    .await?;
                return Ok(DeliverySweep {
                    paused,
                    ..DeliverySweep::default()
                });
            }
        };
        self.store.resume_email_deliveries(now_unix_ms).await?;
        let deliveries = self
            .store
            .claim_email_deliveries(&self.worker_id, now_unix_ms, 60_000, 16)
            .await?;
        let mut sweep = DeliverySweep {
            claimed: u32::try_from(deliveries.len()).unwrap_or(u32::MAX),
            ..DeliverySweep::default()
        };
        for delivery in deliveries {
            let mail = render_mail(&delivery);
            let started = Instant::now();
            match self
                .transport
                .send(configuration.connection.clone(), password.clone(), mail)
                .await
            {
                Ok(()) => {
                    if self
                        .store
                        .complete_email_delivery(
                            delivery.id,
                            &self.worker_id,
                            now_unix_ms,
                            elapsed_ms(started),
                        )
                        .await?
                    {
                        sweep.delivered = sweep.delivered.saturating_add(1);
                    }
                }
                Err(error) => {
                    let transient = M::is_transient(&error);
                    self.store
                        .fail_email_delivery(
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

    /// Sends one explicit test message to every enabled recipient without
    /// creating or changing outbox rows.
    ///
    /// # Errors
    ///
    /// Returns only stable categories that contain no credential, address, or
    /// upstream server response.
    pub async fn send_test_email(&self) -> Result<u32, TestEmailError> {
        let configuration = self
            .store
            .smtp_delivery_configuration()
            .await
            .map_err(|_| TestEmailError::StorageUnavailable)?
            .filter(|configuration| !configuration.recipients.is_empty())
            .ok_or(TestEmailError::NotConfigured)?;
        let password = self
            .vault
            .get(configuration.credential_slot)
            .await
            .map_err(|_| TestEmailError::CredentialUnavailable)?
            .ok_or(TestEmailError::CredentialMissing)?;
        let mut sent = 0_u32;
        let (subject, body) = match configuration.interface_locale {
            InterfaceLocalePreference::ZhCn => (
                "QuotaTide：测试邮件",
                "这是一封 QuotaTide 测试邮件。SMTP 与收件地址配置可用。",
            ),
            InterfaceLocalePreference::En | InterfaceLocalePreference::System => (
                "QuotaTide: Test email",
                "This is a QuotaTide test email. Your SMTP and recipient settings work.",
            ),
        };
        for (index, recipient) in configuration.recipients.into_iter().enumerate() {
            let mail = SafeMail {
                delivery_key: format!("smtp-test-{}-{index}", unix_time_ms()),
                recipient,
                subject: subject.to_owned(),
                body: body.to_owned(),
            };
            self.transport
                .send(configuration.connection.clone(), password.clone(), mail)
                .await
                .map_err(|_| TestEmailError::DeliveryFailed)?;
            sent = sent.saturating_add(1);
        }
        Ok(sent)
    }
}

fn render_mail(delivery: &ClaimedEmailDelivery) -> SafeMail {
    let (subject, body) = match delivery.interface_locale {
        InterfaceLocalePreference::ZhCn => match delivery.event_kind {
            AlertEventKind::Daily80 => (
                "QuotaTide：今日额度接近上限",
                "今日额度已达到动态上限的 80%。请打开 QuotaTide 查看当前七日窗口。",
            ),
            AlertEventKind::Daily100 => (
                "QuotaTide：今日额度已用完",
                "今日额度已达到动态上限。请打开 QuotaTide 查看完整记录。",
            ),
            AlertEventKind::WeeklyRemaining20 => (
                "QuotaTide：本周额度仅剩 20%",
                "当前七日窗口剩余额度已进入注意区间。",
            ),
            AlertEventKind::WeeklyRemaining10 => {
                ("QuotaTide：本周额度仅剩 10%", "当前七日窗口已接近耗尽。")
            }
            AlertEventKind::RadarChance70 => (
                "QuotaTide：重置预测提醒",
                "第三方公开信号的重置预测置信度已达到 70% 档位；这不是 OpenAI 承诺。",
            ),
            AlertEventKind::QuotaResetConfirmed => (
                "QuotaTide：额度重置已确认",
                "本机连续观测已确认当前账号进入新的额度窗口。",
            ),
            AlertEventKind::SourceFailures3 => (
                "QuotaTide：额度来源连续失败",
                "额度或 Reset Radar 来源已连续采集失败 3 次，请打开 QuotaTide 查看安全状态。",
            ),
        },
        InterfaceLocalePreference::En | InterfaceLocalePreference::System => {
            match delivery.event_kind {
                AlertEventKind::Daily80 => (
                    "QuotaTide: Today's quota is close to its limit",
                    "Today's usage has reached 80% of its adjusted limit. Open QuotaTide to view the current seven-day window.",
                ),
                AlertEventKind::Daily100 => (
                    "QuotaTide: Today's quota is exhausted",
                    "Today's usage has reached its adjusted limit. Open QuotaTide to view the full record.",
                ),
                AlertEventKind::WeeklyRemaining20 => (
                    "QuotaTide: 20% weekly quota remaining",
                    "The current seven-day window has entered the caution range.",
                ),
                AlertEventKind::WeeklyRemaining10 => (
                    "QuotaTide: 10% weekly quota remaining",
                    "The current seven-day window is nearly exhausted.",
                ),
                AlertEventKind::RadarChance70 => (
                    "QuotaTide: reset prediction",
                    "A third-party public signal's reset prediction confidence has reached the 70% tier; this is not an OpenAI commitment.",
                ),
                AlertEventKind::QuotaResetConfirmed => (
                    "QuotaTide: Quota reset confirmed",
                    "Consecutive local observations confirm that this account entered a new quota window.",
                ),
                AlertEventKind::SourceFailures3 => (
                    "QuotaTide: Quota source keeps failing",
                    "A quota or Reset Radar source has failed three consecutive refreshes. Open QuotaTide to review its safe status.",
                ),
            }
        }
    };
    SafeMail {
        delivery_key: delivery.delivery_key.clone(),
        recipient: delivery.recipient.clone(),
        subject: subject.to_owned(),
        body: body.to_owned(),
    }
}

fn elapsed_ms(started: Instant) -> i64 {
    i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX)
}

fn unix_time_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or_default()
}
