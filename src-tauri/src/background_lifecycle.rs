//! Testable launch coordination shared by the Tauri setup and single-instance callback.

/// Marker passed by the current-user autostart entry.
pub const AUTOSTART_ARGUMENT: &str = "--quotatide-autostart";

/// How the current process was requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    /// A user explicitly launched the application.
    User,
    /// The operating system launched the application after login.
    Autostart,
}

impl LaunchMode {
    /// Determines the launch mode without retaining process arguments.
    pub fn from_args(args: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        if args
            .into_iter()
            .any(|argument| argument.as_ref() == AUTOSTART_ARGUMENT)
        {
            Self::Autostart
        } else {
            Self::User
        }
    }

    /// Returns whether this launch is allowed to display the tray window.
    #[must_use]
    pub const fn shows_window(self) -> bool {
        matches!(self, Self::User)
    }
}

/// Starts the primary process resources exactly through the supplied ownership seams.
///
/// # Errors
///
/// Returns the window activation error after both background workers have started.
pub fn start_primary<E>(
    mode: LaunchMode,
    start_scheduler: impl FnOnce(),
    start_delivery_worker: impl FnOnce(),
    show_window: impl FnOnce() -> Result<(), E>,
) -> Result<(), E> {
    start_scheduler();
    start_delivery_worker();
    if mode.shows_window() {
        show_window()?;
    }
    Ok(())
}

/// Handles a second launch without creating any primary-process resource.
///
/// Both existing workers are woken even if window activation fails.
///
/// # Errors
///
/// Returns the existing-window activation error for an explicit user launch.
pub fn notify_secondary<E>(
    mode: LaunchMode,
    show_window: impl FnOnce() -> Result<(), E>,
    wake_scheduler: impl FnOnce(),
    wake_delivery_worker: impl FnOnce(),
) -> Result<(), E> {
    let activation = if mode.shows_window() {
        show_window()
    } else {
        Ok(())
    };
    wake_scheduler();
    wake_delivery_worker();
    activation
}
