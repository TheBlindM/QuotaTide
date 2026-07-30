import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { AlertTarget } from "../bindings/AlertTarget";
import type { NotificationPermissionStatus } from "../bindings/NotificationPermissionStatus";
import type { PublicAlertInbox } from "../bindings/PublicAlertInbox";

export async function getAlerts(): Promise<PublicAlertInbox> {
  return await invoke<PublicAlertInbox>("get_alerts");
}

export async function requestSystemNotificationPermission(): Promise<NotificationPermissionStatus> {
  return await invoke<NotificationPermissionStatus>(
    "request_system_notification_permission",
  );
}

export async function onNotificationOpened(
  callback: (target: AlertTarget) => void,
): Promise<() => void> {
  return await listen<AlertTarget>(
    "quotatide://notification-opened",
    (event) => {
      callback(event.payload);
    },
  );
}

export async function onAlertsChanged(callback: () => void): Promise<() => void> {
  return await listen("quotatide://alerts-changed", callback);
}
