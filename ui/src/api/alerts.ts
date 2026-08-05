import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { AlertTarget } from "../bindings/AlertTarget";
import type { NotificationPermissionStatus } from "../bindings/NotificationPermissionStatus";
import type { PublicAlertInbox } from "../bindings/PublicAlertInbox";

export type NotificationActivation = {
  target: AlertTarget;
  activationId: number;
};

export async function getAlerts(): Promise<PublicAlertInbox> {
  return await invoke<PublicAlertInbox>("get_alerts");
}

export async function dismissAlert(
  eventId: number,
): Promise<PublicAlertInbox> {
  return await invoke<PublicAlertInbox>("dismiss_alert", { eventId });
}

export async function dismissAllAlerts(): Promise<PublicAlertInbox> {
  return await invoke<PublicAlertInbox>("dismiss_all_alerts");
}

export async function requestSystemNotificationPermission(): Promise<NotificationPermissionStatus> {
  return await invoke<NotificationPermissionStatus>(
    "request_system_notification_permission",
  );
}

export async function onNotificationOpened(
  callback: (activation: NotificationActivation) => void,
): Promise<() => void> {
  return await listen<NotificationActivation>(
    "quotatide://notification-opened",
    (event) => {
      callback(event.payload);
    },
  );
}

export async function onAlertsChanged(callback: () => void): Promise<() => void> {
  return await listen("quotatide://alerts-changed", callback);
}
