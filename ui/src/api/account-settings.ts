import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { PublicAccountSettings } from "../bindings/PublicAccountSettings";
import type { QuotaPolicyDraft } from "../bindings/QuotaPolicyDraft";
import type { SettingsChanged } from "../bindings/SettingsChanged";

export async function getAccountSettings(): Promise<PublicAccountSettings> {
  return await invoke<PublicAccountSettings>("get_account_settings");
}

export async function onSettingsChanged(
  callback: (change: SettingsChanged) => void,
): Promise<() => void> {
  return await listen<SettingsChanged>(
    "quotatide://settings-changed",
    (event) => {
      callback(event.payload);
    },
  );
}

export async function selectAuthFile(
  expectedSettingsRevision: number,
): Promise<PublicAccountSettings> {
  return await invoke<PublicAccountSettings>("select_auth_file", {
    expectedSettingsRevision,
  });
}

export async function updateQuotaPolicy(
  expectedSettingsRevision: number,
  draft: QuotaPolicyDraft,
): Promise<PublicAccountSettings> {
  return await invoke<PublicAccountSettings>("update_quota_policy", {
    expectedSettingsRevision,
    draft,
  });
}
