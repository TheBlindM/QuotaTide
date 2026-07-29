/// A point in the operating system's physical pixel coordinate space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicalPoint {
    x: f64,
    y: f64,
}

impl PhysicalPoint {
    /// Creates a physical point.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Returns the horizontal coordinate.
    #[must_use]
    pub const fn x(self) -> f64 {
        self.x
    }

    /// Returns the vertical coordinate.
    #[must_use]
    pub const fn y(self) -> f64 {
        self.y
    }
}

/// A size measured in operating system physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicalSize {
    width: f64,
    height: f64,
}

impl PhysicalSize {
    /// Creates a physical size.
    #[must_use]
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    /// Returns the horizontal extent.
    #[must_use]
    pub const fn width(self) -> f64 {
        self.width
    }

    /// Returns the vertical extent.
    #[must_use]
    pub const fn height(self) -> f64 {
        self.height
    }

    const fn is_finite_and_positive(self) -> bool {
        self.width.is_finite() && self.height.is_finite() && self.width > 0.0 && self.height > 0.0
    }
}

/// A rectangle in the operating system's physical pixel coordinate space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicalRect {
    origin: PhysicalPoint,
    size: PhysicalSize,
}

impl PhysicalRect {
    /// Creates a physical rectangle.
    #[must_use]
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            origin: PhysicalPoint::new(x, y),
            size: PhysicalSize::new(width, height),
        }
    }

    /// Returns the rectangle's top-left point.
    #[must_use]
    pub const fn origin(self) -> PhysicalPoint {
        self.origin
    }

    /// Returns the rectangle's size.
    #[must_use]
    pub const fn size(self) -> PhysicalSize {
        self.size
    }

    const fn is_finite_and_positive(self) -> bool {
        self.origin.x.is_finite() && self.origin.y.is_finite() && self.size.is_finite_and_positive()
    }
}

/// Places the existing tray window beneath the tray icon and clamps it to the
/// selected display's work area.
#[must_use]
pub fn place_tray_window(
    tray: PhysicalRect,
    work_area: PhysicalRect,
    window: PhysicalSize,
    gap: f64,
) -> PhysicalPoint {
    let geometry_is_valid = work_area.is_finite_and_positive()
        && window.is_finite_and_positive()
        && window.width <= work_area.size.width
        && window.height <= work_area.size.height
        && gap.is_finite()
        && gap >= 0.0;
    if !geometry_is_valid {
        return PhysicalPoint::new(work_area.origin.x, work_area.origin.y);
    }

    if !tray.is_finite_and_positive() {
        return PhysicalPoint::new(
            work_area.origin.x + (work_area.size.width - window.width) / 2.0,
            (work_area.origin.y + gap)
                .min(work_area.origin.y + work_area.size.height - window.height),
        );
    }

    let centered_x = tray.origin.x + (tray.size.width - window.width) / 2.0;
    let centered_y = tray.origin.y + (tray.size.height - window.height) / 2.0;
    let max_x = work_area.origin.x + work_area.size.width - window.width;
    let max_y = work_area.origin.y + work_area.size.height - window.height;
    let distances = [
        (tray.origin.y - work_area.origin.y).abs(),
        (work_area.origin.y + work_area.size.height - tray.origin.y - tray.size.height).abs(),
        (tray.origin.x - work_area.origin.x).abs(),
        (work_area.origin.x + work_area.size.width - tray.origin.x - tray.size.width).abs(),
    ];
    let closest_edge = distances
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| left.total_cmp(right))
        .map_or(0, |(index, _)| index);
    let (x, y) = match closest_edge {
        0 => (centered_x, tray.origin.y + tray.size.height + gap),
        1 => (centered_x, tray.origin.y - window.height - gap),
        2 => (tray.origin.x + tray.size.width + gap, centered_y),
        _ => (tray.origin.x - window.width - gap, centered_y),
    };

    PhysicalPoint::new(
        x.clamp(work_area.origin.x, max_x),
        y.clamp(work_area.origin.y, max_y),
    )
}

/// User and platform events handled by the single tray window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellEvent {
    /// The primary tray button was released.
    LeftClick,
    /// The native menu requested the existing window.
    OpenRequested,
    /// The transient window lost focus.
    FocusLost,
    /// The operating system requested that the window close.
    CloseRequested,
    /// The native menu requested a manual data refresh.
    RefreshRequested,
    /// The native menu explicitly requested application termination.
    ExitRequested,
    /// The operating system resumed the application after sleep.
    ResumeRequested,
}

/// Platform operation requested by the tray shell state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellEffect {
    /// No platform operation is needed.
    None,
    /// Show, position, and focus the existing window.
    Show,
    /// Hide the existing window.
    Hide,
    /// Ask the application layer to start one manual refresh.
    Refresh,
    /// Recalculate placement for the visible window without creating another.
    Reposition,
    /// Terminate the application event loop.
    Exit,
}

/// State of the one tray-owned Weekly Ledger window.
#[derive(Debug, Default)]
pub struct TrayShell {
    visible: bool,
}

impl TrayShell {
    /// Creates a hidden tray shell.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reports whether the existing window should currently be visible.
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        self.visible
    }

    /// Applies an event and returns the platform operation needed to realize it.
    pub const fn handle(&mut self, event: ShellEvent) -> ShellEffect {
        match event {
            ShellEvent::LeftClick => {
                self.visible = !self.visible;
                if self.visible {
                    ShellEffect::Show
                } else {
                    ShellEffect::Hide
                }
            }
            ShellEvent::OpenRequested => {
                if self.visible {
                    ShellEffect::None
                } else {
                    self.visible = true;
                    ShellEffect::Show
                }
            }
            ShellEvent::FocusLost | ShellEvent::CloseRequested => {
                if self.visible {
                    self.visible = false;
                    ShellEffect::Hide
                } else {
                    ShellEffect::None
                }
            }
            ShellEvent::RefreshRequested => ShellEffect::Refresh,
            ShellEvent::ExitRequested => ShellEffect::Exit,
            ShellEvent::ResumeRequested => {
                if self.visible {
                    ShellEffect::Reposition
                } else {
                    ShellEffect::None
                }
            }
        }
    }
}
