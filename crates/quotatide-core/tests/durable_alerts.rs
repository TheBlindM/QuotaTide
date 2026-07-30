use std::future::Future;
use std::sync::{Arc, Mutex};

use quotatide_core::{
    AccountSettingsStore, DeliveryWorker, NotificationPermissionStatus, QuotaUnits,
    RadarObservation, RadarSnapshot, RadarSourceErrorCode, RefreshAccountBinding, SafeNotification,
    SystemNotifier, UsageSourceErrorCode, WeeklyUsageObservation,
};
use tempfile::tempdir;
use tokio::sync::Notify;

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

fn binding() -> RefreshAccountBinding {
    RefreshAccountBinding::selected(1, "/chosen/auth.json".into())
        .with_account_id("account-one".to_owned())
}

fn event_kinds(database: &std::path::Path) -> Vec<(String, Option<String>)> {
    let connection =
        tokio_rusqlite::rusqlite::Connection::open(database).expect("inspect alert events");
    connection
        .prepare("SELECT event_kind, source FROM alert_events ORDER BY created_at_ms, id")
        .expect("prepare event query")
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("query events")
        .collect::<Result<_, _>>()
        .expect("collect events")
}

#[derive(Debug, thiserror::Error)]
#[error("recording notifier failure")]
struct RecordingNotifierError;

#[derive(Clone)]
struct RecordingNotifier {
    permission: NotificationPermissionStatus,
    sent: Arc<Mutex<Vec<SafeNotification>>>,
    fail: bool,
}

impl RecordingNotifier {
    fn granted() -> Self {
        Self {
            permission: NotificationPermissionStatus::Granted,
            sent: Arc::new(Mutex::new(Vec::new())),
            fail: false,
        }
    }
}

impl SystemNotifier for RecordingNotifier {
    type Error = RecordingNotifierError;

    fn permission_state(
        &self,
    ) -> impl Future<Output = Result<NotificationPermissionStatus, Self::Error>> + Send {
        std::future::ready(Ok(self.permission))
    }

    fn notify(
        &self,
        notification: SafeNotification,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        let sent = Arc::clone(&self.sent);
        let fail = self.fail;
        async move {
            if fail {
                Err(RecordingNotifierError)
            } else {
                sent.lock()
                    .expect("notification recorder")
                    .push(notification);
                Ok(())
            }
        }
    }

    fn is_transient(_error: &Self::Error) -> bool {
        true
    }
}

#[derive(Clone)]
struct BlockingNotifier {
    entered: Arc<Notify>,
    release: Arc<Notify>,
}

impl SystemNotifier for BlockingNotifier {
    type Error = RecordingNotifierError;

    fn permission_state(
        &self,
    ) -> impl Future<Output = Result<NotificationPermissionStatus, Self::Error>> + Send {
        std::future::ready(Ok(NotificationPermissionStatus::Granted))
    }

    fn notify(
        &self,
        _notification: SafeNotification,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        let entered = Arc::clone(&self.entered);
        let release = Arc::clone(&self.release);
        async move {
            entered.notify_one();
            release.notified().await;
            Ok(())
        }
    }

    fn is_transient(_error: &Self::Error) -> bool {
        true
    }
}

async fn seed_daily_warning(store: &AccountSettingsStore) {
    for (captured, used) in [
        ("2026-07-30T01:00:00Z", 0),
        ("2026-07-30T02:00:00Z", 13_000_000),
    ] {
        store
            .record_usage_success(&binding(), observation(timestamp_ms(captured), used))
            .await
            .expect("seed daily warning");
    }
}

#[tokio::test]
async fn daily_crossings_create_one_event_and_system_delivery_in_the_refresh_transaction() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open_with_policy_timezone(&database, "Asia/Shanghai")
        .await
        .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");

    for (captured, used) in [
        ("2026-07-30T01:00:00Z", 0),
        ("2026-07-30T02:00:00Z", 13_000_000),
        ("2026-07-30T03:00:00Z", 14_000_000),
        ("2026-07-30T04:00:00Z", 17_000_000),
        ("2026-07-30T05:00:00Z", 18_000_000),
    ] {
        store
            .record_usage_success(&binding(), observation(timestamp_ms(captured), used))
            .await
            .expect("record threshold observation");
    }
    drop(store);

    let connection =
        tokio_rusqlite::rusqlite::Connection::open(database).expect("inspect alert outbox");
    let events = connection
        .prepare(
            "SELECT event_kind, local_date, threshold_micropoints
             FROM alert_events ORDER BY created_at_ms",
        )
        .expect("prepare events")
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        })
        .expect("query events")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect events");
    let deliveries: Vec<(String, String, i64)> = connection
        .prepare(
            "SELECT channel, state, COUNT(*)
             FROM alert_deliveries GROUP BY channel, state ORDER BY channel, state",
        )
        .expect("prepare deliveries")
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .expect("query deliveries")
        .collect::<Result<_, _>>()
        .expect("collect deliveries");

    assert_eq!(
        events,
        vec![
            (
                "daily_80".to_owned(),
                Some("2026-07-30".to_owned()),
                Some(12_800_000),
            ),
            (
                "daily_100".to_owned(),
                Some("2026-07-30".to_owned()),
                Some(16_000_000),
            ),
        ]
    );
    assert_eq!(
        deliveries,
        vec![("system".to_owned(), "pending".to_owned(), 2)]
    );
}

#[tokio::test]
async fn weekly_crossings_and_a_confirmed_epoch_are_stable_once_per_epoch() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open(&database)
        .await
        .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");

    for (captured, used, reset) in [
        (1_700_600_000_000, 70_000_000, 1_700_604_800),
        (1_700_601_000_000, 81_000_000, 1_700_604_800),
        (1_700_602_000_000, 91_000_000, 1_700_604_800),
        (1_700_605_000_000, 2_000_000, 1_701_209_600),
        (1_700_606_000_000, 3_000_000, 1_701_209_600),
    ] {
        store
            .record_usage_success(
                &binding(),
                WeeklyUsageObservation {
                    captured_at_unix_ms: captured,
                    used: QuotaUnits::from_micropoints(used).expect("valid quota"),
                    window_seconds: 604_800,
                    resets_at_unix_s: reset,
                    plan_type: Some("plus".to_owned()),
                    allowed: Some(true),
                },
            )
            .await
            .expect("record weekly transition");
    }
    drop(store);

    let weekly_and_reset = event_kinds(&database)
        .into_iter()
        .filter(|(kind, _)| {
            matches!(
                kind.as_str(),
                "weekly_remaining_20" | "weekly_remaining_10" | "quota_reset_confirmed"
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        weekly_and_reset,
        vec![
            ("weekly_remaining_20".to_owned(), None),
            ("weekly_remaining_10".to_owned(), None),
            ("quota_reset_confirmed".to_owned(), None),
        ]
    );
}

#[tokio::test]
async fn radar_threshold_and_each_sources_third_failure_are_deduplicated() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open(&database)
        .await
        .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");
    let radar = RadarObservation::new(
        "2081899343091843463",
        7_500,
        1_785_000_000_000,
        1_785_086_400_000,
        "Possible additional reset",
        "https://x.com/thsottiaux/status/2081899343091843463",
    )
    .expect("radar observation");
    for attempted_at in [1_785_000_000_001, 1_785_000_000_002] {
        store
            .record_radar_success(attempted_at, RadarSnapshot::new(Some(radar.clone()), None))
            .await
            .expect("record radar");
    }
    for offset in 1..=4 {
        store
            .record_usage_failure(
                &binding(),
                1_785_000_100_000 + offset,
                UsageSourceErrorCode::Timeout,
            )
            .await
            .expect("record Codex failure");
        store
            .record_radar_failure(1_785_000_200_000 + offset, RadarSourceErrorCode::Timeout)
            .await
            .expect("record Radar failure");
    }
    drop(store);

    let relevant = event_kinds(&database)
        .into_iter()
        .filter(|(kind, _)| matches!(kind.as_str(), "radar_chance_70" | "source_failures_3"))
        .collect::<Vec<_>>();
    assert_eq!(
        relevant,
        vec![
            ("radar_chance_70".to_owned(), None),
            ("source_failures_3".to_owned(), Some("codex".to_owned())),
            ("source_failures_3".to_owned(), Some("radar".to_owned())),
        ]
    );
}

#[tokio::test]
async fn delivery_worker_sends_each_system_alert_once_and_records_the_attempt() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open(&database)
        .await
        .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");
    seed_daily_warning(&store).await;
    let notifier = RecordingNotifier::granted();
    let sent = Arc::clone(&notifier.sent);
    let worker = DeliveryWorker::new(store.clone(), notifier, "worker-a");

    let first = worker
        .deliver_pending(timestamp_ms("2026-07-30T02:00:01Z"))
        .await
        .expect("first sweep");
    let second = worker
        .deliver_pending(timestamp_ms("2026-07-30T02:01:01Z"))
        .await
        .expect("second sweep");

    assert_eq!(first.delivered, 1);
    assert_eq!(second.claimed, 0);
    let sent = sent.lock().expect("sent notifications");
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].delivery_key,
        "stream:1:epoch:1:date:2026-07-30:daily_80:system"
    );
    assert!(!sent[0].body.contains("account-one"));
    assert!(!sent[0].title.contains("account-one"));
    assert!(!sent[0].delivery_key.contains("account-one"));
    drop(sent);
    drop(store);

    let connection =
        tokio_rusqlite::rusqlite::Connection::open(database).expect("inspect delivery");
    let facts: (String, i64, i64) = connection
        .query_row(
            "SELECT d.state, d.attempt_count,
                    (SELECT COUNT(*) FROM delivery_attempts)
             FROM alert_deliveries d",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("delivery facts");
    assert_eq!(facts, ("delivered".to_owned(), 1, 1));
}

#[tokio::test]
async fn system_delivery_never_changes_the_email_channels_paused_state() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open(&database)
        .await
        .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");
    drop(store);
    let connection =
        tokio_rusqlite::rusqlite::Connection::open(&database).expect("configure email preference");
    connection
        .execute(
            "UPDATE alert_preferences SET enabled = 1
             WHERE event_kind = 'daily_80' AND channel = 'email'",
            [],
        )
        .expect("enable email preference");
    drop(connection);

    let store = AccountSettingsStore::open(&database)
        .await
        .expect("reopen store");
    seed_daily_warning(&store).await;
    let worker = DeliveryWorker::new(store, RecordingNotifier::granted(), "worker-system");
    assert_eq!(
        worker
            .deliver_pending(timestamp_ms("2026-07-30T02:00:01Z"))
            .await
            .expect("system sweep")
            .delivered,
        1
    );

    let connection =
        tokio_rusqlite::rusqlite::Connection::open(database).expect("inspect channels");
    let states = connection
        .prepare("SELECT channel, state FROM alert_deliveries ORDER BY channel")
        .expect("prepare channel states")
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .expect("query channel states")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect channel states");
    assert_eq!(
        states,
        vec![
            ("email".to_owned(), "paused_config".to_owned()),
            ("system".to_owned(), "delivered".to_owned()),
        ]
    );
}

#[tokio::test]
async fn denied_permission_pauses_without_losing_the_in_app_event_and_can_resume() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");
    seed_daily_warning(&store).await;

    let denied = DeliveryWorker::new(
        store.clone(),
        RecordingNotifier {
            permission: NotificationPermissionStatus::Denied,
            sent: Arc::new(Mutex::new(Vec::new())),
            fail: false,
        },
        "worker-denied",
    )
    .deliver_pending(timestamp_ms("2026-07-30T02:00:01Z"))
    .await
    .expect("denied sweep");
    assert_eq!(denied.paused, 1);

    let notifier = RecordingNotifier::granted();
    let sent = Arc::clone(&notifier.sent);
    let resumed = DeliveryWorker::new(store.clone(), notifier, "worker-granted")
        .deliver_pending(timestamp_ms("2026-07-30T02:01:01Z"))
        .await
        .expect("resumed sweep");
    assert_eq!(resumed.delivered, 1);
    assert_eq!(sent.lock().expect("sent after permission").len(), 1);
    assert_eq!(
        store
            .public_alerts(10)
            .await
            .expect("in-app alerts")
            .events
            .len(),
        1
    );
}

#[tokio::test]
async fn transient_failure_waits_for_backoff_and_reuses_the_same_delivery() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");
    seed_daily_warning(&store).await;
    let now = timestamp_ms("2026-07-30T02:00:01Z");
    let failed = DeliveryWorker::new(
        store.clone(),
        RecordingNotifier {
            permission: NotificationPermissionStatus::Granted,
            sent: Arc::new(Mutex::new(Vec::new())),
            fail: true,
        },
        "worker-failed",
    )
    .deliver_pending(now)
    .await
    .expect("failed sweep");
    assert_eq!(failed.retrying, 1);

    let notifier = RecordingNotifier::granted();
    let sent = Arc::clone(&notifier.sent);
    let worker = DeliveryWorker::new(store, notifier, "worker-retry");
    assert_eq!(
        worker
            .deliver_pending(now + 59_999)
            .await
            .expect("before backoff")
            .claimed,
        0
    );
    assert_eq!(
        worker
            .deliver_pending(now + 60_000)
            .await
            .expect("after backoff")
            .delivered,
        1
    );
    assert_eq!(sent.lock().expect("retried notification").len(), 1);
}

#[tokio::test]
async fn an_abandoned_lease_is_reclaimed_only_after_expiry() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");
    seed_daily_warning(&store).await;
    let now = timestamp_ms("2026-07-30T02:00:01Z");
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let blocked = {
        let store = store.clone();
        let entered = Arc::clone(&entered);
        let release = Arc::clone(&release);
        tokio::spawn(async move {
            DeliveryWorker::new(
                store,
                BlockingNotifier { entered, release },
                "worker-crashed",
            )
            .deliver_pending(now)
            .await
        })
    };
    entered.notified().await;
    blocked.abort();

    let notifier = RecordingNotifier::granted();
    let sent = Arc::clone(&notifier.sent);
    let worker = DeliveryWorker::new(store, notifier, "worker-recovered");
    assert_eq!(
        worker
            .deliver_pending(now + 59_999)
            .await
            .expect("lease still active")
            .claimed,
        0
    );
    assert_eq!(
        worker
            .deliver_pending(now + 60_000)
            .await
            .expect("lease expired")
            .delivered,
        1
    );
    assert_eq!(sent.lock().expect("reclaimed notification").len(), 1);
}
