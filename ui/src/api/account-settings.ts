import { invoke } from "@tauri-apps/api/core";

import type { PublicAccountSettings } from "../bindings/PublicAccountSettings";
import type { QuotaPolicyDraft } from "../bindings/QuotaPolicyDraft";

export async function getAccountSettings(): Promise<PublicAccountSettings> {
  return await invoke<PublicAccountSettings>("get_account_settings");
}

export async function selectAuthFile(
  expectedSettingsRevision: number,
): Promise<PublicAccountSettings> {
  return await invoke<PublicAccountSettings>("select_auth_file", {
    expectedSettingsRevision,
  });
}

export async function saveSettings(
  expectedSettingsRevision: number,
  draft: QuotaPolicyDraft,
): Promise<PublicAccountSettings> {
  return await invoke<PublicAccountSettings>("save_settings", {
    expectedSettingsRevision,
    draft,
  });
}
