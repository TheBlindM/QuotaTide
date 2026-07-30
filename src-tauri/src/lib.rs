//! `QuotaTide` desktop shell.

pub mod auth_file;
pub mod background_lifecycle;
pub mod codex_usage;
mod credential_vault;
mod platform_notifications;
mod privacy;
pub mod reset_radar;
mod smtp;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use auth_file::AuthFileReader;
use background_lifecycle::{AUTOSTART_ARGUMENT, LaunchMode, notify_secondary, start_primary};
use codex_usage::{CodexUsageClient, ConfiguredCodexUsageSource};
use credential_vault::SystemCredentialVault;
use platform_notifications::{PlatformNotificationError, PlatformNotifier};
use privacy::{SafeLogFields, SafeLogLevel, safe_log};
use quotatide_core::{
    AccountApplication, AccountSettingsStore, AlertChannel, AlertTarget, Application,
    AtomicSettingsManager, AutostartControl, BuildInfo, Clock, DashboardChanged, DeliveryWorker,
    EmailDeliveryWorker, NotificationPermissionStatus, PhysicalRect as CoreRect,
    PhysicalSize as CoreSize, PublicAlertInbox, PublicError, PublicErrorCode, PublicLiveQuotaState,
    PublicSettings, RefreshCoordinator, RefreshTrigger, SafeNotification, SettingsChanged,
    SettingsDraft, SettingsManager, SettingsStoreError, ShellEffect, ShellEvent, SystemNotifier,
    TestEmailError, TrayShell, place_tray_window,
};
use reset_radar::ResetRadarClient;
use smtp::LettreMailTransport;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
use tauri::utils::WindowEffect;
use tauri::utils::config::WindowEffectsConfig;
use tauri::{
    App, AppHandle, Emitter, Manager, PhysicalPosition, Rect, RunEvent, WebviewWindow, WindowEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as _};
use tokio::sync::Notify;

const MAIN_WINDOW_LABEL: &str = "main";
const MAIN_TRAY_ID: &str = "main";
const WINDOW_GAP: f64 = 8.0;
const DASHBOARD_CHANGED_EVENT: &str = "quotatide://dashboard-changed";
const SETTINGS_CHANGED_EVENT: &str = "quotatide://settings-changed";
const NOTIFICATION_OPENED_EVENT: &str = "quotatide://notification-opened";
const ALERTS_CHANGED_EVENT: &str = "quotatide://alerts-changed";
static NOTIFICATION_ACTIVATION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum StartupMode {
    Ready,
    RecoveryRequired,
    UnsupportedSchema,
    StoragePermissionDenied,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicStartupState {
    mode: StartupMode,
    message_key: &'static str,
    recovered_from_backup: bool,
}

impl PublicStartupState {
    const fn ready(recovered_from_backup: bool) -> Self {
        Self {
            mode: StartupMode::Ready,
            message_key: "startup.ready",
            recovered_from_backup,
        }
    }

    const fn recovery_required() -> Self {
        Self {
            mode: StartupMode::RecoveryRequired,
            message_key: "startup.recovery_required",
            recovered_from_backup: false,
        }
    }

    const fn unsupported_schema() -> Self {
        Self {
            mode: StartupMode::UnsupportedSchema,
            message_key: "startup.unsupported_schema",
            recovered_from_backup: false,
        }
    }

    const fn storage_permission_denied() -> Self {
        Self {
            mode: StartupMode::StoragePermissionDenied,
            message_key: "startup.storage_permission_denied",
            recovered_from_backup: false,
        }
    }
}

#[derive(Debug, Clone)]
struct LocalDataContext {
    app_data: PathBuf,
}

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct NotificationOpened {
    target: AlertTarget,
    activation_id: u64,
}

#[derive(Debug, Default)]
struct DesktopShell {
    shell: TrayShell,
    last_tray_rect: Option<Rect>,
}

type SharedDesktopShell = Mutex<DesktopShell>;

#[derive(Debug, Clone, Copy)]
struct SystemClock;

impl Clock for SystemClock {
    fn now_unix_ms(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| {
                i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
            })
    }
}

type LiveApplication = Application<AuthFileReader, ConfiguredCodexUsageSource, SystemClock>;
type LiveAtomicSettings =
    AtomicSettingsManager<AuthFileReader, SystemAutostart, SystemCredentialVault>;
type LiveDeliveryWorker = DeliveryWorker<DesktopSystemNotifier>;
type LiveEmailDeliveryWorker = EmailDeliveryWorker<SystemCredentialVault, LettreMailTransport>;

#[derive(Clone, Default)]
struct SharedNotificationPermission {
    status: Arc<AtomicU8>,
}

impl SharedNotificationPermission {
    fn new(status: NotificationPermissionStatus) -> Self {
        let permission = Self::default();
        permission.set(status);
        permission
    }

    fn get(&self) -> NotificationPermissionStatus {
        match self.status.load(Ordering::Acquire) {
            1 => NotificationPermissionStatus::Granted,
            2 => NotificationPermissionStatus::Denied,
            3 => NotificationPermissionStatus::Error,
            _ => NotificationPermissionStatus::Unknown,
        }
    }

    fn set(&self, status: NotificationPermissionStatus) {
        let value = match status {
            NotificationPermissionStatus::Unknown => 0,
            NotificationPermissionStatus::Granted => 1,
            NotificationPermissionStatus::Denied => 2,
            NotificationPermissionStatus::Error => 3,
        };
        self.status.store(value, Ordering::Release);
    }
}

#[derive(Clone, Default)]
struct DeliveryWorkerLifecycle {
    state: Arc<DeliveryWorkerState>,
}

#[derive(Default)]
struct DeliveryWorkerState {
    started: AtomicBool,
    cancelled: AtomicBool,
    wake: Notify,
    worker: Mutex<Option<LiveDeliveryWorker>>,
    email_worker: Mutex<Option<LiveEmailDeliveryWorker>>,
    event_app: Mutex<Option<AppHandle>>,
}

impl DeliveryWorkerLifecycle {
    fn configure(
        &self,
        worker: LiveDeliveryWorker,
        email_worker: LiveEmailDeliveryWorker,
        app: AppHandle,
    ) -> bool {
        let configured = self
            .state
            .worker
            .lock()
            .is_ok_and(|mut configured| configured.replace(worker).is_none());
        let email_configured = self
            .state
            .email_worker
            .lock()
            .is_ok_and(|mut configured| configured.replace(email_worker).is_none());
        if configured
            && email_configured
            && let Ok(mut event_app) = self.state.event_app.lock()
        {
            *event_app = Some(app);
        }
        configured && email_configured
    }

    fn start(&self) -> bool {
        if self
            .state
            .started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        let worker = self.clone();
        tauri::async_runtime::spawn(async move {
            worker.run().await;
        });
        true
    }

    async fn run(self) {
        while !self.state.cancelled.load(Ordering::Acquire) {
            let system_worker = self
                .state
                .worker
                .lock()
                .ok()
                .and_then(|configured| configured.clone());
            let email_worker = self
                .state
                .email_worker
                .lock()
                .ok()
                .and_then(|configured| configured.clone());
            let now = SystemClock.now_unix_ms();
            let system_sweep = async {
                match system_worker {
                    Some(worker) => worker.deliver_pending(now).await.map(Some),
                    None => Ok(None),
                }
            };
            let email_sweep = async {
                match email_worker {
                    Some(worker) => worker.deliver_pending(now).await.map(Some),
                    None => Ok(None),
                }
            };
            let (system_result, email_result) = tokio::join!(system_sweep, email_sweep);
            match system_result {
                Ok(Some(sweep)) if sweep != quotatide_core::DeliverySweep::default() => {
                    if let Ok(event_app) = self.state.event_app.lock()
                        && let Some(app) = event_app.as_ref()
                    {
                        let _ = app.emit(ALERTS_CHANGED_EVENT, ());
                    }
                }
                Ok(_) => {}
                Err(_) => {
                    safe_log(
                        SafeLogLevel::Error,
                        "delivery",
                        "system_sweep",
                        SafeLogFields {
                            public_error_code: Some("delivery_failed"),
                            ..SafeLogFields::default()
                        },
                    );
                }
            }
            match email_result {
                Ok(Some(sweep)) if sweep != quotatide_core::DeliverySweep::default() => {
                    if let Ok(event_app) = self.state.event_app.lock()
                        && let Some(app) = event_app.as_ref()
                    {
                        let _ = app.emit(ALERTS_CHANGED_EVENT, ());
                    }
                }
                Ok(_) => {}
                Err(_) => {
                    safe_log(
                        SafeLogLevel::Error,
                        "delivery",
                        "email_sweep",
                        SafeLogFields {
                            public_error_code: Some("email_delivery_failed"),
                            ..SafeLogFields::default()
                        },
                    );
                }
            }
            tokio::select! {
                () = self.state.wake.notified() => {}
                () = tokio::time::sleep(std::time::Duration::from_secs(60)) => {}
            }
        }
    }

    fn wake(&self) {
        self.state.wake.notify_one();
    }

    fn cancel(&self) {
        self.state.cancelled.store(true, Ordering::Release);
        self.state.wake.notify_one();
    }
}

#[derive(Clone)]
struct DesktopSystemNotifier {
    platform: PlatformNotifier,
    permission: SharedNotificationPermission,
}

impl SystemNotifier for DesktopSystemNotifier {
    type Error = PlatformNotificationError;

    async fn permission_state(&self) -> Result<NotificationPermissionStatus, Self::Error> {
        let persisted = self.permission.get();
        if persisted != NotificationPermissionStatus::Granted {
            return Ok(persisted);
        }
        self.platform.permission_state().await.inspect(|status| {
            self.permission.set(*status);
        })
    }

    async fn notify(&self, notification: SafeNotification) -> Result<(), Self::Error> {
        self.platform.notify(notification).await
    }

    fn is_transient(error: &Self::Error) -> bool {
        error.is_transient()
    }
}

const fn should_request_notification_permission(
    status: NotificationPermissionStatus,
    was_configured: bool,
    system_alerts_were_enabled: bool,
    system_alerts_enabled: bool,
) -> bool {
    system_alerts_enabled
        && matches!(status, NotificationPermissionStatus::Unknown)
        && (!was_configured || !system_alerts_were_enabled)
}

async fn request_notification_permission_with_modal(
    app: &AppHandle,
    platform: &PlatformNotifier,
) -> NotificationPermissionStatus {
    let _ = dispatch_shell_event(app, ShellEvent::ModalActivityOpened, None);
    let status = platform
        .request_permission()
        .await
        .unwrap_or(NotificationPermissionStatus::Error);
    let _ = dispatch_shell_event(app, ShellEvent::ModalActivityClosed, None);
    status
}

#[derive(Clone)]
struct SystemAutostart {
    app: AppHandle,
}

impl SystemAutostart {
    const fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl AutostartControl for SystemAutostart {
    type Error = tauri_plugin_autostart::Error;

    async fn is_enabled(&self) -> Result<bool, Self::Error> {
        self.app.autolaunch().is_enabled()
    }

    async fn set_enabled(&self, enabled: bool) -> Result<(), Self::Error> {
        if enabled {
            self.app.autolaunch().enable()
        } else {
            self.app.autolaunch().disable()
        }
    }
}

/// Returns public metadata that proves the Rust core is connected to the UI.
#[tauri::command]
fn get_build_info() -> BuildInfo {
    quotatide_core::build_info()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri injects managed state command arguments by value.
fn get_startup_state(state: tauri::State<'_, PublicStartupState>) -> PublicStartupState {
    state.inner().clone()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri injects AppHandle command arguments by value.
fn retry_local_recovery(app: AppHandle) {
    app.request_restart();
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri injects managed state command arguments by value.
fn open_local_data_directory(
    context: tauri::State<'_, LocalDataContext>,
) -> Result<(), PublicError> {
    open_directory(&context.app_data).map_err(|_| storage_public_error())
}

fn open_directory(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new("explorer.exe");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut command = std::process::Command::new("xdg-open");

    let status = command.arg(path).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(
            "operating system did not open the application data directory",
        ))
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri injects command dependencies by value.
#[allow(clippy::too_many_lines)] // Each diagnostic field is explicitly allowlisted at this boundary.
async fn export_diagnostics(
    app: AppHandle,
    context: tauri::State<'_, LocalDataContext>,
) -> Result<bool, PublicError> {
    let settings = match app.try_state::<AccountSettingsStore>() {
        Some(store) => store.public_atomic_settings().await.ok(),
        None => None,
    };
    let dashboard = match app.try_state::<LiveApplication>() {
        Some(application) => application.live_quota(SystemClock.now_unix_ms()).await.ok(),
        None => None,
    };
    let safe_settings = settings.as_ref().map_or_else(
        || serde_json::json!({"available": false}),
        |settings| {
            serde_json::json!({
                "available": true,
                "settingsRevision": settings.settings_revision,
                "configured": settings.configured,
                "policyTimezone": settings.quota_policy.policy_timezone,
                "sevenDayBaseMicropoints": settings.quota_policy.base_micropoints,
                "carryWorkdaysEnabled": settings.quota_policy.carry_workdays_enabled,
                "authConfigured": settings.configured,
                "autostartEnabled": settings.autostart_enabled,
                "notificationPermission": settings.notification_permission_status,
                "smtpConfigured": settings.smtp.enabled,
                "smtpTlsMode": settings.smtp.tls_mode,
                "emailDestinationCount": settings.smtp.recipients.len(),
            })
        },
    );
    let source_health = dashboard.as_ref().map_or_else(
        || serde_json::json!({"codex": {"available": false}, "radar": {"available": false}}),
        |dashboard| {
            let codex = dashboard.quota.as_ref().map_or_else(
                || serde_json::json!({"available": false}),
                |quota| {
                    serde_json::json!({
                        "available": true,
                        "status": quota.source_status,
                        "lastAttemptAtUnixMs": quota.last_attempt_at_unix_ms,
                        "lastSuccessAtUnixMs": quota.last_success_at_unix_ms,
                        "consecutiveFailures": quota.consecutive_failures,
                        "publicErrorCode": quota.public_error,
                    })
                },
            );
            serde_json::json!({
                "codex": codex,
                "radar": {
                    "available": true,
                    "status": dashboard.radar.source_status,
                    "lastAttemptAtUnixMs": dashboard.radar.last_attempt_at_unix_ms,
                    "lastSuccessAtUnixMs": dashboard.radar.last_success_at_unix_ms,
                    "consecutiveFailures": dashboard.radar.consecutive_failures,
                    "publicErrorCode": dashboard.radar.public_error,
                },
            })
        },
    );
    let observations = dashboard
        .as_ref()
        .and_then(|dashboard| dashboard.quota.as_ref())
        .map_or_else(
            || serde_json::json!({"available": false}),
            |quota| {
                serde_json::json!({
                    "available": true,
                    "capturedAtUnixMs": quota.captured_at_unix_ms,
                    "windowStartsAtUnixS": quota.window_starts_at_unix_s,
                    "windowEndsAtUnixS": quota.window_ends_at_unix_s,
                    "resetsAtUnixS": quota.resets_at_unix_s,
                    "usedMicropoints": quota.used_micropoints,
                    "remainingMicropoints": quota.remaining_micropoints,
                    "todayBaseMicropoints": quota.today_base_micropoints,
                    "todayCarryMicropoints": quota.today_carry_micropoints,
                    "todayLimitMicropoints": quota.today_limit_micropoints,
                    "ledgerDays": quota.ledger_days,
                })
            },
        );
    let database_path = context.app_data.join("state.sqlite3");
    let payload = privacy::DiagnosticPayload {
        manifest: serde_json::json!({
            "formatVersion": 1,
            "createdAtUnixMs": SystemClock.now_unix_ms(),
            "database": privacy::database_diagnostic(&database_path),
        }),
        app: serde_json::json!({
            "productName": "QuotaTide",
            "version": env!("CARGO_PKG_VERSION"),
            "rustVersion": env!("CARGO_PKG_RUST_VERSION"),
            "tauriVersion": tauri::VERSION,
            "webviewVersion": "system-runtime",
            "osFamily": std::env::consts::OS,
            "architecture": std::env::consts::ARCH,
        }),
        safe_settings,
        source_health,
        current_epoch_observations: observations,
    };
    let app_data = context.app_data.clone();
    let _ = dispatch_shell_event(&app, ShellEvent::ModalActivityOpened, None);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let Some(target) = rfd::FileDialog::new()
            .add_filter("ZIP archive", &["zip"])
            .set_file_name(format!(
                "QuotaTide-diagnostic-{}.zip",
                SystemClock.now_unix_ms()
            ))
            .save_file()
        else {
            return Ok(false);
        };
        privacy::export_diagnostic_zip(&app_data, &target, &payload)
            .map(|()| true)
            .map_err(|_| ())
    })
    .await
    .map_err(|_| storage_public_error())?;
    let _ = dispatch_shell_event(&app, ShellEvent::ModalActivityClosed, None);
    result.map_err(|()| storage_public_error())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri injects command dependencies by value.
async fn clear_all_local_data(
    app: AppHandle,
    context: tauri::State<'_, LocalDataContext>,
) -> Result<(), PublicError> {
    if let Some(application) = app.try_state::<LiveApplication>() {
        application.cancel_scheduler();
    }
    if let Some(worker) = app.try_state::<DeliveryWorkerLifecycle>() {
        worker.cancel();
    }
    quotatide_core::CredentialVault::delete(&SystemCredentialVault, "slot-a")
        .await
        .map_err(|_| {
            PublicError::new(
                PublicErrorCode::CredentialUnavailable,
                "privacy.credential_unavailable",
            )
        })?;
    quotatide_core::CredentialVault::delete(&SystemCredentialVault, "slot-b")
        .await
        .map_err(|_| {
            PublicError::new(
                PublicErrorCode::CredentialUnavailable,
                "privacy.credential_unavailable",
            )
        })?;
    let autostart = SystemAutostart::new(app.clone());
    autostart.set_enabled(false).await.map_err(|_| {
        PublicError::new(
            PublicErrorCode::AutostartUnavailable,
            "privacy.autostart_unavailable",
        )
    })?;
    if autostart.is_enabled().await.unwrap_or(true) {
        return Err(PublicError::new(
            PublicErrorCode::AutostartUnavailable,
            "privacy.autostart_unavailable",
        ));
    }
    privacy::write_clear_marker(&context.app_data).map_err(|_| storage_public_error())?;
    app.request_restart();
    Ok(())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri injects AppHandle command arguments by value.
fn hide_main_window(app: AppHandle) -> Result<(), String> {
    dispatch_shell_event(&app, ShellEvent::CloseRequested, None)
}

#[tauri::command]
async fn request_manual_refresh(
    application: tauri::State<'_, LiveApplication>,
) -> Result<u64, PublicError> {
    let receipt = application
        .refresh(RefreshTrigger::Manual)
        .await
        .map_err(|_| storage_public_error())?;
    Ok(u64::from(receipt.retry_after_ms))
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri injects AppHandle command arguments by value.
fn begin_modal_activity(app: AppHandle) -> Result<(), String> {
    dispatch_shell_event(&app, ShellEvent::ModalActivityOpened, None)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri injects AppHandle command arguments by value.
fn end_modal_activity(app: AppHandle) -> Result<(), String> {
    dispatch_shell_event(&app, ShellEvent::ModalActivityClosed, None)
}

#[tauri::command]
async fn get_settings(
    settings: tauri::State<'_, LiveAtomicSettings>,
) -> Result<PublicSettings, PublicError> {
    settings
        .public_settings()
        .await
        .map_err(|error| error.public::<AuthFileReader>())
}

#[tauri::command]
async fn get_live_quota(
    application: tauri::State<'_, LiveApplication>,
) -> Result<PublicLiveQuotaState, PublicError> {
    application
        .live_quota(SystemClock.now_unix_ms())
        .await
        .map_err(|error| error.public::<AuthFileReader>())
}

#[tauri::command]
async fn get_alerts(
    store: tauri::State<'_, AccountSettingsStore>,
) -> Result<PublicAlertInbox, PublicError> {
    store
        .public_alerts(12)
        .await
        .map_err(|_| storage_public_error())
}

#[tauri::command]
async fn request_system_notification_permission(
    app: AppHandle,
    store: tauri::State<'_, AccountSettingsStore>,
    permission: tauri::State<'_, SharedNotificationPermission>,
    notifier: tauri::State<'_, PlatformNotifier>,
    delivery_worker: tauri::State<'_, DeliveryWorkerLifecycle>,
) -> Result<NotificationPermissionStatus, PublicError> {
    let status = if permission.get() == NotificationPermissionStatus::Unknown {
        request_notification_permission_with_modal(&app, &notifier).await
    } else {
        notifier
            .permission_state()
            .await
            .unwrap_or(NotificationPermissionStatus::Error)
    };
    store
        .set_notification_permission_status(status, SystemClock.now_unix_ms())
        .await
        .map_err(|_| storage_public_error())?;
    permission.set(status);
    if let Ok(settings) = store.public_atomic_settings().await {
        let _ = app.emit(
            SETTINGS_CHANGED_EVENT,
            SettingsChanged {
                revision: settings.settings_revision,
            },
        );
    }
    delivery_worker.wake();
    Ok(status)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri injects each managed command dependency.
async fn save_settings(
    app: AppHandle,
    settings: tauri::State<'_, LiveAtomicSettings>,
    application: tauri::State<'_, LiveApplication>,
    store: tauri::State<'_, AccountSettingsStore>,
    permission: tauri::State<'_, SharedNotificationPermission>,
    notifier: tauri::State<'_, PlatformNotifier>,
    delivery_worker: tauri::State<'_, DeliveryWorkerLifecycle>,
    draft: SettingsDraft,
) -> Result<PublicSettings, PublicError> {
    let previous = settings
        .public_settings()
        .await
        .map_err(|error| error.public::<AuthFileReader>())?;
    let refresh_selected_account = draft.auth_path.is_some();
    let system_alerts_enabled = draft
        .alert_preferences
        .iter()
        .any(|preference| preference.channel == AlertChannel::System && preference.enabled);
    let system_alerts_were_enabled = previous
        .alert_preferences
        .iter()
        .any(|preference| preference.channel == AlertChannel::System && preference.enabled);
    let should_request_permission = should_request_notification_permission(
        previous.notification_permission_status,
        previous.configured,
        system_alerts_were_enabled,
        system_alerts_enabled,
    );
    let mut saved = settings
        .save_settings(draft)
        .await
        .map_err(|error| error.public::<AuthFileReader>())?;
    if should_request_permission && saved.configured {
        let status = request_notification_permission_with_modal(&app, &notifier).await;
        if store
            .set_notification_permission_status(status, SystemClock.now_unix_ms())
            .await
            .is_err()
        {
            safe_log(
                SafeLogLevel::Error,
                "settings",
                "persist_notification_permission",
                SafeLogFields {
                    public_error_code: Some("storage_unavailable"),
                    ..SafeLogFields::default()
                },
            );
        } else {
            permission.set(status);
            if let Ok(reloaded) = settings.public_settings().await {
                saved = reloaded;
            }
        }
    }
    let _ = app.emit(
        SETTINGS_CHANGED_EVENT,
        SettingsChanged {
            revision: saved.settings_revision,
        },
    );
    delivery_worker.wake();
    if refresh_selected_account {
        let application = application.inner().clone();
        tauri::async_runtime::spawn(async move {
            let _ = application.refresh_selected_account().await;
        });
    }
    Ok(saved)
}

#[tauri::command]
async fn send_test_email(
    worker: tauri::State<'_, LiveEmailDeliveryWorker>,
) -> Result<u32, PublicError> {
    worker.send_test_email().await.map_err(|error| match error {
        TestEmailError::NotConfigured => PublicError::new(
            PublicErrorCode::InvalidSmtpSettings,
            "settings.smtp_not_configured",
        ),
        TestEmailError::CredentialMissing | TestEmailError::CredentialUnavailable => {
            PublicError::new(
                PublicErrorCode::CredentialUnavailable,
                "settings.credential_unavailable",
            )
        }
        TestEmailError::DeliveryFailed => PublicError::new(
            PublicErrorCode::EmailDeliveryFailed,
            "settings.smtp_test_failed",
        ),
        TestEmailError::StorageUnavailable => storage_public_error(),
    })
}

#[cfg(test)]
async fn configure_selected_auth(
    application: &AccountApplication<AuthFileReader>,
    expected_settings_revision: u32,
    path: &std::path::Path,
) -> Result<quotatide_core::PublicAccountSettings, PublicError> {
    application
        .select_account(expected_settings_revision, path)
        .await
        .map_err(|error| error.public::<AuthFileReader>())
}

fn storage_public_error() -> PublicError {
    PublicError::new(
        PublicErrorCode::StorageUnavailable,
        "settings.storage_unavailable",
    )
}

fn menu_event_for_id(id: &str) -> Option<ShellEvent> {
    match id {
        "open" => Some(ShellEvent::OpenRequested),
        "refresh" => Some(ShellEvent::RefreshRequested),
        "exit" => Some(ShellEvent::ExitRequested),
        _ => None,
    }
}

fn dispatch_shell_event(
    app: &AppHandle,
    event: ShellEvent,
    tray_rect: Option<Rect>,
) -> Result<(), String> {
    let effect = {
        let state = app.state::<SharedDesktopShell>();
        let mut desktop = state.lock().map_err(|_| "托盘窗口状态不可用".to_owned())?;
        if let Some(rect) = tray_rect {
            desktop.last_tray_rect = Some(rect);
        }
        desktop.shell.handle(event)
    };

    realize_effect(app, effect)
}

fn realize_effect(app: &AppHandle, effect: ShellEffect) -> Result<(), String> {
    match effect {
        ShellEffect::None => Ok(()),
        ShellEffect::Show => show_main_window(app),
        ShellEffect::Hide => main_window(app)?.hide().map_err(platform_error),
        ShellEffect::Refresh => {
            let application = app.state::<LiveApplication>().inner().clone();
            tauri::async_runtime::spawn(async move {
                let _ = application.refresh(RefreshTrigger::Manual).await;
            });
            Ok(())
        }
        ShellEffect::Exit => {
            app.exit(0);
            Ok(())
        }
        ShellEffect::Reposition => position_main_window(app),
    }
}

fn main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "找不到 QuotaTide 主窗口".to_owned())
}

fn show_main_window(app: &AppHandle) -> Result<(), String> {
    let window = main_window(app)?;
    position_main_window(app)?;
    window.show().map_err(platform_error)?;
    window.set_focus().map_err(platform_error)?;
    Ok(())
}

fn activate_notification(app: &AppHandle, target: AlertTarget) {
    let activation_id = NOTIFICATION_ACTIVATION_SEQUENCE.fetch_add(1, Ordering::AcqRel) + 1;
    let activation_app = app.clone();
    let _ = app.run_on_main_thread(move || {
        if show_main_window(&activation_app).is_err() {
            safe_log(
                SafeLogLevel::Warning,
                "window",
                "open_notification_target",
                SafeLogFields {
                    public_error_code: Some("platform_unavailable"),
                    ..SafeLogFields::default()
                },
            );
            return;
        }
        let _ = activation_app.emit(
            NOTIFICATION_OPENED_EVENT,
            NotificationOpened {
                target,
                activation_id,
            },
        );
    });
}

fn position_main_window(app: &AppHandle) -> Result<(), String> {
    let window = main_window(app)?;
    let monitors = window.available_monitors().map_err(platform_error)?;
    let fallback = window
        .current_monitor()
        .map_err(platform_error)?
        .or(window.primary_monitor().map_err(platform_error)?)
        .or_else(|| monitors.first().cloned())
        .ok_or_else(|| "没有可用显示器".to_owned())?;

    let tray_rect = {
        let state = app.state::<SharedDesktopShell>();
        state
            .lock()
            .map_err(|_| "托盘窗口状态不可用".to_owned())?
            .last_tray_rect
    };
    let anchor = tray_rect.as_ref().map_or_else(
        || CoreRect::new(f64::NAN, 0.0, 0.0, 0.0),
        |rect| core_rect(rect, fallback.scale_factor()),
    );
    let anchor_center = (
        anchor.origin().x() + anchor.size().width() / 2.0,
        anchor.origin().y() + anchor.size().height() / 2.0,
    );
    let monitor = monitors
        .iter()
        .find(|monitor| {
            let area = monitor.work_area();
            let left = f64::from(area.position.x);
            let top = f64::from(area.position.y);
            let right = left + f64::from(area.size.width);
            let bottom = top + f64::from(area.size.height);
            anchor_center.0 >= left
                && anchor_center.0 <= right
                && anchor_center.1 >= top
                && anchor_center.1 <= bottom
        })
        .unwrap_or(&fallback);
    let area = monitor.work_area();
    let work_area = CoreRect::new(
        f64::from(area.position.x),
        f64::from(area.position.y),
        f64::from(area.size.width),
        f64::from(area.size.height),
    );
    let window_size = window.outer_size().map_err(platform_error)?;
    let point = place_tray_window(
        anchor,
        work_area,
        CoreSize::new(f64::from(window_size.width), f64::from(window_size.height)),
        WINDOW_GAP,
    );

    window
        .set_position(PhysicalPosition::new(
            physical_coordinate(point.x()),
            physical_coordinate(point.y()),
        ))
        .map_err(platform_error)
}

#[allow(clippy::cast_possible_truncation)]
fn physical_coordinate(value: f64) -> i32 {
    value
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

fn core_rect(rect: &Rect, scale_factor: f64) -> CoreRect {
    let position = rect.position.to_physical::<f64>(scale_factor);
    let size = rect.size.to_physical::<f64>(scale_factor);
    CoreRect::new(position.x, position.y, size.width, size.height)
}

fn setup_tray(app: &mut App) -> tauri::Result<()> {
    let tray = app
        .tray_by_id(MAIN_TRAY_ID)
        .ok_or_else(|| tauri::Error::AssetNotFound("main tray icon".to_owned()))?;
    let open = MenuItem::with_id(app, "open", "打开 QuotaTide", true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, "refresh", "立即刷新", true, None::<&str>)?;
    let exit = MenuItem::with_id(app, "exit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &refresh, &exit])?;

    tray.set_menu(Some(menu))?;
    tray.set_show_menu_on_left_click(false)
}

fn apply_platform_material(window: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    let effects = [WindowEffect::Popover, WindowEffect::HudWindow];
    #[cfg(target_os = "windows")]
    let effects = [WindowEffect::Acrylic, WindowEffect::Mica];
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let effects: [WindowEffect; 0] = [];

    for effect in effects {
        if window
            .set_effects(WindowEffectsConfig {
                effects: vec![effect],
                ..WindowEffectsConfig::default()
            })
            .is_ok()
        {
            return;
        }
    }

    safe_log(
        SafeLogLevel::Info,
        "window",
        "opaque_surface_fallback",
        SafeLogFields::default(),
    );
    let _ = window.eval(
        "document.documentElement.dataset.surface='opaque';\
         document.documentElement.dataset.platformFallback='true';",
    );
}

fn platform_error(error: impl std::fmt::Display) -> String {
    format!("平台操作失败：{error}")
}

fn spawn_refresh_scheduler(application: LiveApplication, refresh_on_startup: bool) {
    tauri::async_runtime::spawn(async move {
        application.run_hourly_scheduler(refresh_on_startup).await;
    });
}

fn spawn_dashboard_event_bridge(
    app: AppHandle,
    application: LiveApplication,
    delivery_worker: DeliveryWorkerLifecycle,
) {
    tauri::async_runtime::spawn(async move {
        let mut changes = application.subscribe_dashboard_changes();
        while changes.changed().await.is_ok() {
            let change: DashboardChanged = *changes.borrow_and_update();
            let _ = app.emit(DASHBOARD_CHANGED_EVENT, change);
            delivery_worker.wake();
        }
    });
}

fn finish_shell_setup(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        apply_platform_material(&window);
    }
    let launch_mode = LaunchMode::from_args(std::env::args());
    if launch_mode.shows_window() {
        dispatch_shell_event(app.handle(), ShellEvent::OpenRequested, None)
            .map_err(|_| "failed to show the initial tray window")?;
    }
    Ok(())
}

fn enter_recovery_mode(
    app: &mut App,
    app_data: PathBuf,
    startup: PublicStartupState,
) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(LocalDataContext { app_data });
    app.manage(startup);
    finish_shell_setup(app)
}

#[allow(clippy::too_many_lines)] // Wires the desktop dependency graph in one startup transaction.
fn setup_application(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    setup_tray(app)?;
    let app_data = app.path().app_data_dir()?;
    if std::fs::create_dir_all(&app_data).is_err() || secure_app_data_directory(&app_data).is_err()
    {
        return enter_recovery_mode(
            app,
            app_data,
            PublicStartupState::storage_permission_denied(),
        );
    }
    if privacy::clear_requested(&app_data) && privacy::clear_scoped_local_data(&app_data).is_err() {
        return enter_recovery_mode(
            app,
            app_data,
            PublicStartupState::storage_permission_denied(),
        );
    }
    let _ = privacy::initialize_safe_logging(&app_data);
    safe_log(
        SafeLogLevel::Info,
        "lifecycle",
        "startup",
        SafeLogFields::default(),
    );
    let database_path = app_data.join("state.sqlite3");
    if validate_database_targets(&database_path).is_err() {
        return enter_recovery_mode(
            app,
            app_data,
            PublicStartupState::storage_permission_denied(),
        );
    }
    let store = match tauri::async_runtime::block_on(AccountSettingsStore::open(&database_path)) {
        Ok(store) => store,
        Err(SettingsStoreError::UnsupportedSchema) => {
            return enter_recovery_mode(app, app_data, PublicStartupState::unsupported_schema());
        }
        Err(_) => {
            return enter_recovery_mode(app, app_data, PublicStartupState::recovery_required());
        }
    };
    if secure_database_files(&database_path).is_err() {
        return enter_recovery_mode(
            app,
            app_data,
            PublicStartupState::storage_permission_denied(),
        );
    }
    let Ok(persisted_settings) = tauri::async_runtime::block_on(store.public_atomic_settings())
    else {
        return enter_recovery_mode(app, app_data, PublicStartupState::recovery_required());
    };
    let notification_permission =
        SharedNotificationPermission::new(persisted_settings.notification_permission_status);
    let atomic_settings = AtomicSettingsManager::new(
        store.clone(),
        AuthFileReader,
        SystemAutostart::new(app.handle().clone()),
    )
    .with_credential_vault(SystemCredentialVault);
    if tauri::async_runtime::block_on(atomic_settings.recover_external_changes()).is_err() {
        return enter_recovery_mode(app, app_data, PublicStartupState::recovery_required());
    }
    app.manage(LocalDataContext { app_data });
    app.manage(PublicStartupState::ready(store.recovered_at_startup()));
    app.manage(store.clone());
    app.manage(notification_permission.clone());
    app.manage(atomic_settings);
    let usage_client =
        CodexUsageClient::new().map_err(|_| "failed to initialize Codex usage client")?;
    let refresh = RefreshCoordinator::new(
        store.clone(),
        ConfiguredCodexUsageSource::new(store.clone(), usage_client),
        SystemClock,
    )
    .with_reset_radar_source(
        ResetRadarClient::new().map_err(|_| "failed to initialize Reset Radar client")?,
    );
    let settings = SettingsManager::new(store.clone(), AuthFileReader);
    let application = Application::new(AccountApplication::new(settings), refresh);
    app.manage(application.clone());
    let delivery_worker = DeliveryWorkerLifecycle::default();
    let late_failure_store = store.clone();
    let late_failure_app = app.handle().clone();
    let late_failure_callback = Arc::new(move |delivery_key: String| {
        let store = late_failure_store.clone();
        let event_app = late_failure_app.clone();
        tauri::async_runtime::spawn(async move {
            match store
                .record_late_system_delivery_failure(&delivery_key, SystemClock.now_unix_ms())
                .await
            {
                Ok(true) => {
                    let _ = event_app.emit(ALERTS_CHANGED_EVENT, ());
                }
                Ok(false) => {}
                Err(_) => {
                    safe_log(
                        SafeLogLevel::Error,
                        "delivery",
                        "persist_late_system_failure",
                        SafeLogFields {
                            public_error_code: Some("storage_unavailable"),
                            ..SafeLogFields::default()
                        },
                    );
                }
            }
        });
    });
    let platform_notifier = PlatformNotifier::new(app.handle(), late_failure_callback)
        .map_err(|_| "failed to initialize native notifications")?;
    app.manage(platform_notifier.clone());
    let notifier = DesktopSystemNotifier {
        platform: platform_notifier,
        permission: notification_permission,
    };
    let live_delivery_worker = DeliveryWorker::new(
        store.clone(),
        notifier,
        format!("desktop-{}", std::process::id()),
    );
    let live_email_worker = EmailDeliveryWorker::new(
        store.clone(),
        SystemCredentialVault,
        LettreMailTransport::default(),
        format!("desktop-email-{}", std::process::id()),
    );
    app.manage(live_email_worker.clone());
    if !delivery_worker.configure(
        live_delivery_worker,
        live_email_worker,
        app.handle().clone(),
    ) {
        return Err("failed to configure the notification delivery worker".into());
    }
    app.manage(delivery_worker.clone());
    spawn_dashboard_event_bridge(
        app.handle().clone(),
        application.clone(),
        delivery_worker.clone(),
    );
    let launch_mode = LaunchMode::from_args(std::env::args());
    start_primary(
        launch_mode,
        || spawn_refresh_scheduler(application, true),
        || {
            let _ = delivery_worker.start();
        },
        || {
            dispatch_shell_event(app.handle(), ShellEvent::OpenRequested, None)
                .map_err(|_| "failed to show the initial tray window")
        },
    )?;
    Ok(())
}

fn handle_run_event(app: &AppHandle, event: &RunEvent) {
    match event {
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => {
            if dispatch_shell_event(app, ShellEvent::OpenRequested, None).is_err() {
                safe_log(
                    SafeLogLevel::Warning,
                    "window",
                    "reopen",
                    SafeLogFields::default(),
                );
            }
        }
        RunEvent::Resumed => {
            if dispatch_shell_event(app, ShellEvent::ResumeRequested, None).is_err() {
                safe_log(
                    SafeLogLevel::Warning,
                    "window",
                    "resume",
                    SafeLogFields::default(),
                );
            }
            if let Some(application) = app.try_state::<LiveApplication>() {
                application.notify_resume();
            }
            if let Some(worker) = app.try_state::<DeliveryWorkerLifecycle>() {
                worker.wake();
            }
        }
        RunEvent::Exit | RunEvent::ExitRequested { .. } => {
            if let Some(application) = app.try_state::<LiveApplication>() {
                application.cancel_scheduler();
            }
            if let Some(worker) = app.try_state::<DeliveryWorkerLifecycle>() {
                worker.cancel();
            }
        }
        _ => {}
    }
}

/// Starts the `QuotaTide` desktop runtime.
///
/// # Panics
///
/// Panics when the desktop runtime cannot be initialized or its event loop fails.
#[allow(clippy::too_many_lines)] // Tauri's builder keeps event ownership visible in one place.
pub fn run() {
    restrict_process_file_creation();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if notify_secondary(
                LaunchMode::from_args(&args),
                || dispatch_shell_event(app, ShellEvent::OpenRequested, None),
                || {
                    if let Some(application) = app.try_state::<LiveApplication>() {
                        application.notify_resume();
                    }
                },
                || {
                    if let Some(worker) = app.try_state::<DeliveryWorkerLifecycle>() {
                        worker.wake();
                    }
                },
            )
            .is_err()
            {
                safe_log(
                    SafeLogLevel::Warning,
                    "lifecycle",
                    "secondary_activation",
                    SafeLogFields::default(),
                );
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![AUTOSTART_ARGUMENT]),
        ))
        .manage(SharedDesktopShell::default())
        .setup(setup_application)
        .on_menu_event(|app, event| {
            if let Some(shell_event) = menu_event_for_id(event.id().0.as_str()) {
                if dispatch_shell_event(app, shell_event, None).is_err() {
                    safe_log(
                        SafeLogLevel::Warning,
                        "window",
                        "menu_event",
                        SafeLogFields::default(),
                    );
                }
            }
        })
        .on_tray_icon_event(|app, event| {
            if let TrayIconEvent::Click {
                id,
                rect,
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if id.as_ref() == MAIN_TRAY_ID
                    && dispatch_shell_event(app, ShellEvent::LeftClick, Some(rect)).is_err()
                {
                    safe_log(
                        SafeLogLevel::Warning,
                        "window",
                        "tray_click",
                        SafeLogFields::default(),
                    );
                }
            }
        })
        .on_window_event(|window, event| {
            if window.label() != MAIN_WINDOW_LABEL {
                return;
            }

            let shell_event = match event {
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    Some(ShellEvent::CloseRequested)
                }
                WindowEvent::Focused(false) => Some(ShellEvent::FocusLost),
                _ => None,
            };
            if let Some(shell_event) = shell_event {
                if dispatch_shell_event(window.app_handle(), shell_event, None).is_err() {
                    safe_log(
                        SafeLogLevel::Warning,
                        "window",
                        "window_event",
                        SafeLogFields::default(),
                    );
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_build_info,
            get_startup_state,
            retry_local_recovery,
            open_local_data_directory,
            export_diagnostics,
            clear_all_local_data,
            hide_main_window,
            request_manual_refresh,
            begin_modal_activity,
            end_modal_activity,
            get_settings,
            get_live_quota,
            get_alerts,
            request_system_notification_permission,
            save_settings,
            send_test_email
        ])
        .build(tauri::generate_context!())
        .expect("failed to build the QuotaTide desktop shell");

    app.run(|app, event| handle_run_event(app, &event));
}

#[cfg(unix)]
fn restrict_process_file_creation() {
    use nix::sys::stat::{Mode, umask};

    let _ = umask(Mode::from_bits_truncate(0o077));
}

#[cfg(not(unix))]
const fn restrict_process_file_creation() {}

#[cfg(unix)]
fn secure_app_data_directory(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != nix::unistd::Uid::effective().as_raw()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "application data directory is not a current-user-owned directory",
        ));
    }
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

#[cfg(windows)]
fn secure_app_data_directory(path: &std::path::Path) -> std::io::Result<()> {
    reject_symlink_or_wrong_kind(path, true)?;
    apply_windows_dacl(path, true)
}

#[cfg(unix)]
fn secure_database_files(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    for candidate in [
        path.to_path_buf(),
        path.with_extension("sqlite3-wal"),
        path.with_extension("sqlite3-shm"),
    ] {
        if candidate.exists() {
            std::fs::set_permissions(candidate, std::fs::Permissions::from_mode(0o600))?;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn secure_database_files(path: &std::path::Path) -> std::io::Result<()> {
    for candidate in database_candidates(path) {
        if candidate.exists() {
            reject_symlink_or_wrong_kind(&candidate, false)?;
            apply_windows_dacl(&candidate, false)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn validate_database_targets(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::MetadataExt as _;

    for candidate in database_candidates(path) {
        let Ok(metadata) = std::fs::symlink_metadata(&candidate) else {
            continue;
        };
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.uid() != nix::unistd::Uid::effective().as_raw()
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "database target is not a current-user-owned regular file",
            ));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn validate_database_targets(path: &std::path::Path) -> std::io::Result<()> {
    for candidate in database_candidates(path) {
        if candidate.exists() {
            reject_symlink_or_wrong_kind(&candidate, false)?;
        }
    }
    Ok(())
}

fn database_candidates(path: &std::path::Path) -> [std::path::PathBuf; 3] {
    [
        path.to_path_buf(),
        path.with_extension("sqlite3-wal"),
        path.with_extension("sqlite3-shm"),
    ]
}

#[cfg(windows)]
fn reject_symlink_or_wrong_kind(path: &std::path::Path, directory: bool) -> std::io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    let valid_kind = if directory {
        metadata.is_dir()
    } else {
        metadata.is_file()
    };
    if metadata.file_type().is_symlink() || !valid_kind {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "security target has an invalid kind",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn apply_windows_dacl(path: &std::path::Path, directory: bool) -> std::io::Result<()> {
    use std::process::Command;

    // Windows PowerShell exposes the managed Windows ACL APIs without requiring
    // unsafe FFI in this crate. Start from an empty ACL, disable inheritance,
    // add the exact allowlist, apply it, and then fail closed unless a read-back
    // proves the DACL is protected and contains exactly those three SIDs.
    const ACL_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$target = $args[0]
$isDirectory = $args[1] -eq 'directory'
$current = [System.Security.Principal.WindowsIdentity]::GetCurrent().User
$system = [System.Security.Principal.SecurityIdentifier]::new('S-1-5-18')
$administrators = [System.Security.Principal.SecurityIdentifier]::new('S-1-5-32-544')
$expected = @($current, $system, $administrators)
if ($isDirectory) {
  $acl = [System.Security.AccessControl.DirectorySecurity]::new()
  $inheritance = [System.Security.AccessControl.InheritanceFlags]'ContainerInherit, ObjectInherit'
} else {
  $acl = [System.Security.AccessControl.FileSecurity]::new()
  $inheritance = [System.Security.AccessControl.InheritanceFlags]::None
}
$acl.SetOwner($current)
$acl.SetAccessRuleProtection($true, $false)
foreach ($sid in $expected) {
  $rule = [System.Security.AccessControl.FileSystemAccessRule]::new(
    $sid,
    [System.Security.AccessControl.FileSystemRights]::FullControl,
    $inheritance,
    [System.Security.AccessControl.PropagationFlags]::None,
    [System.Security.AccessControl.AccessControlType]::Allow
  )
  [void]$acl.AddAccessRule($rule)
}
Set-Acl -LiteralPath $target -AclObject $acl
$actual = Get-Acl -LiteralPath $target
if (-not $actual.AreAccessRulesProtected) { exit 31 }
if ($actual.Owner -ne $current.Translate([System.Security.Principal.NTAccount]).Value) { exit 32 }
$rules = @($actual.GetAccessRules($true, $false, [System.Security.Principal.SecurityIdentifier]))
if ($rules.Count -ne 3) { exit 33 }
foreach ($rule in $rules) {
  if ($rule.AccessControlType -ne [System.Security.AccessControl.AccessControlType]::Allow) { exit 34 }
  if (($expected.Value -notcontains $rule.IdentityReference.Value)) { exit 35 }
  if (($rule.FileSystemRights -band [System.Security.AccessControl.FileSystemRights]::FullControl) -ne [System.Security.AccessControl.FileSystemRights]::FullControl) { exit 36 }
}
"#;
    let kind = if directory { "directory" } else { "file" };
    let status = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            ACL_SCRIPT,
        ])
        .arg(path)
        .arg(kind)
        .status()?;
    if !status.success() {
        return Err(std::io::Error::other(
            "could not apply and verify the protected application DACL",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use quotatide_core::{
        AccountApplication, AccountSettingsStore, NotificationPermissionStatus, SettingsManager,
        ShellEvent,
    };
    use tempfile::tempdir;

    use super::{
        AuthFileReader, DeliveryWorkerLifecycle, PublicStartupState, SharedNotificationPermission,
        configure_selected_auth, get_build_info, menu_event_for_id,
        should_request_notification_permission,
    };
    use crate::background_lifecycle::{AUTOSTART_ARGUMENT, LaunchMode};

    #[test]
    fn command_returns_the_public_core_contract() {
        let info = get_build_info();

        assert_eq!(info.product_name, "QuotaTide");
        assert_eq!(info.identifier, "dev.theblind.quotatide");
        assert_eq!(info.stage, "skeleton");
    }

    #[test]
    fn startup_recovery_contract_contains_no_local_path_or_internal_error() {
        let payload = serde_json::to_string(&PublicStartupState::recovery_required())
            .expect("serialize startup state");

        assert_eq!(
            payload,
            r#"{"mode":"recovery_required","messageKey":"startup.recovery_required","recoveredFromBackup":false}"#
        );
        assert!(!payload.contains("/Users/"));
        assert!(!payload.contains("sqlite"));
    }

    #[cfg(unix)]
    #[test]
    fn application_directory_is_restricted_and_symlink_database_targets_are_rejected() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempdir().expect("temporary directory");
        let app_data = directory.path().join("data");
        fs::create_dir(&app_data).expect("app data");
        fs::set_permissions(&app_data, fs::Permissions::from_mode(0o755))
            .expect("loose permissions");
        super::secure_app_data_directory(&app_data).expect("secure app data");
        assert_eq!(
            fs::metadata(&app_data)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );

        let outside = directory.path().join("outside.sqlite3");
        fs::write(&outside, b"outside").expect("outside database");
        std::os::unix::fs::symlink(&outside, app_data.join("state.sqlite3"))
            .expect("database symlink");
        assert!(super::validate_database_targets(&app_data.join("state.sqlite3")).is_err());
    }

    #[test]
    fn native_menu_ids_map_to_explicit_shell_events() {
        assert_eq!(menu_event_for_id("open"), Some(ShellEvent::OpenRequested));
        assert_eq!(
            menu_event_for_id("refresh"),
            Some(ShellEvent::RefreshRequested)
        );
        assert_eq!(menu_event_for_id("exit"), Some(ShellEvent::ExitRequested));
        assert_eq!(menu_event_for_id("unknown"), None);
    }

    #[test]
    fn autostart_launch_stays_hidden_while_a_user_launch_opens_the_existing_window() {
        assert_eq!(
            LaunchMode::from_args(["quotatide", AUTOSTART_ARGUMENT]),
            LaunchMode::Autostart
        );
        assert!(!LaunchMode::Autostart.shows_window());
        assert_eq!(LaunchMode::from_args(["quotatide"]), LaunchMode::User);
        assert!(LaunchMode::User.shows_window());
        assert!(!LaunchMode::from_args(["quotatide", AUTOSTART_ARGUMENT]).shows_window());
    }

    #[test]
    fn notification_permission_is_requested_only_from_a_user_configuration_transition() {
        assert!(should_request_notification_permission(
            NotificationPermissionStatus::Unknown,
            false,
            true,
            true,
        ));
        assert!(should_request_notification_permission(
            NotificationPermissionStatus::Unknown,
            true,
            false,
            true,
        ));
        assert!(!should_request_notification_permission(
            NotificationPermissionStatus::Unknown,
            true,
            true,
            true,
        ));
        assert!(!should_request_notification_permission(
            NotificationPermissionStatus::Denied,
            true,
            false,
            true,
        ));
    }

    #[test]
    fn notification_worker_keeps_unknown_permission_until_a_user_action_persists_it() {
        let permission = SharedNotificationPermission::new(NotificationPermissionStatus::Unknown);
        assert_eq!(permission.get(), NotificationPermissionStatus::Unknown);

        permission.set(NotificationPermissionStatus::Granted);
        assert_eq!(permission.get(), NotificationPermissionStatus::Granted);
    }

    #[tokio::test]
    async fn delivery_worker_lifecycle_starts_once_and_resume_only_wakes_it() {
        let worker = DeliveryWorkerLifecycle::default();

        assert!(worker.start());
        assert!(!worker.start());
        worker.wake();
        assert!(!worker.start());

        worker.cancel();
        tokio::task::yield_now().await;
    }

    #[test]
    fn command_boundary_serializes_no_auth_canaries_on_success_or_failure() {
        const ACCESS: &str = "access-ticket16-command-canary";
        const ACCOUNT: &str = "account-ticket16-command-canary";
        const JWT: &str = "jwt-ticket16-command-canary";

        let directory = tempdir().expect("temporary directory");
        let valid = directory.path().join("auth.json");
        let invalid = directory.path().join("invalid.json");
        fs::write(
            &valid,
            format!(
                r#"{{"auth_mode":"chatgpt","tokens":{{"access_token":"{ACCESS}","account_id":"{ACCOUNT}","id_token":"{JWT}"}}}}"#
            ),
        )
        .expect("valid auth fixture");
        fs::write(&invalid, format!(r#"{{"access_token":"{ACCESS}""#))
            .expect("invalid auth fixture");

        let application = tauri::async_runtime::block_on(async {
            let store = AccountSettingsStore::open(directory.path().join("state.sqlite3"))
                .await
                .expect("settings store");
            AccountApplication::new(SettingsManager::new(store, AuthFileReader))
        });
        let success =
            tauri::async_runtime::block_on(configure_selected_auth(&application, 0, &valid))
                .expect("success response");
        let failure =
            tauri::async_runtime::block_on(configure_selected_auth(&application, 1, &invalid))
                .expect_err("failure response");

        for payload in [
            serde_json::to_string(&success).expect("serialize success"),
            serde_json::to_string(&failure).expect("serialize failure"),
        ] {
            for canary in [ACCESS, ACCOUNT, JWT] {
                assert!(!payload.contains(canary));
            }
            assert!(!payload.contains(directory.path().to_string_lossy().as_ref()));
        }
    }
}
