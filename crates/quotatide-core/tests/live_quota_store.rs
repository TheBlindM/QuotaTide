use quotatide_core::{
    AccountSettingsStore, QuotaUnits, SourceFreshness, UsageSourceErrorCode, WeeklyUsageObservation,
};
use tempfile::tempdir;

fn observation(captured_at_unix_ms: i64, used_micropoints: i64) -> WeeklyUsageObservation {
    WeeklyUsageObservation {
        captured_at_unix_ms,
        used: QuotaUnits::from_micropoints(used_micropoints).expect("valid quota"),
        window_seconds: 604_800,
        resets_at_unix_s: 1_786_000_000,
        plan_type: Some("plus".to_owned()),
        allowed: Some(true),
    }
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
        .record_usage_success(observation(1_785_000_000_000, 41_250_000))
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
    assert_eq!(quota.freshness, SourceFreshness::Fresh);
    assert_eq!(quota.public_error, None);
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
        .record_usage_success(observation(1_785_000_000_000, 41_250_000))
        .await
        .expect("record success");

    store
        .record_usage_failure(1_785_000_200_000, UsageSourceErrorCode::RateLimited)
        .await
        .expect("record failure");
    let quota = store
        .public_live_quota(1_785_000_200_000)
        .await
        .expect("live quota")
        .expect("last-known-good");

    assert_eq!(quota.used_micropoints, Some(41_250_000));
    assert_eq!(quota.consecutive_failures, 1);
    assert_eq!(quota.freshness, SourceFreshness::Stale);
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
        .record_usage_failure(1_785_000_200_000, UsageSourceErrorCode::Timeout)
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
    assert_eq!(quota.freshness, SourceFreshness::Unavailable);
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
        .record_usage_success(observation(1_785_000_000_000, 41_250_000))
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
        .record_usage_success(observation(1_785_000_000_000, 41_250_000))
        .await
        .expect("record success");

    let quota = store
        .public_live_quota(1_785_005_400_001)
        .await
        .expect("live quota")
        .expect("last-known-good");

    assert_eq!(quota.freshness, SourceFreshness::Stale);
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
            .record_usage_success(observation(1_785_000_000_000, 41_250_000))
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
}
