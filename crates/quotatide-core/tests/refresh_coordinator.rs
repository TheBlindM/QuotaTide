use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

use quotatide_core::{
    AccountApplication, AccountSettingsStore, Application, AuthCandidateValidator, Clock,
    CurrentUsageAuth, PublicError, QuotaUnits, RadarAnnouncement, RadarObservation, RadarSnapshot,
    RadarSourceError, RadarSourceErrorCode, RefreshAccountBinding, RefreshCoordinator,
    RefreshOutcome, RefreshTrigger, ResetRadarSource, SettingsManager, UsageAuthReadFailure,
    UsageRefreshSource, UsageSourceError, UsageSourceErrorCode, ValidatedAccountCandidate,
    WeeklyUsageObservation,
};
use tempfile::tempdir;
use tokio::sync::{Barrier, Notify};

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

#[derive(Clone)]
struct SwitchingSource {
    revision: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
    release_first: Arc<Notify>,
}

#[derive(Clone)]
struct BarrierUsageSource {
    barrier: Arc<Barrier>,
}

#[derive(Clone)]
struct MissingAccountSource;

impl UsageRefreshSource for MissingAccountSource {
    type AuthMaterial = ();

    async fn read_current_auth(
        &self,
    ) -> Result<CurrentUsageAuth<Self::AuthMaterial>, UsageAuthReadFailure> {
        Err(UsageAuthReadFailure::new(
            None,
            UsageSourceError::new(UsageSourceErrorCode::AuthPathUnavailable),
        ))
    }

    async fn fetch_usage<'a>(
        &'a self,
        _auth: &'a Self::AuthMaterial,
        _captured_at_unix_ms: i64,
    ) -> Result<WeeklyUsageObservation, UsageSourceError> {
        unreachable!("missing account never reaches the usage endpoint")
    }
}

impl UsageRefreshSource for BarrierUsageSource {
    type AuthMaterial = ();

    async fn read_current_auth(
        &self,
    ) -> Result<CurrentUsageAuth<Self::AuthMaterial>, UsageAuthReadFailure> {
        Ok(CurrentUsageAuth::new(
            RefreshAccountBinding::selected(1, std::path::PathBuf::from("/chosen/auth.json"))
                .with_account_id("account-one".to_owned()),
            (),
            [1; 32],
        ))
    }

    async fn fetch_usage<'a>(
        &'a self,
        _auth: &'a Self::AuthMaterial,
        captured_at_unix_ms: i64,
    ) -> Result<WeeklyUsageObservation, UsageSourceError> {
        self.barrier.wait().await;
        Ok(observation(captured_at_unix_ms))
    }
}

struct BarrierRadarSource {
    barrier: Arc<Barrier>,
    result: Result<RadarSnapshot, RadarSourceError>,
}

impl ResetRadarSource for BarrierRadarSource {
    fn fetch_radar(
        &self,
        _attempted_at_unix_ms: i64,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<RadarSnapshot, RadarSourceError>> + Send + '_>,
    > {
        Box::pin(async move {
            self.barrier.wait().await;
            self.result.clone()
        })
    }
}

impl UsageRefreshSource for SwitchingSource {
    type AuthMaterial = usize;

    async fn read_current_auth(
        &self,
    ) -> Result<CurrentUsageAuth<Self::AuthMaterial>, UsageAuthReadFailure> {
        let revision = self.revision.load(Ordering::SeqCst);
        let account = if revision == 1 {
            "account-one"
        } else {
            "account-two"
        };
        Ok(CurrentUsageAuth::new(
            RefreshAccountBinding::selected(
                u32::try_from(revision).expect("test revision"),
                std::path::PathBuf::from("/chosen/auth.json"),
            )
            .with_account_id(account.to_owned()),
            revision,
            [u8::try_from(revision).expect("test fingerprint"); 32],
        ))
    }

    async fn fetch_usage<'a>(
        &'a self,
        _auth: &'a Self::AuthMaterial,
        captured_at_unix_ms: i64,
    ) -> Result<WeeklyUsageObservation, UsageSourceError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            self.release_first.notified().await;
        }
        Ok(observation(captured_at_unix_ms))
    }
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
    type AuthMaterial = ();

    async fn read_current_auth(
        &self,
    ) -> Result<CurrentUsageAuth<Self::AuthMaterial>, UsageAuthReadFailure> {
        Ok(CurrentUsageAuth::new(
            RefreshAccountBinding::selected(1, std::path::PathBuf::from("/chosen/auth.json"))
                .with_account_id("account-one".to_owned()),
            (),
            [1; 32],
        ))
    }

    fn fetch_usage<'a>(
        &'a self,
        _auth: &'a Self::AuthMaterial,
        captured_at_unix_ms: i64,
    ) -> impl std::future::Future<Output = Result<WeeklyUsageObservation, UsageSourceError>> + Send + 'a
    {
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
            result.map_err(UsageSourceError::new)
        }
    }
}

fn observation(captured_at_unix_ms: i64) -> WeeklyUsageObservation {
    WeeklyUsageObservation {
        captured_at_unix_ms,
        used: QuotaUnits::from_micropoints(20_000_000).expect("valid quota"),
        window_seconds: 604_800,
        resets_at_unix_s: 1_785_500_000,
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
    let startup = startup.expect("startup");
    let hourly = hourly.expect("hourly");
    let manual = manual.expect("manual");
    assert_eq!(startup.outcome, hourly.outcome);
    assert_eq!(startup.outcome, manual.outcome);
    assert_eq!(startup.attempted_at_unix_ms, hourly.attempted_at_unix_ms);
    assert_eq!(startup.attempted_at_unix_ms, manual.attempted_at_unix_ms);
    assert_eq!(startup.retry_after_ms, 0);
    assert_eq!(hourly.retry_after_ms, 0);
    assert_eq!(manual.retry_after_ms, 30_000);
}

#[tokio::test]
async fn dashboard_revision_events_invalidate_a_query_owned_refreshing_snapshot() {
    let (_directory, store) = configured_store().await;
    let release = Arc::new(Notify::new());
    let mut source = FakeSource::successful(1_785_000_000_000);
    source.release = Some(Arc::clone(&release));
    let calls = Arc::clone(&source.calls);
    let application = Application::new(
        AccountApplication::new(SettingsManager::new(store.clone(), UnusedValidator)),
        RefreshCoordinator::new(store.clone(), source, FakeClock::new(1_785_000_000_000)),
    );
    let mut changes = application.subscribe_dashboard_changes();
    let refresh_application = application.clone();
    let refresh = tokio::spawn(async move {
        refresh_application
            .refresh(RefreshTrigger::Startup)
            .await
            .expect("startup refresh")
    });

    wait_for_count(&calls, 1).await;
    changes.changed().await.expect("refresh-start revision");
    let started = application
        .live_quota(1_785_000_000_000)
        .await
        .expect("started dashboard state");
    assert_eq!(changes.borrow_and_update().revision, 1);
    assert_eq!(started.dashboard_revision, 1);
    assert!(started.refreshing);
    assert!(started.quota.is_none());

    release.notify_waiters();
    assert_eq!(
        refresh.await.expect("refresh task").outcome,
        RefreshOutcome::Updated
    );
    changes.changed().await.expect("refresh-finish revision");
    let completed = application
        .live_quota(1_785_000_000_000)
        .await
        .expect("completed dashboard state");
    assert_eq!(changes.borrow_and_update().revision, 2);
    assert_eq!(completed.dashboard_revision, 2);
    assert!(!completed.refreshing);
    assert_eq!(
        completed.quota.expect("live quota").used_micropoints,
        Some(20_000_000)
    );

    let restarted = Application::new(
        AccountApplication::new(SettingsManager::new(store.clone(), UnusedValidator)),
        RefreshCoordinator::new(
            store,
            FakeSource::successful(1_785_000_000_000),
            FakeClock::new(1_785_000_000_000),
        ),
    );
    let restored = restarted
        .live_quota(1_785_000_000_000)
        .await
        .expect("restored dashboard state");
    assert_eq!(restored.dashboard_revision, 2);
    assert!(!restored.refreshing);
}

#[tokio::test]
async fn settings_refresh_retries_after_joining_a_superseded_account_flight() {
    let (_directory, store) = configured_store().await;
    let source = SwitchingSource {
        revision: Arc::new(AtomicUsize::new(1)),
        calls: Arc::new(AtomicUsize::new(0)),
        release_first: Arc::new(Notify::new()),
    };
    let calls = Arc::clone(&source.calls);
    let revision = Arc::clone(&source.revision);
    let release = Arc::clone(&source.release_first);
    let clock = FakeClock::new(1_785_000_000_000);
    let application = Application::new(
        AccountApplication::new(SettingsManager::new(store.clone(), UnusedValidator)),
        RefreshCoordinator::new(store.clone(), source, clock),
    );

    let old_application = application.clone();
    let old_refresh = tokio::spawn(async move {
        old_application
            .refresh(RefreshTrigger::Hourly)
            .await
            .expect("old account refresh")
    });
    wait_for_count(&calls, 1).await;

    store
        .configure_account(1, "/chosen/auth.json", "account-two")
        .await
        .expect("switch account");
    revision.store(2, Ordering::SeqCst);
    let selected_application = application.clone();
    let selected_refresh = tokio::spawn(async move {
        selected_application
            .refresh_selected_account()
            .await
            .expect("latest account refresh")
    });
    tokio::task::yield_now().await;
    release.notify_waiters();

    assert_eq!(
        old_refresh.await.expect("old task").outcome,
        RefreshOutcome::Superseded
    );
    assert_eq!(
        selected_refresh.await.expect("settings task").outcome,
        RefreshOutcome::Updated
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        store
            .public_live_quota(1_785_000_000_000)
            .await
            .expect("live quota")
            .expect("new account observation")
            .used_micropoints,
        Some(20_000_000)
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

    let hourly = hourly.expect("hourly");
    let manual = manual.expect("manual");
    assert_eq!(hourly.outcome, manual.outcome);
    assert_eq!(hourly.retry_after_ms, 0);
    assert_eq!(manual.retry_after_ms, 30_000);
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
    assert_eq!(first.retry_after_ms, 30_000);
    assert_eq!(cooled.outcome, RefreshOutcome::ManualCooldown);
    assert_eq!(cooled.retry_after_ms, 1);
    assert_eq!(next.outcome, RefreshOutcome::Updated);
    assert_eq!(next.retry_after_ms, 30_000);
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
async fn codex_and_radar_run_concurrently_and_commit_partial_success_independently() {
    let (_directory, store) = configured_store().await;
    let barrier = Arc::new(Barrier::new(2));
    let coordinator = RefreshCoordinator::new(
        store.clone(),
        BarrierUsageSource {
            barrier: Arc::clone(&barrier),
        },
        FakeClock::new(1_785_000_000_000),
    )
    .with_reset_radar_source(BarrierRadarSource {
        barrier,
        result: Err(RadarSourceError::new(RadarSourceErrorCode::Timeout)),
    });

    let receipt = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        coordinator.refresh(RefreshTrigger::Hourly),
    )
    .await
    .expect("sources must overlap")
    .expect("refresh");

    assert_eq!(receipt.outcome, RefreshOutcome::Updated);
    assert!(
        store
            .public_live_quota(1_785_000_000_000)
            .await
            .expect("quota")
            .is_some()
    );
    let radar = store
        .public_reset_radar(1_785_000_000_000)
        .await
        .expect("radar");
    assert_eq!(radar.public_error, Some(RadarSourceErrorCode::Timeout));
    assert_eq!(radar.consecutive_failures, 1);
    let (dashboard_revision, _, _) = store
        .public_dashboard_snapshot(1_785_000_000_000)
        .await
        .expect("dashboard");
    assert_eq!(
        dashboard_revision, 2,
        "one refresh flight must publish exactly one coherent dashboard revision"
    );
}

#[tokio::test]
async fn a_radar_storage_failure_rolls_back_the_whole_dashboard_revision() {
    let (directory, store) = configured_store().await;
    {
        let connection =
            tokio_rusqlite::rusqlite::Connection::open(directory.path().join("state.sqlite3"))
                .expect("open fault injector");
        connection
            .execute_batch(
                "CREATE TRIGGER reject_radar_health
                 BEFORE INSERT ON radar_source_health
                 BEGIN
                   SELECT RAISE(ABORT, 'injected Radar failure');
                 END;",
            )
            .expect("install fault injector");
    }
    let radar = RadarObservation::new(
        "2081899343091843463",
        7_500,
        1_784_999_000_000,
        1_785_086_400_000,
        "Possible additional reset.",
        "https://x.com/thsottiaux/status/2081899343091843463",
    )
    .expect("radar");
    let coordinator = RefreshCoordinator::new(
        store.clone(),
        FakeSource::successful(1_785_000_000_000),
        FakeClock::new(1_785_000_000_000),
    )
    .with_reset_radar_source(BarrierRadarSource {
        barrier: Arc::new(Barrier::new(1)),
        result: Ok(RadarSnapshot::new(Some(radar), None)),
    });

    assert!(coordinator.refresh(RefreshTrigger::Hourly).await.is_err());
    let (revision, quota, radar) = store
        .public_dashboard_snapshot(1_785_000_000_000)
        .await
        .expect("dashboard");
    assert_eq!(revision, 1);
    assert!(quota.is_none());
    assert!(radar.prediction.is_none());
}

#[tokio::test]
async fn radar_success_survives_a_codex_source_failure() {
    let (_directory, store) = configured_store().await;
    let radar = RadarObservation::new(
        "2081899343091843463",
        7_500,
        1_784_999_000_000,
        1_785_086_400_000,
        "Possible additional reset.",
        "https://x.com/thsottiaux/status/2081899343091843463",
    )
    .expect("radar");
    let coordinator = RefreshCoordinator::new(
        store.clone(),
        FakeSource::failed(UsageSourceErrorCode::RateLimited),
        FakeClock::new(1_785_000_000_000),
    )
    .with_reset_radar_source(BarrierRadarSource {
        barrier: Arc::new(Barrier::new(1)),
        result: Ok(RadarSnapshot::new(Some(radar), None)),
    });

    let receipt = coordinator
        .refresh(RefreshTrigger::Hourly)
        .await
        .expect("refresh");

    assert_eq!(
        receipt.outcome,
        RefreshOutcome::Failed(UsageSourceErrorCode::RateLimited)
    );
    assert!(
        store
            .public_reset_radar(1_785_000_000_000)
            .await
            .expect("radar")
            .prediction
            .is_some()
    );
}

#[tokio::test]
async fn radar_refreshes_without_an_account_and_still_obeys_the_hourly_deadline() {
    let directory = tempdir().expect("temporary directory");
    let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
        .await
        .expect("open store");
    let radar = RadarObservation::new(
        "2081899343091843463",
        7_500,
        1_784_999_000_000,
        1_785_086_400_000,
        "Possible additional reset.",
        "https://x.com/thsottiaux/status/2081899343091843463",
    )
    .expect("radar");
    let coordinator = RefreshCoordinator::new(
        store.clone(),
        MissingAccountSource,
        FakeClock::new(1_785_000_000_000),
    )
    .with_reset_radar_source(BarrierRadarSource {
        barrier: Arc::new(Barrier::new(1)),
        result: Ok(RadarSnapshot::new(Some(radar), None)),
    });

    let first = coordinator
        .refresh(RefreshTrigger::Startup)
        .await
        .expect("startup refresh");
    let early = coordinator
        .refresh_if_due(RefreshTrigger::Resume, 3_600_000)
        .await
        .expect("resume due check");

    assert_eq!(
        first.outcome,
        RefreshOutcome::Failed(UsageSourceErrorCode::AuthPathUnavailable)
    );
    assert_eq!(early.outcome, RefreshOutcome::NotDue);
    assert!(
        store
            .public_reset_radar(1_785_000_000_000)
            .await
            .expect("radar")
            .prediction
            .is_some()
    );
}

#[tokio::test]
async fn a_new_announcement_requests_one_account_only_recheck() {
    let (_directory, store) = configured_store().await;
    let source = FakeSource::successful(1_785_000_000_000);
    let calls = Arc::clone(&source.calls);
    let announcement = RadarAnnouncement::new(
        "2082317452755751098",
        1_784_999_000_000,
        "Usage limits have been reset.",
        "https://x.com/thsottiaux/status/2082317452755751098",
    )
    .expect("announcement");
    let clock = FakeClock::new(1_785_000_000_000);
    let coordinator = RefreshCoordinator::new(store, source, clock.clone())
        .with_reset_radar_source(BarrierRadarSource {
            barrier: Arc::new(Barrier::new(1)),
            result: Ok(RadarSnapshot::new(None, Some(announcement))),
        });

    coordinator
        .refresh(RefreshTrigger::Hourly)
        .await
        .expect("first refresh");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "announcement recheck belongs to the next coordinator round"
    );

    clock.set(1_785_000_000_001);
    coordinator
        .refresh_if_due(RefreshTrigger::Resume, 3_600_000)
        .await
        .expect("pending announcement recheck");
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    coordinator
        .refresh_if_due(RefreshTrigger::Resume, 3_600_000)
        .await
        .expect("same announcement is no longer pending");
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
    let (dashboard_revision, public) = store
        .public_live_quota_snapshot(1_785_003_600_000)
        .await
        .expect("public quota");
    let public = public.expect("last-known-good");

    assert_eq!(
        receipt.outcome,
        RefreshOutcome::Failed(UsageSourceErrorCode::Timeout)
    );
    assert_eq!(public.used_micropoints, Some(20_000_000));
    assert_eq!(public.consecutive_failures, 1);
    assert_eq!(public.public_error, Some(UsageSourceErrorCode::Timeout));
    assert_eq!(dashboard_revision, 3);
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
    let application_for_task = application.clone();
    let scheduler = tokio::spawn(async move {
        application_for_task.run_hourly_scheduler(true).await;
    });

    wait_for_count(&calls, 1).await;
    clock.set(1_785_001_800_000);
    tokio::time::advance(std::time::Duration::from_secs(30 * 60)).await;
    application.notify_resume();
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
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
