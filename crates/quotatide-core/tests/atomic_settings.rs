use std::collections::HashMap;
use std::convert::Infallible;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use quotatide_core::{
    AccountSettingsStore, AlertChannel, AlertEventKind, AlertPreferenceDraft,
    AtomicSettingsManager, AuthCandidateValidator, AutostartControl, CredentialVault,
    InterfaceLocalePreference, PublicError, QuotaPolicyDraft, SecretUpdate, SettingsDraft,
    SmtpCredentialStatus, SmtpRecipientDraft, SmtpSettingsDraft, SmtpTlsMode, StoryTheme,
    TrayDisplayMode, ValidatedAccountCandidate,
};
use secrecy::{ExposeSecret as _, SecretString};
use tempfile::tempdir;
use tokio::sync::Notify;

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

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("autostart unavailable")]
struct FakeAutostartError;

#[derive(Clone)]
struct FakeAutostart {
    enabled: Arc<AtomicBool>,
    fail_changes: Arc<AtomicBool>,
    changes: Arc<AtomicUsize>,
}

impl FakeAutostart {
    fn new(enabled: bool) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(enabled)),
            fail_changes: Arc::new(AtomicBool::new(false)),
            changes: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl AutostartControl for FakeAutostart {
    type Error = FakeAutostartError;

    async fn is_enabled(&self) -> Result<bool, Self::Error> {
        Ok(self.enabled.load(Ordering::SeqCst))
    }

    async fn set_enabled(&self, enabled: bool) -> Result<(), Self::Error> {
        self.changes.fetch_add(1, Ordering::SeqCst);
        if self.fail_changes.load(Ordering::SeqCst) {
            return Err(FakeAutostartError);
        }
        self.enabled.store(enabled, Ordering::SeqCst);
        Ok(())
    }
}

#[derive(Clone)]
struct BlockingAutostart {
    enabled: Arc<AtomicBool>,
    applied: Arc<Notify>,
    release: Arc<Notify>,
}

#[derive(Clone)]
struct ReadbackFailingAutostart {
    enabled: Arc<AtomicBool>,
    reads: Arc<AtomicUsize>,
}

impl AutostartControl for ReadbackFailingAutostart {
    type Error = FakeAutostartError;

    async fn is_enabled(&self) -> Result<bool, Self::Error> {
        if self.reads.fetch_add(1, Ordering::SeqCst) == 1 {
            Err(FakeAutostartError)
        } else {
            Ok(self.enabled.load(Ordering::SeqCst))
        }
    }

    async fn set_enabled(&self, enabled: bool) -> Result<(), Self::Error> {
        self.enabled.store(enabled, Ordering::SeqCst);
        Ok(())
    }
}

impl AutostartControl for BlockingAutostart {
    type Error = FakeAutostartError;

    async fn is_enabled(&self) -> Result<bool, Self::Error> {
        Ok(self.enabled.load(Ordering::SeqCst))
    }

    async fn set_enabled(&self, enabled: bool) -> Result<(), Self::Error> {
        self.enabled.store(enabled, Ordering::SeqCst);
        self.applied.notify_one();
        self.release.notified().await;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("credential vault unavailable")]
struct FakeVaultError;

#[derive(Clone, Default)]
struct FakeVault {
    values: Arc<Mutex<HashMap<&'static str, String>>>,
    fail_set: Arc<AtomicBool>,
    wrong_readback: Arc<AtomicBool>,
}

impl FakeVault {
    fn value(&self, slot: &'static str) -> Option<String> {
        self.values.lock().expect("vault lock").get(slot).cloned()
    }
}

impl CredentialVault for FakeVault {
    type Error = FakeVaultError;

    async fn get(&self, slot: &'static str) -> Result<Option<SecretString>, Self::Error> {
        let value = self.value(slot);
        Ok(value.map(|value| {
            if self.wrong_readback.load(Ordering::SeqCst) {
                SecretString::from("wrong-readback".to_owned())
            } else {
                SecretString::from(value)
            }
        }))
    }

    async fn set(&self, slot: &'static str, secret: SecretString) -> Result<(), Self::Error> {
        if self.fail_set.load(Ordering::SeqCst) {
            return Err(FakeVaultError);
        }
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

fn preferences() -> Vec<AlertPreferenceDraft> {
    AlertEventKind::ALL
        .into_iter()
        .flat_map(|event_kind| {
            AlertChannel::ALL
                .into_iter()
                .map(move |channel| AlertPreferenceDraft {
                    event_kind,
                    channel,
                    enabled: channel == AlertChannel::System,
                })
        })
        .collect()
}

fn draft(revision: u32, autostart_enabled: bool) -> SettingsDraft {
    let mut alert_preferences = preferences();
    alert_preferences
        .iter_mut()
        .find(|preference| {
            preference.event_kind == AlertEventKind::Daily80
                && preference.channel == AlertChannel::Email
        })
        .expect("daily email preference")
        .enabled = true;
    SettingsDraft {
        expected_settings_revision: revision,
        auth_path: Some("/chosen/auth.json".to_owned()),
        quota_policy: QuotaPolicyDraft {
            policy_timezone: "Asia/Shanghai".to_owned(),
            carry_workdays_enabled: true,
            base_micropoints: vec![
                16_000_000, 16_000_000, 16_000_000, 16_000_000, 16_000_000, 10_000_000, 10_000_000,
            ],
        },
        alert_preferences,
        autostart_enabled,
        auto_update_enabled: true,
        tray_display_mode: TrayDisplayMode::Wave,
        story_theme: StoryTheme::rising_water(),
        interface_locale: InterfaceLocalePreference::System,
        format_locale: "en-US".to_owned(),
        smtp: SmtpSettingsDraft {
            enabled: false,
            host: String::new(),
            port: 465,
            tls_mode: SmtpTlsMode::Tls,
            username: String::new(),
            from_address: String::new(),
            from_name: String::new(),
            recipients: Vec::new(),
        },
        smtp_password: SecretUpdate::Keep,
    }
}

fn smtp_draft(revision: u32, password: SecretUpdate) -> SettingsDraft {
    let mut value = draft(revision, false);
    value.smtp = SmtpSettingsDraft {
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
    };
    value.smtp_password = password;
    value
}

#[tokio::test]
async fn defaults_expose_every_non_secret_setting_and_channel_preference() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    let service = AtomicSettingsManager::new(store, ValidAuth, FakeAutostart::new(false));

    let settings = service.public_settings().await.expect("public settings");

    assert_eq!(settings.settings_revision, 0);
    assert!(!settings.configured);
    assert!(!settings.autostart_enabled);
    assert!(settings.auto_update_enabled);
    assert_eq!(settings.tray_display_mode, TrayDisplayMode::Wave);
    assert_eq!(settings.interface_locale, InterfaceLocalePreference::System);
    assert_eq!(settings.format_locale, "en");
    assert_eq!(settings.alert_preferences.len(), 14);
    assert!(
        settings.alert_preferences.iter().all(|preference| {
            preference.enabled == (preference.channel == AlertChannel::System)
        })
    );
}

#[tokio::test]
async fn tray_display_mode_is_revisioned_and_survives_restart() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open(&database)
        .await
        .expect("open store");
    let service = AtomicSettingsManager::new(store, ValidAuth, FakeAutostart::new(false));
    let mut countdown = draft(0, false);
    countdown.tray_display_mode = TrayDisplayMode::WaveResetCountdown;

    let saved = service
        .save_settings(countdown)
        .await
        .expect("save tray display mode");
    assert_eq!(saved.tray_display_mode, TrayDisplayMode::WaveResetCountdown);
    drop(service);

    let reopened = AccountSettingsStore::open(database)
        .await
        .expect("reopen store");
    let restarted = AtomicSettingsManager::new(reopened, ValidAuth, FakeAutostart::new(false));
    assert_eq!(
        restarted
            .public_settings()
            .await
            .expect("restarted settings")
            .tray_display_mode,
        TrayDisplayMode::WaveResetCountdown
    );
}

#[tokio::test]
async fn story_theme_is_revisioned_and_survives_restart() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open(&database)
        .await
        .expect("open store");
    let service = AtomicSettingsManager::new(store, ValidAuth, FakeAutostart::new(false));
    let mut siege = draft(0, false);
    let future_theme = StoryTheme::from_id("energy_core").expect("valid future theme id");
    siege.story_theme = future_theme.clone();

    let saved = service
        .save_settings(siege)
        .await
        .expect("save story theme");
    assert_eq!(saved.story_theme, future_theme);
    drop(service);

    let reopened = AccountSettingsStore::open(database)
        .await
        .expect("reopen store");
    let restarted = AtomicSettingsManager::new(reopened, ValidAuth, FakeAutostart::new(false));
    assert_eq!(
        restarted
            .public_settings()
            .await
            .expect("restarted settings")
            .story_theme,
        StoryTheme::from_id("energy_core").expect("valid future theme id")
    );
}

#[tokio::test]
async fn automatic_update_preference_is_revisioned_and_survives_restart() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open(&database)
        .await
        .expect("open store");
    let service = AtomicSettingsManager::new(store, ValidAuth, FakeAutostart::new(false));
    let mut disabled = draft(0, false);
    disabled.auto_update_enabled = false;

    let saved = service
        .save_settings(disabled)
        .await
        .expect("disable automatic updates");
    assert_eq!(saved.settings_revision, 1);
    assert!(!saved.auto_update_enabled);
    drop(service);

    let reopened = AccountSettingsStore::open(database)
        .await
        .expect("reopen store");
    let restarted = AtomicSettingsManager::new(reopened, ValidAuth, FakeAutostart::new(false));
    assert!(
        !restarted
            .public_settings()
            .await
            .expect("restarted settings")
            .auto_update_enabled
    );
}

#[tokio::test]
async fn one_save_commits_account_policy_preferences_and_confirmed_autostart() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    let autostart = FakeAutostart::new(false);
    let service = AtomicSettingsManager::new(store, ValidAuth, autostart.clone());

    let settings = service
        .save_settings(draft(0, true))
        .await
        .expect("save settings");

    assert_eq!(settings.settings_revision, 1);
    assert!(settings.configured);
    assert!(settings.autostart_enabled);
    assert_eq!(settings.interface_locale, InterfaceLocalePreference::System);
    assert_eq!(settings.format_locale, "en-US");
    assert!(autostart.enabled.load(Ordering::SeqCst));
    assert_eq!(autostart.changes.load(Ordering::SeqCst), 1);
    assert!(settings.alert_preferences.iter().any(|preference| {
        preference.event_kind == AlertEventKind::Daily80
            && preference.channel == AlertChannel::Email
            && preference.enabled
    }));
}

#[tokio::test]
async fn revision_conflict_and_autostart_failure_leave_the_old_settings_complete() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    let autostart = FakeAutostart::new(false);
    let service = AtomicSettingsManager::new(store, ValidAuth, autostart.clone());
    service
        .save_settings(draft(0, false))
        .await
        .expect("initial save");

    assert!(service.save_settings(draft(0, true)).await.is_err());
    assert_eq!(autostart.changes.load(Ordering::SeqCst), 0);

    autostart.fail_changes.store(true, Ordering::SeqCst);
    assert!(service.save_settings(draft(1, true)).await.is_err());
    let restored = service.public_settings().await.expect("restored settings");
    assert_eq!(restored.settings_revision, 1);
    assert!(!restored.autostart_enabled);
    assert!(!autostart.enabled.load(Ordering::SeqCst));
}

#[tokio::test]
async fn restart_recovers_a_prepared_external_change_interrupted_before_sqlite_commit() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    let enabled = Arc::new(AtomicBool::new(false));
    let applied = Arc::new(Notify::new());
    let interrupted = AtomicSettingsManager::new(
        store.clone(),
        ValidAuth,
        BlockingAutostart {
            enabled: enabled.clone(),
            applied: applied.clone(),
            release: Arc::new(Notify::new()),
        },
    );

    let save = tokio::spawn(async move { interrupted.save_settings(draft(0, true)).await });
    applied.notified().await;
    save.abort();
    let _ = save.await;
    assert!(enabled.load(Ordering::SeqCst));

    let healthy_autostart = FakeAutostart {
        enabled: enabled.clone(),
        fail_changes: Arc::new(AtomicBool::new(false)),
        changes: Arc::new(AtomicUsize::new(0)),
    };
    let restarted = AtomicSettingsManager::new(store, ValidAuth, healthy_autostart);
    restarted
        .recover_external_changes()
        .await
        .expect("recover interrupted save");

    assert!(!enabled.load(Ordering::SeqCst));
    let settings = restarted.public_settings().await.expect("old settings");
    assert_eq!(settings.settings_revision, 0);
    assert!(!settings.configured);
    assert!(!settings.autostart_enabled);
}

#[tokio::test]
async fn failed_autostart_readback_restores_the_old_external_and_sqlite_state() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    let enabled = Arc::new(AtomicBool::new(false));
    let service = AtomicSettingsManager::new(
        store,
        ValidAuth,
        ReadbackFailingAutostart {
            enabled: enabled.clone(),
            reads: Arc::new(AtomicUsize::new(0)),
        },
    );

    assert!(service.save_settings(draft(0, true)).await.is_err());
    assert!(!enabled.load(Ordering::SeqCst));
    let settings = service.public_settings().await.expect("old settings");
    assert_eq!(settings.settings_revision, 0);
    assert!(!settings.autostart_enabled);
}

#[tokio::test]
async fn invalid_complete_preferences_are_rejected_before_external_state_changes() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    let autostart = FakeAutostart::new(false);
    let service = AtomicSettingsManager::new(store, ValidAuth, autostart.clone());
    let mut invalid = draft(0, true);
    invalid.alert_preferences.pop();

    assert!(service.save_settings(invalid).await.is_err());
    assert_eq!(autostart.changes.load(Ordering::SeqCst), 0);
    let settings = service.public_settings().await.expect("old settings");
    assert_eq!(settings.settings_revision, 0);
    assert!(!settings.configured);
    assert!(!settings.autostart_enabled);
}

#[tokio::test]
async fn sqlite_commit_failure_rolls_back_external_state_and_every_setting() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open(&database)
        .await
        .expect("open store");
    {
        let connection =
            tokio_rusqlite::rusqlite::Connection::open(&database).expect("open fault injector");
        connection
            .execute_batch(
                "CREATE TRIGGER fail_atomic_settings_commit
                 BEFORE UPDATE ON app_settings
                 WHEN NEW.autostart_enabled = 1
                 BEGIN
                   SELECT RAISE(ABORT, 'injected atomic settings failure');
                 END;",
            )
            .expect("install fault injector");
    }
    let autostart = FakeAutostart::new(false);
    let service = AtomicSettingsManager::new(store, ValidAuth, autostart.clone());

    assert!(service.save_settings(draft(0, true)).await.is_err());
    assert!(!autostart.enabled.load(Ordering::SeqCst));
    let settings = service.public_settings().await.expect("old settings");
    assert_eq!(settings.settings_revision, 0);
    assert!(!settings.configured);
    assert!(!settings.autostart_enabled);
    assert!(
        settings
            .alert_preferences
            .iter()
            .filter(|preference| preference.channel == AlertChannel::Email)
            .all(|preference| !preference.enabled)
    );
}

#[tokio::test]
async fn restart_reapplies_a_committed_external_change_before_cleaning_its_journal() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open(&database)
        .await
        .expect("open store");
    {
        let connection =
            tokio_rusqlite::rusqlite::Connection::open(&database).expect("open crash fixture");
        connection
            .execute_batch(
                "BEGIN IMMEDIATE;
                 UPDATE app_settings SET autostart_enabled = 1 WHERE singleton_id = 1;
                 UPDATE app_meta SET settings_revision = 1 WHERE singleton_id = 1;
                 INSERT INTO external_change_journal
                   (operation_key, kind, phase, old_credential_ref,
                    new_credential_ref, old_autostart_enabled,
                    new_autostart_enabled, created_at_ms, updated_at_ms)
                 VALUES
                   ('committed-crash', 'settings', 'committed', NULL, NULL,
                    0, 1, 1785000000000, 1785000000000);
                 COMMIT;",
            )
            .expect("seed committed crash point");
    }
    let autostart = FakeAutostart::new(false);
    let restarted = AtomicSettingsManager::new(store, ValidAuth, autostart.clone());

    restarted
        .recover_external_changes()
        .await
        .expect("reapply committed change");
    assert!(autostart.enabled.load(Ordering::SeqCst));
    assert_eq!(autostart.changes.load(Ordering::SeqCst), 1);
    restarted
        .recover_external_changes()
        .await
        .expect("journal was cleaned");
    assert_eq!(autostart.changes.load(Ordering::SeqCst), 1);
    let settings = restarted.public_settings().await.expect("new settings");
    assert_eq!(settings.settings_revision, 1);
    assert!(settings.autostart_enabled);
}

#[tokio::test]
async fn smtp_secret_rotates_between_slots_and_never_enters_sqlite() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open(&database)
        .await
        .expect("open store");
    let vault = FakeVault::default();
    let service = AtomicSettingsManager::new(store, ValidAuth, FakeAutostart::new(false))
        .with_credential_vault(vault.clone());

    let first = service
        .save_settings(smtp_draft(
            0,
            SecretUpdate::Set("first-secret-canary".to_owned()),
        ))
        .await
        .expect("save first secret");
    assert_eq!(
        first.smtp.credential_status,
        SmtpCredentialStatus::Configured
    );
    assert_eq!(
        vault.value("slot-a").as_deref(),
        Some("first-secret-canary")
    );

    let second = service
        .save_settings(smtp_draft(
            1,
            SecretUpdate::Set("second-secret-canary".to_owned()),
        ))
        .await
        .expect("rotate secret");
    assert_eq!(second.settings_revision, 2);
    assert!(vault.value("slot-a").is_none());
    assert_eq!(
        vault.value("slot-b").as_deref(),
        Some("second-secret-canary")
    );

    let bytes = std::fs::read(database).expect("read sqlite bytes");
    assert!(
        !bytes
            .windows("first-secret-canary".len())
            .any(|window| { window == "first-secret-canary".as_bytes() })
    );
    assert!(
        !bytes
            .windows("second-secret-canary".len())
            .any(|window| { window == "second-secret-canary".as_bytes() })
    );
}

#[tokio::test]
async fn smtp_readback_failure_preserves_the_old_revision_and_removes_staged_secret() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    let vault = FakeVault::default();
    vault.wrong_readback.store(true, Ordering::SeqCst);
    let service = AtomicSettingsManager::new(store, ValidAuth, FakeAutostart::new(false))
        .with_credential_vault(vault.clone());

    assert!(
        service
            .save_settings(smtp_draft(0, SecretUpdate::Set("staged-secret".to_owned())))
            .await
            .is_err()
    );
    assert!(vault.value("slot-a").is_none());
    let settings = service.public_settings().await.expect("old settings");
    assert_eq!(settings.settings_revision, 0);
    assert!(!settings.smtp.enabled);
}

#[tokio::test]
async fn explicit_smtp_secret_delete_clears_the_slot_and_pauses_email_configuration() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    let vault = FakeVault::default();
    let service = AtomicSettingsManager::new(store, ValidAuth, FakeAutostart::new(false))
        .with_credential_vault(vault.clone());
    service
        .save_settings(smtp_draft(0, SecretUpdate::Set("delete-me".to_owned())))
        .await
        .expect("configure SMTP");

    let deleted = service
        .save_settings(smtp_draft(1, SecretUpdate::Delete))
        .await
        .expect("delete SMTP secret");

    assert!(vault.value("slot-a").is_none());
    assert_eq!(
        deleted.smtp.credential_status,
        SmtpCredentialStatus::Missing
    );
}
