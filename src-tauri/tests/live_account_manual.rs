use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use quotatide_core::UsageRefreshSource as _;
use quotatide_lib::codex_usage::{CodexUsageClient, CodexUsageCollector, SelectedAuthFile};

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
    let collector = CodexUsageCollector::new(
        SelectedAuthFile::new(Some(path)),
        CodexUsageClient::new().expect("build fixed-origin client"),
    );

    let observation = collector
        .fetch(captured_at_unix_ms)
        .await
        .expect("fetch current weekly usage");

    assert_eq!(observation.captured_at_unix_ms, captured_at_unix_ms);
    assert_eq!(observation.window_seconds, 604_800);
    assert!(observation.resets_at_unix_s > 0);
    assert!(observation.used.micropoints() <= 100_000_000);
}
