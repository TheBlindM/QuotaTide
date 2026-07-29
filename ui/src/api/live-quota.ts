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
