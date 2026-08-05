use std::fs;

use quotatide_core::{AccountSettingsStore, SettingsStoreError};
use rusqlite::Connection;
use rusqlite::backup::Backup;
use tempfile::tempdir;

fn online_backup(source_path: &std::path::Path, destination_path: &std::path::Path) {
    let source = Connection::open(source_path).expect("open source database");
    let mut destination = Connection::open(destination_path).expect("open backup database");
    Backup::new(&source, &mut destination)
        .expect("start backup")
        .run_to_completion(64, std::time::Duration::from_millis(1), None)
        .expect("copy valid backup");
}

#[cfg(unix)]
#[tokio::test]
async fn database_files_are_restricted_to_the_current_user() {
    use std::os::unix::fs::PermissionsExt as _;

    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open(&database)
        .await
        .expect("create store");
    store.close().await.expect("close store");

    assert_eq!(
        fs::metadata(database)
            .expect("database metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[tokio::test]
async fn corrupted_main_database_is_isolated_and_restored_from_newest_valid_backup() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open(&database)
        .await
        .expect("create store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");
    store.close().await.expect("close store");

    let backups = directory.path().join("backups");
    fs::create_dir(&backups).expect("create backups");
    online_backup(&database, &backups.join("state-v10-100.sqlite3"));
    fs::write(&database, b"corrupt-main-canary").expect("corrupt main");

    let recovered = AccountSettingsStore::open(&database)
        .await
        .expect("recover valid backup");
    assert!(recovered.recovered_at_startup());
    let settings = recovered
        .public_settings()
        .await
        .expect("restored settings");

    assert!(settings.configured);
    assert_eq!(settings.settings_revision, 1);
    let isolated = fs::read_dir(directory.path().join("recovery"))
        .expect("read recovery root")
        .next()
        .expect("isolated database")
        .expect("recovery entry")
        .path()
        .join("state.sqlite3");
    assert_eq!(
        fs::read(isolated).expect("read isolated database"),
        b"corrupt-main-canary"
    );
}

#[tokio::test]
async fn recovery_skips_a_newer_integrity_valid_backup_with_broken_domain_invariants() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open(&database)
        .await
        .expect("create store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");
    store.close().await.expect("close store");
    let backups = directory.path().join("backups");
    fs::create_dir(&backups).expect("create backups");
    online_backup(&database, &backups.join("state-v10-100.sqlite3"));
    online_backup(&database, &backups.join("state-v10-200.sqlite3"));
    let invalid =
        Connection::open(backups.join("state-v10-200.sqlite3")).expect("open newest backup");
    invalid
        .execute_batch(
            "DROP TRIGGER policy_day_limits_are_immutable_delete;
             DELETE FROM policy_day_limits
             WHERE rowid = (SELECT MIN(rowid) FROM policy_day_limits);",
        )
        .expect("break domain invariant");
    drop(invalid);
    fs::write(&database, b"corrupt-main").expect("corrupt main");

    let recovered = AccountSettingsStore::open(&database)
        .await
        .expect("recover older valid backup");

    assert!(
        recovered
            .public_settings()
            .await
            .expect("settings")
            .configured
    );
}

#[tokio::test]
async fn invalid_main_and_backups_require_recovery_without_creating_an_empty_database() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("state.sqlite3");
    fs::write(&database, b"corrupt-main").expect("write corrupt main");
    let backups = directory.path().join("backups");
    fs::create_dir(&backups).expect("create backups");
    fs::write(backups.join("state-v10-100.sqlite3"), b"corrupt-backup")
        .expect("write corrupt backup");

    let result = AccountSettingsStore::open(&database).await;

    assert!(matches!(result, Err(SettingsStoreError::RecoveryRequired)));
    assert!(!database.exists());
    assert!(
        fs::read_dir(directory.path().join("recovery"))
            .expect("read recovery root")
            .next()
            .is_some()
    );
}

#[tokio::test]
async fn leftover_wal_and_shm_are_recovered_before_settings_are_read() {
    let source = tempfile::tempdir().expect("source directory");
    let source_database = source.path().join("state.sqlite3");
    let store = AccountSettingsStore::open(&source_database)
        .await
        .expect("source store");
    store.close().await.expect("close store");

    let writer = Connection::open(&source_database).expect("WAL writer");
    writer
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA wal_autocheckpoint = 0;
             UPDATE app_meta SET settings_revision = 17 WHERE singleton_id = 1;",
        )
        .expect("uncheckpointed revision");
    let source_wal = source_database.with_file_name("state.sqlite3-wal");
    let source_shm = source_database.with_file_name("state.sqlite3-shm");
    assert!(source_wal.is_file());
    assert!(source_shm.is_file());

    let destination = tempfile::tempdir().expect("destination directory");
    let destination_database = destination.path().join("state.sqlite3");
    fs::copy(&source_database, &destination_database).expect("copy main database");
    fs::copy(
        &source_wal,
        destination_database.with_file_name("state.sqlite3-wal"),
    )
    .expect("copy WAL");
    fs::copy(
        &source_shm,
        destination_database.with_file_name("state.sqlite3-shm"),
    )
    .expect("copy SHM");

    let recovered = AccountSettingsStore::open(&destination_database)
        .await
        .expect("recover WAL snapshot");
    assert_eq!(
        recovered
            .public_atomic_settings()
            .await
            .expect("public settings")
            .settings_revision,
        17
    );
    drop(writer);
}
