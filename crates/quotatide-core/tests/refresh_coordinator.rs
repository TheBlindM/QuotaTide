use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

use quotatide_core::{
    AccountApplication, AccountSettingsStore, Application, AuthCandidateValidator, Clock,
    PublicError, QuotaUnits, RefreshAccountBinding, RefreshCoordinator, RefreshOutcome,
    RefreshTrigger, SettingsManager, UsageRefreshAttempt, UsageRefreshSource, UsageSourceError,
    UsageSourceErrorCode, ValidatedAccountCandidate, WeeklyUsageObservation,
};
use tempfile::tempdir;
use tokio::sync::Notify;

#[derive(Clone)]
struct UnusedValidator;

impl AuthCandidateValidator for UnusedValidator {
    type Error = std::convert::Infallible;

    fn validate(&self, _path: &std::path::Path) -> Result<ValidatedAccountCandidate, Self::Error> {
        unreachable!("scheduler test does not configure accounts")
    }

    fn public_error(error: &Self::Error) -> PublicError {
        match *error {}
    }
}

#[derive(Clone)]
struct FakeClock(Arc<AtomicI64>);

impl FakeClock {
    fn new(now: i64) -> Self {
        Self(Arc::new(AtomicI64::new(now)))
    }

    fn set(&self, now: i64) {
        self.0.store(now, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now_unix_ms(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Clone)]
struct FakeSource {
    calls: Arc<AtomicUsize>,
    release: Option<Arc<Notify>>,
    result: Result<WeeklyUsageObservation, UsageSourceErrorCode>,
}

impl FakeSource {
    fn successful(captured_at: i64) -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            release: None,
            result: Ok(observation(captured_at)),
        }
    }

    fn failed(error: UsageSourceErrorCode) -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            release: None,
            result: Err(error),
        }
    }
}

impl UsageRefreshSource for FakeSource {
    fn fetch(
        &self,
        captured_at_unix_ms: i64,
    ) -> impl std::future::Future<Output = UsageRefreshAttempt> + Send {
        let calls = Arc::clone(&self.calls);
        let release = self.release.clone();
        let mut result = self.result.clone();
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            if let Some(release) = release {
                release.notified().await;
            }
            if let Ok(observation) = &mut result {
                observation.captured_at_unix_ms = captured_at_unix_ms;
            }
            let binding =
                RefreshAccountBinding::selected(1, std::path::PathBuf::from("/chosen/auth.json"))
                    .with_account_id("account-one".to_owned());
            match result {
                Ok(observation) => UsageRefreshAttempt::success(binding, observation),
                Err(error) => {
                    UsageRefreshAttempt::failure(Some(binding), UsageSourceError::new(error))
                }
            }
        }
    }
}

fn observation(captured_at_unix_ms: i64) -> WeeklyUsageObservation {
    WeeklyUsageObservation {
        captured_at_unix_ms,
        used: QuotaUnits::from_micropoints(20_000_000).expect("valid quota"),
        window_seconds: 604_800,
        resets_at_unix_s: 1_786_000_000,
        plan_type: Some("plus".to_owned()),
        allowed: Some(true),
    }
}

async fn configured_store() -> (tempfile::TempDir, AccountSettingsStore) {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("state.sqlite3");
    let store = AccountSettingsStore::open(path).await.expect("open store");
    store
        .configure_account(0, "/chosen/auth.json", "account-one")
        .await
        .expect("configure account");
    (directory, store)
}

#[tokio::test]
async fn concurrent_triggers_share_one_source_attempt_and_result() {
    let (_directory, store) = configured_store().await;
    let release = Arc::new(Notify::new());
    let mut source = FakeSource::successful(1_785_000_000_000);
    source.release = Some(Arc::clone(&release));
    let calls_for_release = Arc::clone(&source.calls);
    let calls_for_assert = Arc::clone(&source.calls);
    let coordinator = RefreshCoordinator::new(store, source, FakeClock::new(1_785_000_000_000));
    let release_task = tokio::spawn(async move {
        while calls_for_release.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        release.notify_waiters();
    });

    let (startup, hourly, manual) = tokio::join!(
        coordinator.refresh(RefreshTrigger::Startup),
        coordinator.refresh(RefreshTrigger::Hourly),
        coordinator.refresh(RefreshTrigger::Manual),
    );
    release_task.await.expect("release task");

    assert_eq!(calls_for_assert.load(Ordering::SeqCst), 1);
    assert_eq!(
        startup.as_ref().expect("startup"),
        hourly.as_ref().expect("hourly")
    );
    assert_eq!(
        startup.as_ref().expect("startup"),
        manual.as_ref().expect("manual")
    );
}

#[tokio::test]
async fn a_manual_trigger_joining_an_hourly_flight_starts_its_cooldown() {
    let (_directory, store) = configured_store().await;
    let release = Arc::new(Notify::new());
    let mut source = FakeSource::successful(1_785_000_000_000);
    source.release = Some(Arc::clone(&release));
    let calls_for_release = Arc::clone(&source.calls);
    let calls_for_assert = Arc::clone(&source.calls);
    let clock = FakeClock::new(1_785_000_000_000);
    let coordinator = RefreshCoordinator::new(store, source, clock);
    let release_task = tokio::spawn(async move {
        while calls_for_release.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        tokio::task::yield_now().await;
        release.notify_waiters();
    });

    let (hourly, manual) = tokio::join!(
        coordinator.refresh(RefreshTrigger::Hourly),
        coordinator.refresh(RefreshTrigger::Manual),
    );
    release_task.await.expect("release task");
    let cooled = coordinator
        .refresh(RefreshTrigger::Manual)
        .await
        .expect("cooled manual");

    assert_eq!(hourly.expect("hourly"), manual.expect("manual"));
    assert_eq!(cooled.outcome, RefreshOutcome::ManualCooldown);
    assert_eq!(calls_for_assert.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn manual_refresh_has_a_thirty_second_coordinator_cooldown() {
    let (_directory, store) = configured_store().await;
    let source = FakeSource::successful(1_785_000_000_000);
    let calls = Arc::clone(&source.calls);
    let clock = FakeClock::new(1_785_000_000_000);
    let coordinator = RefreshCoordinator::new(store, source, clock.clone());

    let first = coordinator
        .refresh(RefreshTrigger::Manual)
        .await
        .expect("first manual");
    clock.set(1_785_000_029_999);
    let cooled = coordinator
        .refresh(RefreshTrigger::Manual)
        .await
        .expect("cooled manual");
    clock.set(1_785_000_030_000);
    let next = coordinator
        .refresh(RefreshTrigger::Manual)
        .await
        .expect("next manual");

    assert_eq!(first.outcome, RefreshOutcome::Updated);
    assert_eq!(cooled.outcome, RefreshOutcome::ManualCooldown);
    assert_eq!(next.outcome, RefreshOutcome::Updated);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn resume_runs_once_only_after_the_hourly_deadline() {
    let (_directory, store) = configured_store().await;
    let source = FakeSource::successful(1_785_000_000_000);
    let calls = Arc::clone(&source.calls);
    let clock = FakeClock::new(1_785_000_000_000);
    let coordinator = RefreshCoordinator::new(store, source, clock.clone());
    coordinator
        .refresh(RefreshTrigger::Startup)
        .await
        .expect("startup");

    clock.set(1_785_003_599_999);
    let early = coordinator
        .refresh_if_due(RefreshTrigger::Resume, 3_600_000)
        .await
        .expect("early resume");
    clock.set(1_785_003_600_000);
    let due = coordinator
        .refresh_if_due(RefreshTrigger::Resume, 3_600_000)
        .await
        .expect("due resume");

    assert_eq!(early.outcome, RefreshOutcome::NotDue);
    assert_eq!(due.outcome, RefreshOutcome::Updated);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn timeout_is_persisted_as_public_health_without_discarding_the_snapshot() {
    let (_directory, store) = configured_store().await;
    store
        .record_usage_success(
            &RefreshAccountBinding::selected(1, std::path::PathBuf::from("/chosen/auth.json"))
                .with_account_id("account-one".to_owned()),
            observation(1_785_000_000_000),
        )
        .await
        .expect("seed last-known-good");
    let clock = FakeClock::new(1_785_003_600_000);
    let coordinator = RefreshCoordinator::new(
        store.clone(),
        FakeSource::failed(UsageSourceErrorCode::Timeout),
        clock,
    );

    let receipt = coordinator
        .refresh(RefreshTrigger::Hourly)
        .await
        .expect("timeout is a source outcome");
    let public = store
        .public_live_quota(1_785_003_600_000)
        .await
        .expect("public quota")
        .expect("last-known-good");

    assert_eq!(
        receipt.outcome,
        RefreshOutcome::Failed(UsageSourceErrorCode::Timeout)
    );
    assert_eq!(public.used_micropoints, Some(20_000_000));
    assert_eq!(public.consecutive_failures, 1);
    assert_eq!(public.public_error, Some(UsageSourceErrorCode::Timeout));
}

#[tokio::test]
async fn a_storage_failure_retains_its_internal_source_chain() {
    let (directory, store) = configured_store().await;
    {
        let connection =
            tokio_rusqlite::rusqlite::Connection::open(directory.path().join("state.sqlite3"))
                .expect("open fault injector");
        connection
            .execute_batch(
                "CREATE TRIGGER reject_coordinator_health
                 BEFORE INSERT ON usage_source_health
                 BEGIN
                   SELECT RAISE(ABORT, 'injected coordinator failure');
                 END;",
            )
            .expect("install fault injector");
    }
    let coordinator = RefreshCoordinator::new(
        store,
        FakeSource::successful(1_785_000_000_000),
        FakeClock::new(1_785_000_000_000),
    );

    let error = coordinator
        .refresh(RefreshTrigger::Startup)
        .await
        .expect_err("storage failure");

    assert!(std::error::Error::source(&error).is_some());
}

#[tokio::test(start_paused = true)]
async fn resume_resets_the_next_hourly_deadline_and_shutdown_cancels_the_actor() {
    let (_directory, store) = configured_store().await;
    let source = FakeSource::successful(1_785_000_000_000);
    let calls = Arc::clone(&source.calls);
    let clock = FakeClock::new(1_785_000_000_000);
    let coordinator = RefreshCoordinator::new(store.clone(), source, clock.clone());
    let application = Application::new(
        AccountApplication::new(SettingsManager::new(store, UnusedValidator)),
        coordinator,
    );
    let callbacks = Arc::new(AtomicUsize::new(0));
    let callbacks_for_task = Arc::clone(&callbacks);
    let application_for_task = application.clone();
    let scheduler = tokio::spawn(async move {
        application_for_task
            .run_hourly_scheduler(true, move || {
                callbacks_for_task.fetch_add(1, Ordering::SeqCst);
            })
            .await;
    });

    wait_for_count(&calls, 1).await;
    clock.set(1_785_001_800_000);
    tokio::time::advance(std::time::Duration::from_secs(30 * 60)).await;
    application.notify_resume();
    wait_for_count(&callbacks, 2).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    clock.set(1_785_003_600_000);
    tokio::time::advance(std::time::Duration::from_secs(30 * 60)).await;
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    clock.set(1_785_005_400_000);
    tokio::time::advance(std::time::Duration::from_secs(30 * 60)).await;
    wait_for_count(&calls, 2).await;

    application.cancel_scheduler();
    scheduler.await.expect("scheduler shutdown");
}

async fn wait_for_count(counter: &AtomicUsize, expected: usize) {
    while counter.load(Ordering::SeqCst) < expected {
        tokio::task::yield_now().await;
    }
}
