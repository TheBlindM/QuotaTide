use quotatide_core::{
    AccountSettingsStore, QuotaUnits, RefreshAccountBinding, SourceStatus, UsageCommitDisposition,
    UsageSourceErrorCode, WeeklyUsageObservation,
};
use tempfile::tempdir;

fn timestamp_ms(value: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(value)
        .expect("RFC 3339 timestamp")
        .timestamp_millis()
}

fn observation(captured_at_unix_ms: i64, used_micropoints: i64) -> WeeklyUsageObservation {
    observation_with_reset(captured_at_unix_ms, used_micropoints, 1_785_500_000)
}

fn observation_with_reset(
    captured_at_unix_ms: i64,
    used_micropoints: i64,
    resets_at_unix_s: i64,
) -> WeeklyUsageObservation {
    WeeklyUsageObservation {
        captured_at_unix_ms,
        used: QuotaUnits::from_micropoints(used_micropoints).expect("valid quota"),
        window_seconds: 604_800,
        resets_at_unix_s,
        plan_type: Some("plus".to_owned()),
        allowed: Some(true),
    }
}

fn binding(revision: u32, path: &str, account_id: &str) -> RefreshAccountBinding {
    RefreshAccountBinding::selected(revision, path.into()).with_account_id(account_id.to_owned())
}

#[tokio::test]
async fn success_commits_observation_and_health_as_one_public_snapshot() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");

    store
        .record_usage_success(
            &binding(1, "/chosen/auth.json", "account-one"),
            observation(1_785_000_000_000, 41_250_000),
        )
        .await
        .expect("record success");
    let quota = store
        .public_live_quota(1_785_000_100_000)
        .await
        .expect("live quota")
        .expect("configured quota");

    assert_eq!(quota.used_micropoints, Some(41_250_000));
    assert_eq!(quota.remaining_micropoints, Some(58_750_000));
    assert_eq!(quota.last_success_at_unix_ms, Some(1_785_000_000_000));
    assert_eq!(quota.consecutive_failures, 0);
    assert_eq!(quota.source_status, SourceStatus::Fresh);
    assert_eq!(quota.window_starts_at_unix_s, Some(1_784_895_200));
    assert_eq!(quota.window_ends_at_unix_s, Some(1_785_499_999));
    assert_eq!(quota.public_error, None);
    assert_eq!(quota.ledger_days.len(), 7);
    assert!(
        quota
            .ledger_days
            .iter()
            .all(|day| day.used_micropoints.is_none())
    );
    assert!(
        quota
            .ledger_days
            .iter()
            .all(|day| matches!(day.limit_micropoints, 10_000_000 | 16_000_000))
    );
    assert_eq!(
        quota
            .ledger_days
            .iter()
            .map(|day| day.base_micropoints)
            .sum::<u32>(),
        100_000_000
    );
}

#[tokio::test]
async fn ledger_survives_restart_and_only_projects_the_current_epoch_dates() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open(&database)
        .await
        .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");
    store
        .record_usage_success(
            &binding(1, "/chosen/auth.json", "account-one"),
            observation(1_785_000_000_000, 41_250_000),
        )
        .await
        .expect("baseline");
    store
        .record_usage_success(
            &binding(1, "/chosen/auth.json", "account-one"),
            observation(1_785_003_600_000, 42_250_000),
        )
        .await
        .expect("increase");
    let before_restart = store
        .public_live_quota(1_785_003_600_000)
        .await
        .expect("projection")
        .expect("quota");

    assert_eq!(before_restart.ledger_days.len(), 7);
    assert_eq!(
        before_restart
            .ledger_days
            .iter()
            .filter_map(|day| day.used_micropoints)
            .sum::<i64>(),
        1_000_000
    );
    assert_eq!(
        before_restart
            .ledger_days
            .iter()
            .filter(|day| day.status != quotatide_core::LedgerDayStatus::Unknown)
            .count(),
        1
    );

    drop(store);
    remove_derived_state_after_asserting_atomic_facts(&database);
    let reopened = AccountSettingsStore::open(&database)
        .await
        .expect("reopen store");
    let (after_restart_revision, after_restart) = reopened
        .public_live_quota_snapshot(1_785_003_600_000)
        .await
        .expect("rebuilt projection");
    let after_restart = after_restart.expect("quota");
    assert_eq!(after_restart_revision, 3);
    assert_eq!(after_restart, before_restart);

    reopened
        .record_usage_success(
            &binding(1, "/chosen/auth.json", "account-one"),
            observation(1_785_007_200_000, 42_750_000),
        )
        .await
        .expect("continue from rebuilt facts");
    let continued = reopened
        .public_live_quota(1_785_007_200_000)
        .await
        .expect("continued projection")
        .expect("quota");
    assert_eq!(
        continued
            .ledger_days
            .iter()
            .filter_map(|day| day.used_micropoints)
            .sum::<i64>(),
        1_500_000
    );
}

#[tokio::test]
async fn dashboard_projects_confirmed_weekday_carry_from_the_active_policy() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open_with_policy_timezone(
        directory.path().join("state.sqlite3"),
        "Asia/Shanghai",
    )
    .await
    .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");
    let account = binding(1, "/chosen/auth.json", "account-one");
    let reset = chrono::DateTime::parse_from_rfc3339("2026-08-03T00:00:00Z")
        .expect("reset")
        .timestamp();
    store
        .record_usage_success(
            &account,
            observation_with_reset(timestamp_ms("2026-07-27T01:00:00Z"), 0, reset),
        )
        .await
        .expect("baseline");
    store
        .record_usage_success(
            &account,
            observation_with_reset(timestamp_ms("2026-07-27T10:00:00Z"), 6_000_000, reset),
        )
        .await
        .expect("monday usage");
    store
        .record_usage_success(
            &account,
            observation_with_reset(timestamp_ms("2026-07-28T04:00:00Z"), 20_000_000, reset),
        )
        .await
        .expect("tuesday usage");

    let quota = store
        .public_live_quota(timestamp_ms("2026-07-28T04:00:00Z"))
        .await
        .expect("projection")
        .expect("quota");
    let monday = quota
        .ledger_days
        .iter()
        .find(|day| day.local_date == "2026-07-27")
        .expect("monday");
    let tuesday = quota
        .ledger_days
        .iter()
        .find(|day| day.local_date == "2026-07-28")
        .expect("tuesday");

    assert_eq!(monday.status, quotatide_core::LedgerDayStatus::Finalized);
    assert_eq!(monday.base_micropoints, 16_000_000);
    assert_eq!(monday.carry_micropoints, 0);
    assert_eq!(tuesday.used_micropoints, Some(14_000_000));
    assert_eq!(tuesday.carry_micropoints, 2_500_000);
    assert_eq!(tuesday.limit_micropoints, 18_500_000);
    assert_eq!(tuesday.status, quotatide_core::LedgerDayStatus::Normal);
}

fn remove_derived_state_after_asserting_atomic_facts(database: &std::path::Path) {
    let connection =
        tokio_rusqlite::rusqlite::Connection::open(database).expect("remove derived state");
    let facts: (i64, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT
               (SELECT dashboard_revision FROM app_meta WHERE singleton_id = 1),
               (SELECT COUNT(*) FROM usage_observations),
               (SELECT COUNT(*) FROM usage_observations
                WHERE quota_epoch_id IS NOT NULL),
               (SELECT COUNT(*) FROM quota_epochs),
               (SELECT COUNT(*) FROM daily_ledgers)",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("atomic ledger facts");
    assert_eq!(facts, (3, 2, 2, 1, 7));
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             DELETE FROM daily_ledgers;",
        )
        .expect("retain immutable observations only");
    assert!(
        connection
            .execute("UPDATE usage_observations SET used_micropoints = 0", [])
            .is_err()
    );
    assert!(
        connection
            .execute("DELETE FROM usage_observations", [])
            .is_err()
    );
    assert!(
        connection
            .execute(
                "INSERT INTO usage_observations
                 (account_stream_id, captured_at_ms, used_micropoints,
                  window_seconds, resets_at_s, quota_epoch_id)
                 VALUES (1, 1785007200000, 42000000, 604800, 1785500000, NULL)",
                [],
            )
            .is_err()
    );
}

#[tokio::test]
async fn confirmed_same_day_reset_retains_pre_and_post_reset_usage() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");
    let account = binding(1, "/chosen/auth.json", "account-one");
    store
        .record_usage_success(
            &account,
            observation_with_reset(1_700_600_000_000, 50_000_000, 1_700_604_800),
        )
        .await
        .expect("baseline");
    store
        .record_usage_success(
            &account,
            observation_with_reset(1_700_602_000_000, 55_000_000, 1_700_604_800),
        )
        .await
        .expect("pre-reset use");
    store
        .record_usage_success(
            &account,
            observation_with_reset(1_700_605_000_000, 2_000_000, 1_701_209_600),
        )
        .await
        .expect("confirmed reset");

    let quota = store
        .public_live_quota(1_700_605_000_000)
        .await
        .expect("projection")
        .expect("quota");
    assert_eq!(quota.ledger_days.len(), 7);
    assert_eq!(
        quota
            .ledger_days
            .iter()
            .filter_map(|day| day.used_micropoints)
            .sum::<i64>(),
        7_000_000
    );
}

#[tokio::test]
async fn two_coherent_low_observations_confirm_an_early_reset_after_restart() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open(&database)
        .await
        .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");
    let account = binding(1, "/chosen/auth.json", "account-one");
    store
        .record_usage_success(
            &account,
            observation_with_reset(1_700_000_000_000, 42_000_000, 1_700_604_800),
        )
        .await
        .expect("baseline");
    store
        .record_usage_success(
            &account,
            observation_with_reset(1_700_003_600_000, 2_000_000, 1_700_608_400),
        )
        .await
        .expect("candidate");
    store
        .record_usage_success(
            &account,
            observation_with_reset(1_700_007_200_000, 2_500_000, 1_700_608_420),
        )
        .await
        .expect("confirmation");
    drop(store);

    let reopened = AccountSettingsStore::open(&database)
        .await
        .expect("reopen store");
    let quota = reopened
        .public_live_quota(1_700_007_200_000)
        .await
        .expect("projection")
        .expect("quota");
    assert_eq!(
        quota
            .ledger_days
            .iter()
            .filter_map(|day| day.used_micropoints)
            .sum::<i64>(),
        2_500_000
    );
}

#[tokio::test]
async fn production_ledger_uses_the_persisted_iana_policy_timezone() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open_with_policy_timezone(
        directory.path().join("state.sqlite3"),
        "America/New_York",
    )
    .await
    .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");
    let before_midnight = chrono::DateTime::parse_from_rfc3339("2026-07-28T03:59:00Z")
        .expect("before midnight")
        .timestamp_millis();
    let after_midnight = chrono::DateTime::parse_from_rfc3339("2026-07-28T04:01:00Z")
        .expect("after midnight")
        .timestamp_millis();
    let reset = chrono::DateTime::parse_from_rfc3339("2026-08-03T04:00:00Z")
        .expect("reset")
        .timestamp();
    let account = binding(1, "/chosen/auth.json", "account-one");
    store
        .record_usage_success(
            &account,
            observation_with_reset(before_midnight, 40_000_000, reset),
        )
        .await
        .expect("baseline");
    store
        .record_usage_success(
            &account,
            observation_with_reset(after_midnight, 41_000_000, reset),
        )
        .await
        .expect("increase");

    let quota = store
        .public_live_quota(after_midnight)
        .await
        .expect("projection")
        .expect("quota");
    let known = quota
        .ledger_days
        .iter()
        .find(|day| day.used_micropoints.is_some())
        .expect("known day");
    assert_eq!(known.local_date, "2026-07-28");
    assert!(known.is_today);
}

#[tokio::test]
async fn public_reset_and_ledger_share_the_confirmed_schedule_boundary() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");
    let account = binding(1, "/chosen/auth.json", "account-one");
    store
        .record_usage_success(
            &account,
            observation_with_reset(1_700_000_000_000, 42_000_000, 1_700_604_800),
        )
        .await
        .expect("baseline");
    store
        .record_usage_success(
            &account,
            observation_with_reset(1_700_007_200_000, 42_500_000, 1_700_612_000),
        )
        .await
        .expect("schedule candidate");
    let candidate = store
        .public_live_quota(1_700_007_200_000)
        .await
        .expect("candidate projection")
        .expect("candidate quota");
    assert_eq!(candidate.resets_at_unix_s, Some(1_700_604_800));

    store
        .record_usage_success(
            &account,
            observation_with_reset(1_700_010_800_000, 43_000_000, 1_700_612_020),
        )
        .await
        .expect("confirmed schedule");
    let confirmed = store
        .public_live_quota(1_700_010_800_000)
        .await
        .expect("confirmed projection")
        .expect("confirmed quota");
    assert_eq!(confirmed.resets_at_unix_s, Some(1_700_612_020));
    assert_eq!(confirmed.window_starts_at_unix_s, Some(1_700_007_220));
    assert_eq!(confirmed.ledger_days.len(), 7);
}

#[tokio::test]
async fn failures_keep_last_known_good_and_mark_it_stale() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");
    store
        .record_usage_success(
            &binding(1, "/chosen/auth.json", "account-one"),
            observation(1_785_000_000_000, 41_250_000),
        )
        .await
        .expect("record success");

    store
        .record_usage_failure(
            &binding(1, "/chosen/auth.json", "account-one"),
            1_785_000_200_000,
            UsageSourceErrorCode::RateLimited,
        )
        .await
        .expect("record failure");
    let quota = store
        .public_live_quota(1_785_000_200_000)
        .await
        .expect("live quota")
        .expect("last-known-good");

    assert_eq!(quota.used_micropoints, Some(41_250_000));
    assert_eq!(quota.consecutive_failures, 1);
    assert_eq!(quota.source_status, SourceStatus::StaleAfterFailure);
    assert_eq!(quota.public_error, Some(UsageSourceErrorCode::RateLimited));
}

#[tokio::test]
async fn failure_before_any_success_is_available_as_unavailable_health() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");

    store
        .record_usage_failure(
            &binding(1, "/chosen/auth.json", "account-one"),
            1_785_000_200_000,
            UsageSourceErrorCode::Timeout,
        )
        .await
        .expect("record failure");
    let quota = store
        .public_live_quota(1_785_000_200_000)
        .await
        .expect("live quota")
        .expect("source health");

    assert_eq!(quota.used_micropoints, None);
    assert_eq!(quota.last_success_at_unix_ms, None);
    assert_eq!(quota.consecutive_failures, 1);
    assert_eq!(quota.source_status, SourceStatus::Unavailable);
    assert_eq!(quota.public_error, Some(UsageSourceErrorCode::Timeout));
}

#[tokio::test]
async fn account_switches_do_not_project_the_previous_accounts_quota() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    store
        .configure_account(0, "/one/auth.json", "account-one")
        .await
        .expect("first account");
    store
        .record_usage_success(
            &binding(1, "/one/auth.json", "account-one"),
            observation(1_785_000_000_000, 41_250_000),
        )
        .await
        .expect("first observation");

    store
        .configure_account(1, "/two/auth.json", "account-two")
        .await
        .expect("second account");

    assert_eq!(
        store
            .public_live_quota(1_785_000_100_000)
            .await
            .expect("second quota"),
        None
    );
}

#[tokio::test]
async fn switching_back_restores_only_that_accounts_ledger() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    store
        .configure_account(0, "/one/auth.json", "account-one")
        .await
        .expect("first account");
    let first_binding = binding(1, "/one/auth.json", "account-one");
    store
        .record_usage_success(&first_binding, observation(1_785_000_000_000, 40_000_000))
        .await
        .expect("baseline");
    store
        .record_usage_success(&first_binding, observation(1_785_003_600_000, 43_000_000))
        .await
        .expect("increase");

    store
        .configure_account(1, "/two/auth.json", "account-two")
        .await
        .expect("second account");
    assert_eq!(
        store
            .public_live_quota(1_785_003_600_000)
            .await
            .expect("second account projection"),
        None
    );

    store
        .configure_account(2, "/one/auth.json", "account-one")
        .await
        .expect("switch back");
    let restored = store
        .public_live_quota(1_785_003_600_000)
        .await
        .expect("restored projection")
        .expect("first account quota");
    assert_eq!(
        restored
            .ledger_days
            .iter()
            .filter_map(|day| day.used_micropoints)
            .sum::<i64>(),
        3_000_000
    );
}

#[tokio::test]
async fn age_alone_makes_a_last_success_stale_after_ninety_minutes() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");
    store
        .record_usage_success(
            &binding(1, "/chosen/auth.json", "account-one"),
            observation(1_785_000_000_000, 41_250_000),
        )
        .await
        .expect("record success");

    let quota = store
        .public_live_quota(1_785_005_400_001)
        .await
        .expect("live quota")
        .expect("last-known-good");

    assert_eq!(quota.source_status, SourceStatus::StaleByAge);
}

#[tokio::test]
async fn a_mid_transaction_failure_leaves_no_partial_success_visible() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open(&database)
        .await
        .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");
    {
        let connection =
            tokio_rusqlite::rusqlite::Connection::open(&database).expect("open fault injector");
        connection
            .execute_batch(
                "CREATE TRIGGER reject_success_health
                 BEFORE INSERT ON usage_source_health
                 WHEN NEW.consecutive_failures = 0
                 BEGIN
                   SELECT RAISE(ABORT, 'injected health failure');
                 END;",
            )
            .expect("install fault injector");
    }

    assert!(
        store
            .record_usage_success(
                &binding(1, "/chosen/auth.json", "account-one"),
                observation(1_785_000_000_000, 41_250_000),
            )
            .await
            .is_err()
    );
    assert_eq!(
        store
            .public_live_quota(1_785_000_100_000)
            .await
            .expect("public projection"),
        None
    );
    drop(store);
    let connection =
        tokio_rusqlite::rusqlite::Connection::open(&database).expect("inspect rollback");
    for table in [
        "usage_observations",
        "quota_epochs",
        "daily_ledgers",
        "usage_source_health",
    ] {
        let count: i64 = connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("rolled-back table count");
        assert_eq!(count, 0, "{table} must roll back");
    }
    let dashboard_revision: i64 = connection
        .query_row(
            "SELECT dashboard_revision FROM app_meta WHERE singleton_id = 1",
            [],
            |row| row.get(0),
        )
        .expect("dashboard revision");
    assert_eq!(dashboard_revision, 1);
}

#[tokio::test]
async fn account_identity_changes_on_the_same_path_create_a_new_current_stream() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure first account");
    let first_label = store
        .public_settings()
        .await
        .expect("first public settings")
        .account_label;
    store
        .record_usage_success(
            &binding(1, "/chosen/auth.json", "account-one"),
            observation(1_785_000_000_000, 41_250_000),
        )
        .await
        .expect("first account observation");

    let disposition = store
        .record_usage_success(
            &binding(1, "/chosen/auth.json", "account-two"),
            observation(1_785_000_100_000, 7_000_000),
        )
        .await
        .expect("second account observation");
    let public = store
        .public_live_quota(1_785_000_100_000)
        .await
        .expect("current quota")
        .expect("second account quota");

    assert_eq!(disposition, UsageCommitDisposition::Committed);
    assert_eq!(store.account_stream_count().await.expect("stream count"), 2);
    assert_eq!(public.used_micropoints, Some(7_000_000));
    let updated_settings = store
        .public_settings()
        .await
        .expect("updated account settings");
    assert_eq!(updated_settings.settings_revision, 2);
    assert_ne!(updated_settings.account_label, first_label);
}

#[tokio::test]
async fn a_response_from_a_previous_selection_is_discarded() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    store
        .configure_account(0, "/one/auth.json", "account-one")
        .await
        .expect("first selection");
    store
        .configure_account(1, "/two/auth.json", "account-two")
        .await
        .expect("second selection");

    let disposition = store
        .record_usage_success(
            &binding(1, "/one/auth.json", "account-one"),
            observation(1_785_000_000_000, 41_250_000),
        )
        .await
        .expect("discard old response");

    assert_eq!(disposition, UsageCommitDisposition::Superseded);
    assert_eq!(
        store
            .public_live_quota(1_785_000_100_000)
            .await
            .expect("current quota"),
        None
    );
}

#[tokio::test]
async fn a_new_account_failure_never_displays_the_previous_accounts_snapshot() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure first account");
    store
        .record_usage_success(
            &binding(1, "/chosen/auth.json", "account-one"),
            observation(1_785_000_000_000, 41_250_000),
        )
        .await
        .expect("first account observation");

    store
        .record_usage_failure(
            &binding(1, "/chosen/auth.json", "account-two"),
            1_785_000_100_000,
            UsageSourceErrorCode::Timeout,
        )
        .await
        .expect("new account failure");
    let public = store
        .public_live_quota(1_785_000_100_000)
        .await
        .expect("current quota")
        .expect("new account health");

    assert_eq!(store.account_stream_count().await.expect("stream count"), 2);
    assert_eq!(public.used_micropoints, None);
    assert_eq!(public.source_status, SourceStatus::Unavailable);
    assert_eq!(public.public_error, Some(UsageSourceErrorCode::Timeout));
}
