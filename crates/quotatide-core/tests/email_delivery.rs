use std::collections::HashMap;
use std::convert::Infallible;
use std::path::Path;
use std::sync::{Arc, Mutex};

use quotatide_core::{
    AccountSettingsStore, AlertChannel, AlertEventKind, AlertPreferenceDraft,
    AtomicSettingsManager, AuthCandidateValidator, AutostartControl, CredentialVault,
    EmailDeliveryWorker, InterfaceLocalePreference, MailTransport, PublicError, QuotaPolicyDraft,
    QuotaUnits, RefreshAccountBinding, SafeMail, SecretUpdate, SettingsDraft, SmtpConnection,
    SmtpRecipientDraft, SmtpSettingsDraft, SmtpTlsMode, ValidatedAccountCandidate,
    WeeklyUsageObservation,
};
use secrecy::{ExposeSecret as _, SecretString};
use tempfile::tempdir;

#[derive(Clone)]
struct ValidAuth;

impl AuthCandidateValidator for ValidAuth {
    type Error = Infallible;

    fn validate(&self, path: &Path) -> Result<ValidatedAccountCandidate, Self::Error> {
        Ok(ValidatedAccountCandidate::new(
            path.to_string_lossy().into_owned(),
            "account-one".to_owned(),
        ))
    }

    fn public_error(error: &Self::Error) -> PublicError {
        match *error {}
    }
}

#[derive(Clone, Copy)]
struct DisabledAutostart;

impl AutostartControl for DisabledAutostart {
    type Error = Infallible;

    async fn is_enabled(&self) -> Result<bool, Self::Error> {
        Ok(false)
    }

    async fn set_enabled(&self, _enabled: bool) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Clone, Default)]
struct MemoryVault {
    values: Arc<Mutex<HashMap<&'static str, String>>>,
}

impl MemoryVault {
    fn clear(&self) {
        self.values.lock().expect("vault lock").clear();
    }
}

impl CredentialVault for MemoryVault {
    type Error = Infallible;

    async fn get(&self, slot: &'static str) -> Result<Option<SecretString>, Self::Error> {
        Ok(self
            .values
            .lock()
            .expect("vault lock")
            .get(slot)
            .cloned()
            .map(SecretString::from))
    }

    async fn set(&self, slot: &'static str, secret: SecretString) -> Result<(), Self::Error> {
        self.values
            .lock()
            .expect("vault lock")
            .insert(slot, secret.expose_secret().to_owned());
        Ok(())
    }

    async fn delete(&self, slot: &'static str) -> Result<(), Self::Error> {
        self.values.lock().expect("vault lock").remove(slot);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("recording SMTP failure")]
struct RecordingMailError {
    transient: bool,
}

#[derive(Clone, Default)]
struct RecordingMailTransport {
    sent: Arc<Mutex<Vec<String>>>,
    fail_recipient: Arc<Mutex<Option<(String, bool)>>>,
}

impl RecordingMailTransport {
    fn fail(&self, recipient: &str, transient: bool) {
        *self.fail_recipient.lock().expect("failure lock") =
            Some((recipient.to_owned(), transient));
    }
}

impl MailTransport for RecordingMailTransport {
    type Error = RecordingMailError;

    async fn send(
        &self,
        _connection: SmtpConnection,
        _password: SecretString,
        mail: SafeMail,
    ) -> Result<(), Self::Error> {
        let failure = self.fail_recipient.lock().expect("failure lock").clone();
        if let Some((recipient, transient)) = failure
            && recipient == mail.recipient
        {
            return Err(RecordingMailError { transient });
        }
        self.sent.lock().expect("sent lock").push(mail.recipient);
        Ok(())
    }

    fn is_transient(error: &Self::Error) -> bool {
        error.transient
    }
}

fn preferences() -> Vec<AlertPreferenceDraft> {
    AlertEventKind::ALL
        .into_iter()
        .flat_map(|event_kind| {
            AlertChannel::ALL
                .into_iter()
                .map(move |channel| AlertPreferenceDraft {
                    event_kind,
                    channel,
                    enabled: channel == AlertChannel::System
                        || (channel == AlertChannel::Email
                            && event_kind == AlertEventKind::Daily80),
                })
        })
        .collect()
}

fn settings_draft() -> SettingsDraft {
    SettingsDraft {
        expected_settings_revision: 0,
        auth_path: Some("/chosen/auth.json".to_owned()),
        quota_policy: QuotaPolicyDraft {
            policy_timezone: "Asia/Shanghai".to_owned(),
            carry_workdays_enabled: true,
            base_micropoints: vec![
                16_000_000, 16_000_000, 16_000_000, 16_000_000, 16_000_000, 10_000_000, 10_000_000,
            ],
        },
        alert_preferences: preferences(),
        autostart_enabled: false,
        auto_update_enabled: true,
        interface_locale: InterfaceLocalePreference::System,
        format_locale: "en-US".to_owned(),
        smtp: SmtpSettingsDraft {
            enabled: true,
            host: "smtp.example.com".to_owned(),
            port: 587,
            tls_mode: SmtpTlsMode::Starttls,
            username: "sender@example.com".to_owned(),
            from_address: "sender@example.com".to_owned(),
            from_name: "QuotaTide".to_owned(),
            recipients: vec![
                SmtpRecipientDraft {
                    address: "first@example.com".to_owned(),
                    enabled: true,
                },
                SmtpRecipientDraft {
                    address: "second@example.com".to_owned(),
                    enabled: true,
                },
            ],
        },
        smtp_password: SecretUpdate::Set("smtp-secret-canary".to_owned()),
    }
}

fn timestamp_ms(value: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(value)
        .expect("RFC 3339 timestamp")
        .timestamp_millis()
}

fn observation(captured_at_unix_ms: i64, used_micropoints: i64) -> WeeklyUsageObservation {
    WeeklyUsageObservation {
        captured_at_unix_ms,
        used: QuotaUnits::from_micropoints(used_micropoints).expect("valid quota"),
        window_seconds: 604_800,
        resets_at_unix_s: timestamp_ms("2026-08-03T00:00:00Z") / 1000,
        plan_type: Some("plus".to_owned()),
        allowed: Some(true),
    }
}

async fn configured_store(
    database: &Path,
) -> (AccountSettingsStore, MemoryVault, RecordingMailTransport) {
    let store = AccountSettingsStore::open_with_policy_timezone(database, "Asia/Shanghai")
        .await
        .expect("open store");
    let vault = MemoryVault::default();
    AtomicSettingsManager::new(store.clone(), ValidAuth, DisabledAutostart)
        .with_credential_vault(vault.clone())
        .save_settings(settings_draft())
        .await
        .expect("configure SMTP");
    let binding = RefreshAccountBinding::selected(1, "/chosen/auth.json".into())
        .with_account_id("account-one".to_owned());
    for (captured, used) in [
        ("2026-07-30T01:00:00Z", 0),
        ("2026-07-30T02:00:00Z", 13_000_000),
    ] {
        store
            .record_usage_success(&binding, observation(timestamp_ms(captured), used))
            .await
            .expect("seed daily warning");
    }
    (store, vault, RecordingMailTransport::default())
}

fn delivery_states(database: &Path) -> Vec<(String, String, i64)> {
    let connection =
        tokio_rusqlite::rusqlite::Connection::open(database).expect("inspect deliveries");
    connection
        .prepare(
            "SELECT channel, state, COUNT(*)
             FROM alert_deliveries GROUP BY channel, state ORDER BY channel, state",
        )
        .expect("prepare state query")
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .expect("query states")
        .collect::<Result<_, _>>()
        .expect("collect states")
}

#[tokio::test]
async fn email_deliveries_are_recipient_scoped_with_independent_retry_state() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let (store, vault, transport) = configured_store(&database).await;
    transport.fail("second@example.com", true);
    let worker = EmailDeliveryWorker::new(store, vault, transport.clone(), "email-test");

    let sweep = worker
        .deliver_pending(timestamp_ms("2026-07-30T02:01:00Z"))
        .await
        .expect("deliver batch");

    assert_eq!(sweep.claimed, 2);
    assert_eq!(sweep.delivered, 1);
    assert_eq!(sweep.retrying, 1);
    assert_eq!(
        transport.sent.lock().expect("sent lock").as_slice(),
        ["first@example.com"]
    );
    assert_eq!(
        delivery_states(&database),
        vec![
            ("email".to_owned(), "delivered".to_owned(), 1),
            ("email".to_owned(), "retry_wait".to_owned(), 1),
            ("system".to_owned(), "pending".to_owned(), 1),
        ]
    );
}

#[tokio::test]
async fn missing_credential_pauses_only_email_and_never_attempts_smtp() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let (store, vault, transport) = configured_store(&database).await;
    vault.clear();
    let worker = EmailDeliveryWorker::new(store, vault, transport.clone(), "email-test");

    let sweep = worker
        .deliver_pending(timestamp_ms("2026-07-30T02:01:00Z"))
        .await
        .expect("pause batch");

    assert_eq!(sweep.paused, 2);
    assert!(transport.sent.lock().expect("sent lock").is_empty());
    assert_eq!(
        delivery_states(&database),
        vec![
            ("email".to_owned(), "paused_config".to_owned(), 2),
            ("system".to_owned(), "pending".to_owned(), 1),
        ]
    );
}

#[tokio::test]
async fn explicit_test_email_skips_the_outbox_and_reports_enabled_recipient_count() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let (store, vault, transport) = configured_store(&database).await;
    let before = delivery_states(&database);
    let worker = EmailDeliveryWorker::new(store, vault, transport.clone(), "email-test");

    assert_eq!(worker.send_test_email().await.expect("send test"), 2);
    assert_eq!(delivery_states(&database), before);
    assert_eq!(
        transport.sent.lock().expect("sent lock").as_slice(),
        ["first@example.com", "second@example.com"]
    );
    let bytes = std::fs::read(database).expect("read SQLite");
    assert!(
        !bytes
            .windows("smtp-secret-canary".len())
            .any(|window| window == "smtp-secret-canary".as_bytes())
    );
}
