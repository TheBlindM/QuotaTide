use quotatide_core::AccountSettingsStore;
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
async fn newer_schema_is_rejected_without_downgrade() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    {
        let connection =
            tokio_rusqlite::rusqlite::Connection::open(&database).expect("seed database");
        connection
            .pragma_update(None, "user_version", 3)
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
    assert_eq!(version, 3);
}

#[tokio::test]
async fn version_one_settings_are_preserved_while_live_quota_tables_are_added() {
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

    assert_eq!(version, 2);
    assert_eq!(quota_table, "usage_observations");
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

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|candidate| candidate == needle)
}
