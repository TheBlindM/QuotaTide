use quotatide_core::AccountSettingsStore;
use tempfile::tempdir;

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
            .pragma_update(None, "user_version", 2)
            .expect("seed newer schema");
    }

    assert!(AccountSettingsStore::open(&database).await.is_err());

    let connection =
        tokio_rusqlite::rusqlite::Connection::open(database).expect("inspect database");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version");
    assert_eq!(version, 2);
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|candidate| candidate == needle)
}
