//! Tauri-owned wrapper around the small native notification boundary.

use std::sync::Arc;

use quotatide_core::{AlertTarget, NotificationPermissionStatus, SafeNotification};
use quotatide_native_notifications::{
    DeliveryFailureHandler, NativeAlertTarget, NativeNotification, NativeNotificationError,
    NativeNotifier, NativePermissionStatus,
};
use tauri::AppHandle;

use crate::activate_notification;

pub type PlatformNotificationError = NativeNotificationError;

#[derive(Clone)]
pub struct PlatformNotifier {
    native: NativeNotifier,
}

impl PlatformNotifier {
    pub fn new(
        app: &AppHandle,
        delivery_failure: DeliveryFailureHandler,
    ) -> Result<Self, PlatformNotificationError> {
        let app_id = app.config().identifier.clone();
        let activation_app = app.clone();
        let native = NativeNotifier::new(
            app_id,
            Arc::new(move |target| {
                activate_notification(&activation_app, map_target_from_native(target));
            }),
            delivery_failure,
        )?;
        Ok(Self { native })
    }

    pub async fn permission_state(
        &self,
    ) -> Result<NotificationPermissionStatus, PlatformNotificationError> {
        let native = self.native.clone();
        tauri::async_runtime::spawn_blocking(move || native.permission_state())
            .await
            .map_err(|_| NativeNotificationError::Authorization)?
            .map(map_permission)
    }

    pub async fn request_permission(
        &self,
    ) -> Result<NotificationPermissionStatus, PlatformNotificationError> {
        let native = self.native.clone();
        tauri::async_runtime::spawn_blocking(move || native.request_permission())
            .await
            .map_err(|_| NativeNotificationError::Authorization)?
            .map(map_permission)
    }

    pub async fn notify(
        &self,
        notification: SafeNotification,
    ) -> Result<(), PlatformNotificationError> {
        let native = self.native.clone();
        let notification = NativeNotification {
            delivery_key: notification.delivery_key,
            title: notification.title,
            body: notification.body,
            target: map_target_to_native(notification.target),
        };
        tauri::async_runtime::spawn_blocking(move || native.notify(&notification))
            .await
            .map_err(|_| NativeNotificationError::Delivery)?
    }
}

const fn map_target_to_native(target: AlertTarget) -> NativeAlertTarget {
    match target {
        AlertTarget::Today => NativeAlertTarget::Today,
        AlertTarget::Radar => NativeAlertTarget::Radar,
        AlertTarget::Source => NativeAlertTarget::Source,
    }
}

const fn map_target_from_native(target: NativeAlertTarget) -> AlertTarget {
    match target {
        NativeAlertTarget::Today => AlertTarget::Today,
        NativeAlertTarget::Radar => AlertTarget::Radar,
        NativeAlertTarget::Source => AlertTarget::Source,
    }
}

const fn map_permission(status: NativePermissionStatus) -> NotificationPermissionStatus {
    match status {
        NativePermissionStatus::Unknown => NotificationPermissionStatus::Unknown,
        NativePermissionStatus::Granted => NotificationPermissionStatus::Granted,
        NativePermissionStatus::Denied => NotificationPermissionStatus::Denied,
    }
}
