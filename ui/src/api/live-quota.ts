import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { PublicLiveQuota } from "../bindings/PublicLiveQuota";

export async function getLiveQuota(): Promise<PublicLiveQuota | null> {
  return await invoke<PublicLiveQuota | null>("get_live_quota");
}

export async function onDashboardChanged(
  callback: () => void,
): Promise<UnlistenFn> {
  return await listen("quotatide://dashboard-changed", callback);
}

type RefreshActivityEvent = {
  refreshing: boolean;
};

export async function onRefreshActivity(
  callback: (refreshing: boolean) => void,
): Promise<UnlistenFn> {
  return await listen<RefreshActivityEvent>(
    "quotatide://refresh-activity",
    (event) => {
      callback(event.payload.refreshing);
    },
  );
}
