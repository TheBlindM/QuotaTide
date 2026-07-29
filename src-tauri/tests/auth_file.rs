use std::fs;

use quotatide_core::{AccountApplication, AccountSettingsStore, SettingsManager};
use quotatide_lib::auth_file::{AuthFileErrorCode, AuthFileReader, read_auth_file};
use secrecy::ExposeSecret;
use sha2::{Digest, Sha256};
use tempfile::tempdir;

const ACCESS_CANARY: &str = "access-ticket16-canary";
const ACCOUNT_CANARY: &str = "account-ticket16-canary";
const ID_TOKEN_CANARY: &str = "header.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9hY2NvdW50X2lkIjoiYWNjb3VudC1qd3QtZmFsbGJhY2sifX0.signature";

#[test]
fn reads_nested_codex_auth_without_modifying_the_file() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("auth.json");
    fs::write(
        &path,
        format!(
            r#"{{"auth_mode":"chatgpt","tokens":{{"access_token":"{ACCESS_CANARY}","account_id":"{ACCOUNT_CANARY}","id_token":"ignored"}}}}"#
        ),
    )
    .expect("write fixture");
    let before = snapshot(&path);

    let material = read_auth_file(&path).expect("valid Codex auth");

    assert_eq!(material.account_id(), ACCOUNT_CANARY);
    assert_eq!(material.access_token().expose_secret(), ACCESS_CANARY);
    assert_eq!(material.canonical_path(), fs::canonicalize(&path).unwrap());
    assert_eq!(snapshot(&path), before);
}

#[test]
fn falls_back_to_the_nested_canonical_account_claim() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("auth.json");
    fs::write(
        &path,
        format!(
            r#"{{"auth_mode":"chatgpt","tokens":{{"access_token":"{ACCESS_CANARY}","id_token":"{ID_TOKEN_CANARY}"}}}}"#
        ),
    )
    .expect("write fixture");

    let material = read_auth_file(&path).expect("valid fallback claim");

    assert_eq!(material.account_id(), "account-jwt-fallback");
}

#[test]
fn maps_invalid_inputs_to_stable_safe_error_codes() {
    let directory = tempdir().expect("temporary directory");
    let missing = directory.path().join("missing.json");
    assert_eq!(
        read_auth_file(&missing)
            .err()
            .expect("missing error")
            .code(),
        AuthFileErrorCode::NotFound
    );

    let invalid = directory.path().join("auth.json");
    fs::write(&invalid, format!(r#"{{"secret":"{ACCESS_CANARY}""#)).unwrap();
    let error = read_auth_file(&invalid).err().expect("invalid JSON error");
    assert_eq!(error.code(), AuthFileErrorCode::InvalidJson);
    let public = serde_json::to_string(&error.public()).expect("serialize public error");
    assert!(!public.contains(ACCESS_CANARY));
    assert!(!public.contains(invalid.to_string_lossy().as_ref()));
}

#[test]
fn distinguishes_size_mode_and_required_field_failures() {
    let directory = tempdir().expect("temporary directory");

    let too_large = directory.path().join("too-large.json");
    fs::write(&too_large, vec![b'x'; 1024 * 1024 + 1]).unwrap();
    assert_eq!(
        read_auth_file(&too_large).err().expect("size error").code(),
        AuthFileErrorCode::TooLarge
    );

    let unsupported = directory.path().join("unsupported.json");
    fs::write(
        &unsupported,
        format!(
            r#"{{"auth_mode":"api_key","tokens":{{"access_token":"{ACCESS_CANARY}","account_id":"{ACCOUNT_CANARY}"}}}}"#
        ),
    )
    .unwrap();
    assert_eq!(
        read_auth_file(&unsupported)
            .err()
            .expect("mode error")
            .code(),
        AuthFileErrorCode::UnsupportedAuthMode
    );

    let missing_token = directory.path().join("missing-token.json");
    fs::write(
        &missing_token,
        format!(r#"{{"auth_mode":"chatgpt","tokens":{{"account_id":"{ACCOUNT_CANARY}"}}}}"#),
    )
    .unwrap();
    assert_eq!(
        read_auth_file(&missing_token)
            .err()
            .expect("token error")
            .code(),
        AuthFileErrorCode::MissingAccessToken
    );

    let missing_account = directory.path().join("missing-account.json");
    fs::write(
        &missing_account,
        format!(r#"{{"auth_mode":"chatgpt","tokens":{{"access_token":"{ACCESS_CANARY}"}}}}"#),
    )
    .unwrap();
    assert_eq!(
        read_auth_file(&missing_account)
            .err()
            .expect("account error")
            .code(),
        AuthFileErrorCode::MissingAccountId
    );
}

#[test]
fn rejects_non_files_and_non_canonical_account_ids() {
    let directory = tempdir().expect("temporary directory");
    assert_eq!(
        read_auth_file(directory.path())
            .err()
            .expect("directory error")
            .code(),
        AuthFileErrorCode::NotRegularFile
    );

    let path = directory.path().join("auth.json");
    fs::write(
        &path,
        format!(
            r#"{{"auth_mode":"chatgpt","tokens":{{"access_token":"{ACCESS_CANARY}","account_id":" account-with-spaces "}}}}"#
        ),
    )
    .unwrap();
    assert_eq!(
        read_auth_file(&path)
            .err()
            .expect("account validation error")
            .code(),
        AuthFileErrorCode::InvalidAccountId
    );
}

#[cfg(unix)]
#[test]
fn reports_permission_denied_without_echoing_the_candidate_path() {
    use std::os::unix::fs::PermissionsExt as _;

    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("permission-canary-auth.json");
    fs::write(
        &path,
        format!(
            r#"{{"auth_mode":"chatgpt","tokens":{{"access_token":"{ACCESS_CANARY}","account_id":"{ACCOUNT_CANARY}"}}}}"#
        ),
    )
    .expect("write fixture");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).expect("remove read access");

    let error = read_auth_file(&path).err().expect("permission error");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("restore read access");

    assert_eq!(error.code(), AuthFileErrorCode::PermissionDenied);
    let public = serde_json::to_string(&error.public()).expect("serialize public error");
    assert!(!public.contains(path.to_string_lossy().as_ref()));
    assert!(!public.contains(ACCESS_CANARY));
}

#[test]
fn application_boundary_and_sqlite_artifacts_never_expose_auth_canaries() {
    let directory = tempdir().expect("temporary directory");
    let auth_directory = directory.path().join("auth-source");
    let state_directory = directory.path().join("state");
    fs::create_dir_all(&auth_directory).expect("auth directory");
    fs::create_dir_all(&state_directory).expect("state directory");
    let auth_path = auth_directory.join("auth.json");
    fs::write(
        &auth_path,
        format!(
            r#"{{"auth_mode":"chatgpt","tokens":{{"access_token":"{ACCESS_CANARY}","account_id":"{ACCOUNT_CANARY}","id_token":"{ID_TOKEN_CANARY}"}}}}"#
        ),
    )
    .expect("write auth fixture");

    let application = tauri::async_runtime::block_on(async {
        let store = AccountSettingsStore::open(state_directory.join("state.sqlite3"))
            .await
            .expect("open store");
        AccountApplication::new(SettingsManager::new(store, AuthFileReader))
    });
    let response = tauri::async_runtime::block_on(application.select_account(0, &auth_path))
        .expect("configure account");
    let serialized = serde_json::to_vec(&response).expect("serialize command response");
    assert_canaries_absent(&serialized);
    drop(application);

    for entry in fs::read_dir(&state_directory).expect("list sqlite artifacts") {
        let path = entry.expect("artifact entry").path();
        if path.is_file() {
            assert_canaries_absent(&fs::read(path).expect("read sqlite artifact"));
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct FileSnapshot {
    hash: Vec<u8>,
    permissions: fs::Permissions,
    modified: std::time::SystemTime,
}

fn snapshot(path: &std::path::Path) -> FileSnapshot {
    let metadata = fs::metadata(path).expect("fixture metadata");
    FileSnapshot {
        hash: Sha256::digest(fs::read(path).expect("fixture bytes")).to_vec(),
        permissions: metadata.permissions(),
        modified: metadata.modified().expect("modified time"),
    }
}

fn assert_canaries_absent(bytes: &[u8]) {
    for canary in [ACCESS_CANARY, ACCOUNT_CANARY, ID_TOKEN_CANARY] {
        assert!(
            !bytes
                .windows(canary.len())
                .any(|candidate| candidate == canary.as_bytes()),
            "secret canary crossed a public boundary"
        );
    }
}
