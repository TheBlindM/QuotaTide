use quotatide_core::{
    PhysicalPoint, PhysicalRect, PhysicalSize, ShellEffect, ShellEvent, TrayShell,
    place_tray_window,
};

#[test]
fn left_click_toggles_the_single_tray_window() {
    let mut shell = TrayShell::new();

    assert_eq!(shell.handle(ShellEvent::LeftClick), ShellEffect::Show);
    assert!(shell.is_visible());
    assert_eq!(shell.handle(ShellEvent::LeftClick), ShellEffect::Hide);
    assert!(!shell.is_visible());
}

#[test]
fn close_and_focus_loss_hide_instead_of_terminating() {
    let mut shell = TrayShell::new();
    shell.handle(ShellEvent::OpenRequested);

    assert_eq!(shell.handle(ShellEvent::FocusLost), ShellEffect::Hide);
    assert_eq!(shell.handle(ShellEvent::CloseRequested), ShellEffect::None);
    assert!(!shell.is_visible());
}

#[test]
fn native_menu_actions_keep_refresh_and_exit_explicit() {
    let mut shell = TrayShell::new();

    assert_eq!(
        shell.handle(ShellEvent::RefreshRequested),
        ShellEffect::Refresh
    );
    assert_eq!(shell.handle(ShellEvent::ExitRequested), ShellEffect::Exit);
    assert!(!shell.is_visible());
}

#[test]
fn repeated_open_and_resume_reuse_and_reposition_the_existing_window() {
    let mut shell = TrayShell::new();

    assert_eq!(shell.handle(ShellEvent::OpenRequested), ShellEffect::Show);
    assert_eq!(shell.handle(ShellEvent::OpenRequested), ShellEffect::None);
    assert_eq!(
        shell.handle(ShellEvent::ResumeRequested),
        ShellEffect::Reposition
    );
    assert!(shell.is_visible());
}

#[test]
fn external_dialogs_suspend_focus_loss_auto_hide() {
    let mut shell = TrayShell::new();
    shell.handle(ShellEvent::OpenRequested);

    assert_eq!(
        shell.handle(ShellEvent::ExternalDialogOpened),
        ShellEffect::None
    );
    assert_eq!(shell.handle(ShellEvent::FocusLost), ShellEffect::None);
    assert!(shell.is_visible());

    assert_eq!(
        shell.handle(ShellEvent::ExternalDialogClosed),
        ShellEffect::None
    );
    assert_eq!(shell.handle(ShellEvent::FocusLost), ShellEffect::Hide);
}

#[test]
fn tray_anchor_is_clamped_inside_the_display_work_area() {
    let work_area = PhysicalRect::new(0.0, 0.0, 1_440.0, 900.0);
    let tray = PhysicalRect::new(1_300.0, 0.0, 24.0, 24.0);

    assert_eq!(
        place_tray_window(tray, work_area, PhysicalSize::new(420.0, 680.0), 8.0),
        PhysicalPoint::new(1_020.0, 32.0)
    );
}

#[test]
fn bottom_tray_places_the_window_above_the_taskbar() {
    let work_area = PhysicalRect::new(0.0, 0.0, 1_440.0, 900.0);
    let tray = PhysicalRect::new(10.0, 876.0, 24.0, 24.0);

    assert_eq!(
        place_tray_window(tray, work_area, PhysicalSize::new(420.0, 680.0), 8.0),
        PhysicalPoint::new(0.0, 188.0)
    );
}

#[test]
fn side_taskbars_place_the_window_beside_the_tray() {
    let work_area = PhysicalRect::new(0.0, 0.0, 1_440.0, 900.0);
    let window = PhysicalSize::new(420.0, 680.0);

    assert_eq!(
        place_tray_window(
            PhysicalRect::new(0.0, 438.0, 24.0, 24.0),
            work_area,
            window,
            8.0
        ),
        PhysicalPoint::new(32.0, 110.0)
    );
    assert_eq!(
        place_tray_window(
            PhysicalRect::new(1_416.0, 438.0, 24.0, 24.0),
            work_area,
            window,
            8.0
        ),
        PhysicalPoint::new(988.0, 110.0)
    );
}

#[test]
fn invalid_tray_geometry_falls_back_to_top_center() {
    assert_eq!(
        place_tray_window(
            PhysicalRect::new(f64::NAN, 0.0, 24.0, 24.0),
            PhysicalRect::new(0.0, 0.0, 1_440.0, 900.0),
            PhysicalSize::new(420.0, 680.0),
            8.0
        ),
        PhysicalPoint::new(510.0, 8.0)
    );
}
