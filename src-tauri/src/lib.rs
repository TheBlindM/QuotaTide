//! `QuotaTide` desktop shell.

pub mod auth_file;
pub mod codex_usage;

use std::sync::Mutex;

use auth_file::AuthFileReader;
use codex_usage::{CodexUsageClient, ConfiguredCodexUsageSource};
use quotatide_core::{
    AccountApplication, AccountSettingsStore, Application, BuildInfo, Clock, DashboardChanged,
    PhysicalRect as CoreRect, PhysicalSize as CoreSize, PublicAccountSettings, PublicError,
    PublicErrorCode, PublicLiveQuotaState, QuotaPolicyDraft, RefreshCoordinator, RefreshTrigger,
    SettingsChanged, SettingsManager, ShellEffect, ShellEvent, TrayShell, place_tray_window,
};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
use tauri::utils::WindowEffect;
use tauri::utils::config::WindowEffectsConfig;
use tauri::{
    App, AppHandle, Emitter, Manager, PhysicalPosition, Rect, RunEvent, WebviewWindow, WindowEvent,
};
use tauri_plugin_dialog::DialogExt;

const MAIN_WINDOW_LABEL: &str = "main";
const MAIN_TRAY_ID: &str = "main";
const WINDOW_GAP: f64 = 8.0;
const DASHBOARD_CHANGED_EVENT: &str = "quotatide://dashboard-changed";
const SETTINGS_CHANGED_EVENT: &str = "quotatide://settings-changed";

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

/// Returns public metadata that proves the Rust core is connected to the UI.
#[tauri::command]
fn get_build_info() -> BuildInfo {
    quotatide_core::build_info()
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
fn begin_external_dialog(app: AppHandle) -> Result<(), String> {
    dispatch_shell_event(&app, ShellEvent::ExternalDialogOpened, None)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri injects AppHandle command arguments by value.
fn end_external_dialog(app: AppHandle) -> Result<(), String> {
    dispatch_shell_event(&app, ShellEvent::ExternalDialogClosed, None)
}

#[tauri::command]
async fn get_account_settings(
    application: tauri::State<'_, LiveApplication>,
) -> Result<PublicAccountSettings, PublicError> {
    application
        .account_settings()
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
async fn update_quota_policy(
    app: AppHandle,
    application: tauri::State<'_, LiveApplication>,
    expected_settings_revision: u32,
    draft: QuotaPolicyDraft,
) -> Result<PublicAccountSettings, PublicError> {
    let settings = application
        .update_quota_policy(expected_settings_revision, draft)
        .await
        .map_err(|error| error.public::<AuthFileReader>())?;
    let dashboard_revision = application
        .live_quota(SystemClock.now_unix_ms())
        .await
        .map_err(|error| error.public::<AuthFileReader>())?
        .dashboard_revision;
    let _ = app.emit(
        SETTINGS_CHANGED_EVENT,
        SettingsChanged {
            revision: settings.settings_revision,
        },
    );
    let _ = app.emit(
        DASHBOARD_CHANGED_EVENT,
        DashboardChanged {
            revision: dashboard_revision,
        },
    );
    Ok(settings)
}

#[tauri::command]
async fn select_auth_file(
    app: AppHandle,
    application: tauri::State<'_, LiveApplication>,
    expected_settings_revision: u32,
) -> Result<PublicAccountSettings, PublicError> {
    dispatch_shell_event(&app, ShellEvent::ExternalDialogOpened, None).map_err(|_| {
        PublicError::new(
            PublicErrorCode::NativeDialogUnavailable,
            "dialog.native_unavailable",
        )
    })?;
    let selection = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .set_title("选择 Codex auth.json")
        .blocking_pick_file();
    dispatch_shell_event(&app, ShellEvent::ExternalDialogClosed, None).map_err(|_| {
        PublicError::new(
            PublicErrorCode::NativeDialogUnavailable,
            "dialog.native_unavailable",
        )
    })?;

    let Some(selection) = selection else {
        return get_account_settings(application).await;
    };
    let path = selection
        .into_path()
        .map_err(|_| PublicError::new(PublicErrorCode::InvalidPath, "auth.path.invalid"))?;
    let settings = application
        .select_account(expected_settings_revision, &path)
        .await
        .map_err(|error| error.public::<AuthFileReader>())?;
    let application = application.inner().clone();
    tauri::async_runtime::spawn(async move {
        let _ = application.refresh_selected_account().await;
    });
    Ok(settings)
}

#[cfg(test)]
async fn configure_selected_auth(
    application: &AccountApplication<AuthFileReader>,
    expected_settings_revision: u32,
    path: &std::path::Path,
) -> Result<PublicAccountSettings, PublicError> {
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
    window.set_focus().map_err(platform_error)
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

    eprintln!("QuotaTide: native glass material unavailable; using the opaque fallback");
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

fn spawn_dashboard_event_bridge(app: AppHandle, application: LiveApplication) {
    tauri::async_runtime::spawn(async move {
        let mut changes = application.subscribe_dashboard_changes();
        while changes.changed().await.is_ok() {
            let change: DashboardChanged = *changes.borrow_and_update();
            let _ = app.emit(DASHBOARD_CHANGED_EVENT, change);
        }
    });
}

fn setup_application(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    setup_tray(app)?;
    let app_data = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_data)?;
    secure_app_data_directory(&app_data)?;
    let database_path = app_data.join("state.sqlite3");
    validate_database_targets(&database_path)?;
    let store = tauri::async_runtime::block_on(AccountSettingsStore::open(&database_path))
        .map_err(|_| "failed to open the account settings store")?;
    secure_database_files(&database_path)?;
    let refresh_on_startup = tauri::async_runtime::block_on(store.public_settings())
        .map_err(|_| "failed to read account settings")?
        .configured;
    let usage_client =
        CodexUsageClient::new().map_err(|_| "failed to initialize Codex usage client")?;
    let refresh = RefreshCoordinator::new(
        store.clone(),
        ConfiguredCodexUsageSource::new(store.clone(), usage_client),
        SystemClock,
    );
    let settings = SettingsManager::new(store, AuthFileReader);
    let application = Application::new(AccountApplication::new(settings), refresh);
    app.manage(application.clone());
    spawn_dashboard_event_bridge(app.handle().clone(), application.clone());
    spawn_refresh_scheduler(application, refresh_on_startup);
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        apply_platform_material(&window);
    }
    Ok(())
}

fn handle_run_event(app: &AppHandle, event: &RunEvent) {
    match event {
        RunEvent::Resumed => {
            if let Err(error) = dispatch_shell_event(app, ShellEvent::ResumeRequested, None) {
                eprintln!("QuotaTide: {error}");
            }
            app.state::<LiveApplication>().notify_resume();
        }
        RunEvent::Exit | RunEvent::ExitRequested { .. } => {
            app.state::<LiveApplication>().cancel_scheduler();
        }
        _ => {}
    }
}

/// Starts the `QuotaTide` desktop runtime.
///
/// # Panics
///
/// Panics when the desktop runtime cannot be initialized or its event loop fails.
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(SharedDesktopShell::default())
        .setup(setup_application)
        .on_menu_event(|app, event| {
            if let Some(shell_event) = menu_event_for_id(event.id().0.as_str()) {
                if let Err(error) = dispatch_shell_event(app, shell_event, None) {
                    eprintln!("QuotaTide: {error}");
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
                if id.as_ref() == MAIN_TRAY_ID {
                    if let Err(error) = dispatch_shell_event(app, ShellEvent::LeftClick, Some(rect))
                    {
                        eprintln!("QuotaTide: {error}");
                    }
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
                if let Err(error) = dispatch_shell_event(window.app_handle(), shell_event, None) {
                    eprintln!("QuotaTide: {error}");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_build_info,
            hide_main_window,
            request_manual_refresh,
            begin_external_dialog,
            end_external_dialog,
            get_account_settings,
            get_live_quota,
            select_auth_file,
            update_quota_policy
        ])
        .build(tauri::generate_context!())
        .expect("failed to build the QuotaTide desktop shell");

    app.run(|app, event| handle_run_event(app, &event));
}

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

    use quotatide_core::{AccountApplication, AccountSettingsStore, SettingsManager, ShellEvent};
    use tempfile::tempdir;

    use super::{AuthFileReader, configure_selected_auth, get_build_info, menu_event_for_id};

    #[test]
    fn command_returns_the_public_core_contract() {
        let info = get_build_info();

        assert_eq!(info.product_name, "QuotaTide");
        assert_eq!(info.identifier, "dev.theblind.quotatide");
        assert_eq!(info.stage, "skeleton");
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
