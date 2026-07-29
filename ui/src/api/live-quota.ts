import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { DashboardChanged } from "../bindings/DashboardChanged";
import type { PublicLiveQuotaState } from "../bindings/PublicLiveQuotaState";

export async function getLiveQuota(): Promise<PublicLiveQuotaState> {
  return await invoke<PublicLiveQuotaState>("get_live_quota");
}

export async function onDashboardChanged(
  callback: (change: DashboardChanged) => void,
): Promise<UnlistenFn> {
  return await listen<DashboardChanged>(
    "quotatide://dashboard-changed",
    (event) => {
      callback(event.payload);
    },
  );
}
