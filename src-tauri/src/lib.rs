//! `QuotaTide` desktop shell.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use quotatide_core::{
    BuildInfo, PhysicalRect as CoreRect, PhysicalSize as CoreSize, ShellEffect, ShellEvent,
    TrayShell, place_tray_window,
};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
use tauri::utils::WindowEffect;
use tauri::utils::config::WindowEffectsConfig;
use tauri::{
    App, AppHandle, Emitter, Manager, PhysicalPosition, Rect, RunEvent, WebviewWindow, WindowEvent,
};

const MAIN_WINDOW_LABEL: &str = "main";
const MAIN_TRAY_ID: &str = "main";
const WINDOW_GAP: f64 = 8.0;
const MANUAL_REFRESH_COOLDOWN: Duration = Duration::from_secs(30);

#[derive(Debug, Default)]
struct RefreshGate {
    last_started: Option<Instant>,
}

impl RefreshGate {
    fn try_start(&mut self, now: Instant) -> bool {
        if self
            .last_started
            .is_some_and(|started| now.duration_since(started) < MANUAL_REFRESH_COOLDOWN)
        {
            return false;
        }

        self.last_started = Some(now);
        true
    }
}

#[derive(Debug, Default)]
struct DesktopShell {
    shell: TrayShell,
    last_tray_rect: Option<Rect>,
    refresh_gate: RefreshGate,
}

type SharedDesktopShell = Mutex<DesktopShell>;

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
#[allow(clippy::needless_pass_by_value)] // Tauri injects AppHandle command arguments by value.
fn request_manual_refresh(app: AppHandle) -> Result<(), String> {
    dispatch_shell_event(&app, ShellEvent::RefreshRequested, None)
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
        if event == ShellEvent::RefreshRequested && !desktop.refresh_gate.try_start(Instant::now())
        {
            return Ok(());
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
        ShellEffect::Refresh => app
            .emit("quotatide://manual-refresh", ())
            .map_err(platform_error),
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

/// Starts the `QuotaTide` desktop runtime.
///
/// # Panics
///
/// Panics when the desktop runtime cannot be initialized or its event loop fails.
pub fn run() {
    let app = tauri::Builder::default()
        .manage(SharedDesktopShell::default())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            setup_tray(app)?;
            if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                apply_platform_material(&window);
            }
            Ok(())
        })
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
            end_external_dialog
        ])
        .build(tauri::generate_context!())
        .expect("failed to build the QuotaTide desktop shell");

    app.run(|app, event| {
        if matches!(event, RunEvent::Resumed) {
            if let Err(error) = dispatch_shell_event(app, ShellEvent::ResumeRequested, None) {
                eprintln!("QuotaTide: {error}");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use quotatide_core::ShellEvent;

    use super::{RefreshGate, get_build_info, menu_event_for_id};

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
    fn manual_refresh_gate_enforces_thirty_seconds() {
        let now = Instant::now();
        let mut gate = RefreshGate::default();

        assert!(gate.try_start(now));
        assert!(!gate.try_start(now + Duration::from_secs(29)));
        assert!(gate.try_start(now + Duration::from_secs(30)));
    }
}
