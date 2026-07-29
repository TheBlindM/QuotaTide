use quotatide_core::{AccountSettingsStore, QuotaPolicyDraft};
use tempfile::tempdir;
use tokio_rusqlite::rusqlite;

const CANARY_ACCOUNT_ID: &str = "user-ticket16-account-canary";
const CANARY_PATH: &str = "/private/canary/home/.codex/auth.json";

#[tokio::test]
async fn account_configuration_is_atomic_persistent_and_secret_free() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open(&database)
        .await
        .expect("open settings store");

    let initial = store.public_settings().await.expect("initial settings");
    assert_eq!(initial.settings_revision, 0);
    assert!(!initial.configured);

    let configured = store
        .configure_account(0, CANARY_PATH, CANARY_ACCOUNT_ID)
        .await
        .expect("configure account");
    assert_eq!(configured.settings_revision, 1);
    assert!(configured.configured);
    assert_eq!(configured.path_summary.as_deref(), Some("…/auth.json"));
    assert!(configured.account_label.as_deref().is_some_and(|label| {
        label.starts_with("账号 • ") && !label.contains(CANARY_ACCOUNT_ID)
    }));

    drop(store);
    let reopened = AccountSettingsStore::open(&database)
        .await
        .expect("reopen settings store");
    assert_eq!(
        reopened.public_settings().await.expect("restored settings"),
        configured
    );

    drop(reopened);
    for entry in std::fs::read_dir(directory.path()).expect("list database artifacts") {
        let path = entry.expect("artifact entry").path();
        if path.is_file() {
            let bytes = std::fs::read(path).expect("read database artifact");
            assert!(!contains(&bytes, CANARY_ACCOUNT_ID.as_bytes()));
        }
    }
}

#[tokio::test]
async fn switching_accounts_uses_distinct_stable_streams_without_merging() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open settings store");

    let first = store
        .configure_account(0, "/one/auth.json", "account-one")
        .await
        .expect("first account");
    let second = store
        .configure_account(1, "/two/auth.json", "account-two")
        .await
        .expect("second account");
    let first_again = store
        .configure_account(2, "/one/auth.json", "account-one")
        .await
        .expect("first account again");

    assert_ne!(first.account_label, second.account_label);
    assert_eq!(first.account_label, first_again.account_label);
    assert_eq!(store.account_stream_count().await.expect("stream count"), 2);
}

#[tokio::test]
async fn stale_revision_is_rejected_without_overwriting_the_selected_account() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open settings store");

    let accepted = store
        .configure_account(0, "/one/auth.json", "account-one")
        .await
        .expect("first account");
    let rejected = store
        .configure_account(0, "/two/auth.json", "account-two")
        .await;

    assert!(matches!(
        rejected,
        Err(quotatide_core::SettingsStoreError::Conflict)
    ));
    assert_eq!(
        store.public_settings().await.expect("current settings"),
        accepted
    );
    assert_eq!(store.account_stream_count().await.expect("stream count"), 1);
}

#[tokio::test]
async fn daily_policy_updates_append_atomically_and_survive_restart() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open_with_policy_timezone(&database, "Asia/Shanghai")
        .await
        .expect("open settings store");
    let initial = store.public_settings().await.expect("initial settings");
    assert_eq!(
        initial.quota_policy.base_micropoints,
        vec![
            16_000_000, 16_000_000, 16_000_000, 16_000_000, 16_000_000, 10_000_000, 10_000_000,
        ]
    );

    let updated = store
        .update_quota_policy(
            0,
            QuotaPolicyDraft {
                policy_timezone: "America/New_York".to_owned(),
                carry_workdays_enabled: true,
                base_micropoints: vec![
                    20_000_000, 20_000_000, 20_000_000, 20_000_000, 0, 10_000_000, 10_000_000,
                ],
            },
        )
        .await
        .expect("update policy");
    assert_eq!(updated.settings_revision, 1);
    assert_eq!(
        updated.quota_policy.policy_revision,
        initial.quota_policy.policy_revision + 1
    );
    assert_eq!(updated.quota_policy.policy_timezone, "America/New_York");

    let rejected = store
        .update_quota_policy(
            1,
            QuotaPolicyDraft {
                policy_timezone: "Asia/Shanghai".to_owned(),
                carry_workdays_enabled: false,
                base_micropoints: vec![20_000_000; 7],
            },
        )
        .await;
    assert!(matches!(
        rejected,
        Err(quotatide_core::SettingsStoreError::InvalidPolicy(_))
    ));
    assert_eq!(
        store.public_settings().await.expect("unchanged settings"),
        updated
    );

    drop(store);
    let reopened = AccountSettingsStore::open(&database)
        .await
        .expect("reopen settings store");
    assert_eq!(
        reopened.public_settings().await.expect("restored settings"),
        updated
    );
}

#[tokio::test]
async fn newer_schema_is_rejected_without_downgrade() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    {
        let connection =
            tokio_rusqlite::rusqlite::Connection::open(&database).expect("seed database");
        connection
            .pragma_update(None, "user_version", 6)
            .expect("seed newer schema");
    }
    let before = std::fs::read(&database).expect("snapshot newer database");

    assert!(AccountSettingsStore::open(&database).await.is_err());
    assert_eq!(
        std::fs::read(&database).expect("re-read newer database"),
        before
    );
    let artifacts = std::fs::read_dir(directory.path())
        .expect("list newer database artifacts")
        .count();
    assert_eq!(artifacts, 1);

    let connection =
        tokio_rusqlite::rusqlite::Connection::open(database).expect("inspect database");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version");
    assert_eq!(version, 6);
}

#[tokio::test]
async fn version_one_settings_are_preserved_while_live_quota_and_ledger_tables_are_added() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    seed_version_one_database(&database);

    let store = AccountSettingsStore::open(&database)
        .await
        .expect("migrate version one");
    let settings = store.public_settings().await.expect("migrated settings");

    assert!(settings.configured);
    assert_eq!(settings.settings_revision, 7);
    assert_eq!(settings.path_summary.as_deref(), Some("…/auth.json"));
    assert_eq!(store.account_stream_count().await.expect("stream count"), 1);
    assert_eq!(
        store
            .public_live_quota(1_785_000_000_000)
            .await
            .expect("empty live quota"),
        None
    );

    drop(store);
    let connection = rusqlite::Connection::open(database).expect("inspect migrated database");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read migrated version");
    let quota_table: String = connection
        .query_row(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name = 'usage_observations'",
            [],
            |row| row.get(0),
        )
        .expect("live quota table");
    let ledger_table: String = connection
        .query_row(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name = 'daily_ledgers'",
            [],
            |row| row.get(0),
        )
        .expect("daily ledger table");
    let migration_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .expect("migration count");

    assert_eq!(version, 5);
    assert_eq!(migration_count, 5);
    assert_eq!(quota_table, "usage_observations");
    assert_eq!(ledger_table, "daily_ledgers");
}

#[tokio::test]
async fn populated_version_two_observations_are_backfilled_and_made_immutable() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    seed_version_two_database_with_observations(&database);

    let store = AccountSettingsStore::open_with_policy_timezone(&database, "America/New_York")
        .await
        .expect("migrate populated version two");
    let quota = store
        .public_live_quota(1_785_003_600_000)
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
        1_000_000
    );
    drop(store);

    let connection = rusqlite::Connection::open(database).expect("inspect migrated database");
    let facts: (i64, i64, String) = connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM usage_observations),
               (SELECT COUNT(*) FROM usage_observations
                WHERE quota_epoch_id IS NOT NULL),
               (SELECT policy_timezone FROM app_settings WHERE singleton_id = 1)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("backfilled facts");
    assert_eq!(facts, (2, 2, "America/New_York".to_owned()));
    let epoch_not_null: i64 = connection
        .query_row(
            "SELECT \"notnull\" FROM pragma_table_info('usage_observations')
             WHERE name = 'quota_epoch_id'",
            [],
            |row| row.get(0),
        )
        .expect("quota epoch nullability");
    assert_eq!(epoch_not_null, 1);
    assert!(
        connection
            .execute("DELETE FROM usage_observations", [])
            .is_err()
    );
}

#[tokio::test]
async fn legacy_observation_outside_the_new_strict_window_is_quarantined() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    seed_version_two_database_with_observations(&database);
    {
        let connection = rusqlite::Connection::open(&database).expect("extend legacy database");
        connection
            .execute_batch(
                "INSERT INTO usage_observations VALUES
                   (3, 1, 1785007200000, 42000000, 604800, 1788000000, 'plus', 1);
                 UPDATE usage_source_health
                   SET last_attempt_at_ms = 1785007200000,
                       last_success_at_ms = 1785007200000,
                       consecutive_failures = 0,
                       public_error = NULL
                   WHERE account_stream_id = 1;",
            )
            .expect("legacy-valid observation");
    }

    let store = AccountSettingsStore::open_with_policy_timezone(&database, "Asia/Shanghai")
        .await
        .expect("migrate legacy observation");
    let quota = store
        .public_live_quota(1_785_007_200_000)
        .await
        .expect("public quota")
        .expect("eligible quota");
    assert_eq!(quota.used_micropoints, Some(41_000_000));
    assert_eq!(quota.last_success_at_unix_ms, Some(1_785_003_600_000));
    assert_eq!(
        quota.source_status,
        quotatide_core::SourceStatus::StaleAfterFailure
    );
    assert_eq!(
        quota.public_error,
        Some(quotatide_core::UsageSourceErrorCode::ContractViolation)
    );
    drop(store);

    let connection = rusqlite::Connection::open(database).expect("inspect migrated database");
    let facts: (i64, i64, i64) = connection
        .query_row(
            "SELECT COUNT(*), SUM(ledger_eligible),
                    SUM(quota_epoch_id IS NOT NULL)
             FROM usage_observations",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("quarantine facts");
    assert_eq!(facts, (3, 2, 3));
}

#[tokio::test]
async fn all_quarantined_legacy_observations_make_source_unavailable() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    seed_version_two_database_with_observations(&database);
    {
        let connection = rusqlite::Connection::open(&database).expect("invalidate legacy windows");
        connection
            .execute("UPDATE usage_observations SET resets_at_s = 1788000000", [])
            .expect("legacy-only window relationship");
    }

    let store = AccountSettingsStore::open(&database)
        .await
        .expect("migrate all-quarantined stream");
    let quota = store
        .public_live_quota(1_785_003_600_000)
        .await
        .expect("public health")
        .expect("configured stream health");

    assert_eq!(quota.used_micropoints, None);
    assert_eq!(quota.last_success_at_unix_ms, None);
    assert_eq!(
        quota.source_status,
        quotatide_core::SourceStatus::Unavailable
    );
    assert_eq!(
        quota.public_error,
        Some(quotatide_core::UsageSourceErrorCode::ContractViolation)
    );
    assert!(quota.ledger_days.is_empty());
}

#[tokio::test]
async fn populated_version_three_is_upgraded_without_rewriting_its_checksum() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    seed_version_three_database_with_observations(&database);

    let store = AccountSettingsStore::open_with_policy_timezone(&database, "Europe/Berlin")
        .await
        .expect("migrate populated version three");
    assert!(
        store
            .public_live_quota(1_785_003_600_000)
            .await
            .expect("projection")
            .is_some()
    );
    drop(store);

    let connection = rusqlite::Connection::open(database).expect("inspect migrated database");
    let facts: (i64, i64, String, String, String) = connection
        .query_row(
            "SELECT
               (SELECT user_version FROM pragma_user_version),
               (SELECT COUNT(*) FROM usage_observations
                WHERE quota_epoch_id IS NOT NULL),
               (SELECT checksum FROM schema_migrations WHERE version = 3),
               (SELECT checksum FROM schema_migrations WHERE version = 4),
               (SELECT policy_timezone FROM app_settings WHERE singleton_id = 1)",
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
        .expect("upgraded facts");
    assert_eq!(
        facts,
        (
            5,
            2,
            "quotatide-v3-current-seven-day-ledger".to_owned(),
            "quotatide-v4-immutable-observations-iana-policy".to_owned(),
            "Europe/Berlin".to_owned(),
        )
    );
}

fn seed_version_one_database(path: &std::path::Path) {
    let connection = rusqlite::Connection::open(path).expect("seed version one database");
    connection
        .execute_batch(
            "CREATE TABLE schema_migrations (
               version INTEGER PRIMARY KEY,
               applied_at_ms INTEGER NOT NULL,
               app_version TEXT NOT NULL,
               checksum TEXT NOT NULL
             );
             CREATE TABLE app_meta (
               singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
               app_instance_id TEXT NOT NULL UNIQUE,
               local_hash_salt BLOB NOT NULL CHECK (length(local_hash_salt) = 32),
               settings_revision INTEGER NOT NULL CHECK (settings_revision >= 0),
               created_at_ms INTEGER NOT NULL,
               updated_at_ms INTEGER NOT NULL
             );
             CREATE TABLE account_streams (
               id INTEGER PRIMARY KEY,
               stream_key TEXT NOT NULL UNIQUE,
               account_key BLOB NOT NULL UNIQUE,
               first_seen_at_ms INTEGER NOT NULL,
               last_seen_at_ms INTEGER NOT NULL
             );
             CREATE TABLE app_settings (
               singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
               auth_path TEXT,
               configured_account_stream_id INTEGER REFERENCES account_streams(id),
               active_account_stream_id INTEGER REFERENCES account_streams(id),
               created_at_ms INTEGER NOT NULL,
               updated_at_ms INTEGER NOT NULL
             );
             INSERT INTO schema_migrations VALUES
               (1, 1785000000000, '0.1.0', 'quotatide-settings-v1-account-path-stream');
             INSERT INTO app_meta VALUES
               (1, 'migration-canary', zeroblob(32), 7, 1785000000000, 1785000000000);
             INSERT INTO account_streams VALUES
               (1, 'stream-canary', zeroblob(32), 1785000000000, 1785000000000);
             INSERT INTO app_settings VALUES
               (1, '/preserved/auth.json', 1, NULL, 1785000000000, 1785000000000);
             PRAGMA user_version = 1;",
        )
        .expect("seed version one schema");
}

fn seed_version_two_database_with_observations(path: &std::path::Path) {
    seed_version_one_database(path);
    let connection = rusqlite::Connection::open(path).expect("seed version two database");
    connection
        .execute_batch(
            "CREATE TABLE usage_observations (
               id INTEGER PRIMARY KEY,
               account_stream_id INTEGER NOT NULL REFERENCES account_streams(id),
               captured_at_ms INTEGER NOT NULL,
               used_micropoints INTEGER NOT NULL
                 CHECK (used_micropoints BETWEEN 0 AND 100000000),
               window_seconds INTEGER NOT NULL CHECK (window_seconds = 604800),
               resets_at_s INTEGER NOT NULL,
               plan_type TEXT,
               allowed INTEGER,
               UNIQUE(account_stream_id, captured_at_ms)
             );
             CREATE TABLE usage_source_health (
               account_stream_id INTEGER PRIMARY KEY REFERENCES account_streams(id),
               last_attempt_at_ms INTEGER NOT NULL,
               last_success_at_ms INTEGER,
               consecutive_failures INTEGER NOT NULL CHECK (consecutive_failures >= 0),
               public_error TEXT
             );
             CREATE INDEX usage_observations_stream_capture
               ON usage_observations(account_stream_id, captured_at_ms DESC);
             INSERT INTO usage_observations VALUES
               (1, 1, 1785000000000, 40000000, 604800, 1785500000, 'plus', 1),
               (2, 1, 1785003600000, 41000000, 604800, 1785500000, 'plus', 1);
             INSERT INTO usage_source_health VALUES
               (1, 1785003600000, 1785003600000, 0, NULL);
             INSERT INTO schema_migrations VALUES
               (2, 1785000000000, '0.1.0', 'quotatide-v2-live-quota-health');
             PRAGMA user_version = 2;",
        )
        .expect("seed version two schema");
}

fn seed_version_three_database_with_observations(path: &std::path::Path) {
    seed_version_two_database_with_observations(path);
    let connection = rusqlite::Connection::open(path).expect("seed version three database");
    connection
        .execute_batch(
            "CREATE TABLE quota_epochs (
               id INTEGER PRIMARY KEY,
               account_stream_id INTEGER NOT NULL REFERENCES account_streams(id),
               sequence INTEGER NOT NULL CHECK (sequence > 0),
               baseline_micropoints INTEGER NOT NULL
                 CHECK (baseline_micropoints BETWEEN 0 AND 100000000),
               high_water_micropoints INTEGER NOT NULL
                 CHECK (high_water_micropoints BETWEEN 0 AND 100000000),
               first_observed_at_ms INTEGER NOT NULL,
               latest_observed_at_ms INTEGER NOT NULL,
               scheduled_reset_at_s INTEGER NOT NULL,
               closed_at_ms INTEGER,
               UNIQUE(account_stream_id, sequence)
             );
             CREATE UNIQUE INDEX one_active_quota_epoch_per_stream
               ON quota_epochs(account_stream_id) WHERE closed_at_ms IS NULL;
             CREATE TABLE daily_ledgers (
               id INTEGER PRIMARY KEY,
               account_stream_id INTEGER NOT NULL REFERENCES account_streams(id),
               local_date TEXT NOT NULL,
               policy_timezone TEXT NOT NULL,
               used_micropoints INTEGER NOT NULL CHECK (used_micropoints >= 0),
               updated_at_ms INTEGER NOT NULL,
               UNIQUE(account_stream_id, local_date, policy_timezone)
             );
             ALTER TABLE usage_observations
               ADD COLUMN quota_epoch_id INTEGER REFERENCES quota_epochs(id);
             ALTER TABLE app_meta
               ADD COLUMN dashboard_revision INTEGER NOT NULL DEFAULT 0;
             INSERT INTO quota_epochs VALUES
               (1, 1, 1, 40000000, 41000000, 1785000000000,
                1785003600000, 1785500000, NULL);
             UPDATE usage_observations SET quota_epoch_id = 1;
             INSERT INTO daily_ledgers VALUES
               (1, 1, '2026-07-25', 'Asia/Shanghai', 1000000, 1785003600000);
             INSERT INTO schema_migrations VALUES
               (3, 1785000000000, '0.1.0',
                'quotatide-v3-current-seven-day-ledger');
             PRAGMA user_version = 3;",
        )
        .expect("seed version three schema");
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|candidate| candidate == needle)
}
