use std::cell::Cell;

use quotatide_lib::background_lifecycle::{
    AUTOSTART_ARGUMENT, LaunchMode, notify_secondary, start_primary,
};

#[test]
fn login_and_secondary_launches_share_one_background_runtime() {
    let scheduler_starts = Cell::new(0);
    let delivery_starts = Cell::new(0);
    let window_opens = Cell::new(0);
    let scheduler_wakes = Cell::new(0);
    let delivery_wakes = Cell::new(0);

    start_primary(
        LaunchMode::from_args(["quotatide", AUTOSTART_ARGUMENT]),
        || scheduler_starts.set(scheduler_starts.get() + 1),
        || delivery_starts.set(delivery_starts.get() + 1),
        || {
            window_opens.set(window_opens.get() + 1);
            Ok::<(), ()>(())
        },
    )
    .expect("login launch");

    notify_secondary(
        LaunchMode::from_args(["quotatide", AUTOSTART_ARGUMENT]),
        || {
            window_opens.set(window_opens.get() + 1);
            Ok::<(), ()>(())
        },
        || scheduler_wakes.set(scheduler_wakes.get() + 1),
        || delivery_wakes.set(delivery_wakes.get() + 1),
    )
    .expect("secondary login launch");

    assert_eq!(scheduler_starts.get(), 1);
    assert_eq!(delivery_starts.get(), 1);
    assert_eq!(window_opens.get(), 0);
    assert_eq!(scheduler_wakes.get(), 1);
    assert_eq!(delivery_wakes.get(), 1);

    notify_secondary(
        LaunchMode::from_args(["quotatide"]),
        || {
            window_opens.set(window_opens.get() + 1);
            Ok::<(), ()>(())
        },
        || scheduler_wakes.set(scheduler_wakes.get() + 1),
        || delivery_wakes.set(delivery_wakes.get() + 1),
    )
    .expect("secondary user launch");

    assert_eq!(scheduler_starts.get(), 1);
    assert_eq!(delivery_starts.get(), 1);
    assert_eq!(window_opens.get(), 0);
    assert_eq!(scheduler_wakes.get(), 2);
    assert_eq!(delivery_wakes.get(), 2);
}

#[test]
fn secondary_launch_wakes_workers_without_attempting_window_activation() {
    let scheduler_wakes = Cell::new(0);
    let delivery_wakes = Cell::new(0);

    let result = notify_secondary(
        LaunchMode::User,
        || Err("window unavailable"),
        || scheduler_wakes.set(scheduler_wakes.get() + 1),
        || delivery_wakes.set(delivery_wakes.get() + 1),
    );

    assert_eq!(result, Ok(()));
    assert_eq!(scheduler_wakes.get(), 1);
    assert_eq!(delivery_wakes.get(), 1);
}
