use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use quotatide_core::{AccountSettingsStore, Clock};
use serde::Serialize;
use tauri::{AppHandle, Emitter as _};
use tauri_plugin_updater::{Update, UpdaterExt as _};
use tokio::sync::Notify;

use crate::SystemClock;

const UPDATE_STATE_EVENT: &str = "quotatide://update-state";
const FIRST_CHECK_DELAY_MS: i64 = 60 * 1000;
const DAILY_CHECK_INTERVAL_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available,
    Installing,
    Error,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicUpdateState {
    pub status: UpdateStatus,
    pub current_version: String,
    pub available_version: Option<String>,
    pub notes: Option<String>,
    pub last_checked_at_unix_ms: Option<i64>,
    pub error_code: Option<&'static str>,
}

struct Inner {
    public: PublicUpdateState,
    pending: Option<Update>,
}

struct Shared {
    inner: Mutex<Inner>,
    checking: AtomicBool,
    installing: AtomicBool,
    cancelled: AtomicBool,
    wake: Notify,
    first_check_not_before_unix_ms: i64,
}

#[derive(Clone)]
pub struct UpdateRuntime {
    shared: Arc<Shared>,
}

impl UpdateRuntime {
    #[must_use]
    pub fn new(current_version: &str) -> Self {
        Self::new_at(current_version, SystemClock.now_unix_ms())
    }

    fn new_at(current_version: &str, now_unix_ms: i64) -> Self {
        Self {
            shared: Arc::new(Shared {
                inner: Mutex::new(Inner {
                    public: PublicUpdateState {
                        current_version: current_version.to_owned(),
                        ..PublicUpdateState::default()
                    },
                    pending: None,
                }),
                checking: AtomicBool::new(false),
                installing: AtomicBool::new(false),
                cancelled: AtomicBool::new(false),
                wake: Notify::new(),
                first_check_not_before_unix_ms: now_unix_ms.saturating_add(FIRST_CHECK_DELAY_MS),
            }),
        }
    }

    #[must_use]
    pub fn public_state(&self) -> PublicUpdateState {
        self.shared
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .public
            .clone()
    }

    pub fn wake(&self) {
        self.shared.wake.notify_one();
    }

    pub fn cancel(&self) {
        self.shared.cancelled.store(true, Ordering::Release);
        self.wake();
    }

    fn publish(&self, app: &AppHandle) -> PublicUpdateState {
        let public = self.public_state();
        let _ = app.emit(UPDATE_STATE_EVENT, &public);
        public
    }

    fn start_check(&self, app: &AppHandle) -> Option<CheckGuard> {
        if !self.claim_check() {
            return None;
        }
        {
            let mut inner = self
                .shared
                .inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            inner.public.status = UpdateStatus::Checking;
            inner.public.error_code = None;
        }
        self.publish(app);
        Some(CheckGuard(self.clone()))
    }

    fn claim_check(&self) -> bool {
        if self.shared.installing.load(Ordering::Acquire)
            || self
                .shared
                .checking
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return false;
        }
        if self.shared.installing.load(Ordering::Acquire) {
            self.shared.checking.store(false, Ordering::Release);
            return false;
        }
        true
    }

    fn claim_install(&self) -> bool {
        if self.shared.checking.load(Ordering::Acquire)
            || self
                .shared
                .installing
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return false;
        }
        if self.shared.checking.load(Ordering::Acquire) {
            self.shared.installing.store(false, Ordering::Release);
            return false;
        }
        true
    }

    fn seconds_until_next_check(&self, now_unix_ms: i64) -> Duration {
        let last_checked = self.public_state().last_checked_at_unix_ms;
        let remaining_ms = last_checked.map_or_else(
            || {
                self.shared
                    .first_check_not_before_unix_ms
                    .saturating_sub(now_unix_ms)
                    .max(1)
            },
            |last_checked| {
                last_checked
                    .saturating_add(DAILY_CHECK_INTERVAL_MS)
                    .saturating_sub(now_unix_ms)
                    .max(1)
            },
        );
        Duration::from_millis(u64::try_from(remaining_ms).unwrap_or(u64::MAX))
    }

    fn is_due(&self, now_unix_ms: i64) -> bool {
        self.public_state().last_checked_at_unix_ms.map_or(
            now_unix_ms >= self.shared.first_check_not_before_unix_ms,
            |last| now_unix_ms.saturating_sub(last) >= DAILY_CHECK_INTERVAL_MS,
        )
    }
}

struct CheckGuard(UpdateRuntime);

impl Drop for CheckGuard {
    fn drop(&mut self) {
        self.0.shared.checking.store(false, Ordering::Release);
    }
}

pub async fn check_for_update(app: &AppHandle, runtime: &UpdateRuntime) -> PublicUpdateState {
    let Some(_guard) = runtime.start_check(app) else {
        return runtime.public_state();
    };
    let now = SystemClock.now_unix_ms();
    let result = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(error) => Err(error),
    };
    {
        let mut inner = runtime
            .shared
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        inner.public.last_checked_at_unix_ms = Some(now);
        match result {
            Ok(Some(update)) => {
                inner.public.status = UpdateStatus::Available;
                inner.public.available_version = Some(update.version.clone());
                inner.public.notes.clone_from(&update.body);
                inner.public.error_code = None;
                inner.pending = Some(update);
            }
            Ok(None) => {
                inner.public.status = UpdateStatus::UpToDate;
                inner.public.available_version = None;
                inner.public.notes = None;
                inner.public.error_code = None;
                inner.pending = None;
            }
            Err(_) => {
                inner.public.status = UpdateStatus::Error;
                inner.public.error_code = Some("update_check_failed");
            }
        }
    }
    runtime.publish(app)
}

pub async fn run_scheduler(app: AppHandle, runtime: UpdateRuntime, store: AccountSettingsStore) {
    loop {
        let auto_update_enabled = matches!(
            store.public_atomic_settings().await,
            Ok(settings) if settings.auto_update_enabled
        );
        if !auto_update_enabled {
            runtime.shared.wake.notified().await;
            if runtime.shared.cancelled.load(Ordering::Acquire) {
                return;
            }
            continue;
        }

        let delay = runtime.seconds_until_next_check(SystemClock.now_unix_ms());
        tokio::select! {
            () = tokio::time::sleep(delay) => {}
            () = runtime.shared.wake.notified() => {}
        }
        if runtime.shared.cancelled.load(Ordering::Acquire) {
            return;
        }
        let due = runtime.is_due(SystemClock.now_unix_ms());
        let still_enabled = matches!(
            store.public_atomic_settings().await,
            Ok(settings) if settings.auto_update_enabled
        );
        if due && still_enabled {
            let _ = check_for_update(&app, &runtime).await;
        }
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri injects managed state by value.
pub fn get_update_state(runtime: tauri::State<'_, UpdateRuntime>) -> PublicUpdateState {
    runtime.public_state()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri injects AppHandle and state by value.
pub async fn request_update_check(
    app: AppHandle,
    runtime: tauri::State<'_, UpdateRuntime>,
) -> Result<PublicUpdateState, &'static str> {
    Ok(check_for_update(&app, &runtime).await)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri injects AppHandle and state by value.
pub async fn install_pending_update(
    app: AppHandle,
    runtime: tauri::State<'_, UpdateRuntime>,
) -> Result<PublicUpdateState, &'static str> {
    if !runtime.claim_install() {
        return Ok(runtime.public_state());
    }
    let update = {
        let mut inner = runtime
            .shared
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        inner.public.status = UpdateStatus::Installing;
        inner.public.error_code = None;
        inner.pending.take()
    };
    runtime.publish(&app);
    let Some(update) = update else {
        runtime.shared.installing.store(false, Ordering::Release);
        {
            let mut inner = runtime
                .shared
                .inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            inner.public.status = UpdateStatus::Error;
            inner.public.error_code = Some("no_pending_update");
        }
        return Ok(runtime.publish(&app));
    };

    let result = update.download_and_install(|_, _| {}, || {}).await;
    runtime.shared.installing.store(false, Ordering::Release);
    if result.is_err() {
        {
            let mut inner = runtime
                .shared
                .inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            inner.public.status = UpdateStatus::Available;
            inner.public.error_code = Some("update_install_failed");
            inner.pending = Some(update);
        }
        return Ok(runtime.publish(&app));
    }

    app.request_restart();
    Ok(runtime.public_state())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_check_waits_sixty_seconds_and_later_checks_wait_a_day() {
        let runtime = UpdateRuntime::new_at("0.1.0", 1_000);
        assert_eq!(
            runtime.seconds_until_next_check(1_000),
            Duration::from_secs(60)
        );
        assert!(!runtime.is_due(1_000 + FIRST_CHECK_DELAY_MS - 1));
        assert!(runtime.is_due(1_000 + FIRST_CHECK_DELAY_MS));
        runtime
            .shared
            .inner
            .lock()
            .expect("update state")
            .public
            .last_checked_at_unix_ms = Some(1_000);
        assert_eq!(
            runtime.seconds_until_next_check(1_000),
            Duration::from_secs(24 * 60 * 60)
        );
    }

    #[test]
    fn overdue_check_wakes_once_without_backlog() {
        let runtime = UpdateRuntime::new_at("0.1.0", 1_000);
        runtime
            .shared
            .inner
            .lock()
            .expect("update state")
            .public
            .last_checked_at_unix_ms = Some(1_000);
        assert_eq!(
            runtime.seconds_until_next_check(1_000 + DAILY_CHECK_INTERVAL_MS + 50),
            Duration::from_millis(1)
        );
    }

    #[test]
    fn check_and_install_claims_are_mutually_exclusive_without_stuck_flags() {
        let runtime = UpdateRuntime::new_at("0.1.0", 0);
        assert!(runtime.claim_check());
        assert!(!runtime.claim_install());
        assert!(!runtime.shared.installing.load(Ordering::Acquire));
        runtime.shared.checking.store(false, Ordering::Release);

        assert!(runtime.claim_install());
        assert!(!runtime.claim_check());
        assert!(!runtime.shared.checking.load(Ordering::Acquire));
        runtime.shared.installing.store(false, Ordering::Release);
        assert!(runtime.claim_check());
    }
}
