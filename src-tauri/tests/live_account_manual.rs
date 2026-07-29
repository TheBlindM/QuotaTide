use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use quotatide_core::{
    AccountSettingsStore, Clock, RefreshCoordinator, RefreshOutcome, RefreshTrigger,
};
use quotatide_lib::auth_file::read_auth_file;
use quotatide_lib::codex_usage::{CodexUsageClient, ConfiguredCodexUsageSource};

#[derive(Clone, Copy)]
struct FixedClock(i64);

impl Clock for FixedClock {
    fn now_unix_ms(&self) -> i64 {
        self.0
    }
}

#[tokio::test]
#[ignore = "requires QUOTATIDE_AUTH_JSON and live Codex network access"]
async fn fetches_one_strict_current_seven_day_observation() {
    let path = std::env::var_os("QUOTATIDE_AUTH_JSON")
        .map(PathBuf::from)
        .expect("set QUOTATIDE_AUTH_JSON to a Codex-managed auth.json");
    let captured_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .expect("system clock after epoch");
    let material = read_auth_file(&path).expect("read selected auth file");
    let directory = tempfile::tempdir().expect("temporary state directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open temporary settings store");
    store
        .configure_account(
            0,
            material
                .canonical_path()
                .to_str()
                .expect("Unicode canonical auth path"),
            material.account_id(),
        )
        .await
        .expect("configure current account");
    let coordinator = RefreshCoordinator::new(
        store.clone(),
        ConfiguredCodexUsageSource::new(
            store.clone(),
            CodexUsageClient::new().expect("build fixed-origin client"),
        ),
        FixedClock(captured_at_unix_ms),
    );

    let receipt = coordinator
        .refresh(RefreshTrigger::Startup)
        .await
        .expect("fetch current weekly usage");
    assert_eq!(
        receipt.outcome,
        RefreshOutcome::Updated,
        "live source did not return a current weekly observation"
    );
    let observation = store
        .public_live_quota(captured_at_unix_ms)
        .await
        .expect("read current weekly usage")
        .expect("successful live observation");

    assert_eq!(observation.captured_at_unix_ms, Some(captured_at_unix_ms));
    assert_eq!(
        observation
            .window_ends_at_unix_s
            .zip(observation.window_starts_at_unix_s)
            .map(|(end, start)| end - start),
        Some(604_800)
    );
    assert!(observation.resets_at_unix_s.is_some_and(|reset| reset > 0));
    assert!(
        observation
            .used_micropoints
            .is_some_and(|used| used <= 100_000_000)
    );
}
