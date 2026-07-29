import { invoke } from "@tauri-apps/api/core";

import type { PublicAccountSettings } from "../bindings/PublicAccountSettings";

export async function getAccountSettings(): Promise<PublicAccountSettings> {
  return await invoke<PublicAccountSettings>("get_account_settings");
}

export async function selectAuthFile(): Promise<PublicAccountSettings> {
  return await invoke<PublicAccountSettings>("select_auth_file");
}
