import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { PublicSettings } from "../bindings/PublicSettings";
import type { SettingsChanged } from "../bindings/SettingsChanged";
import type { SettingsDraft } from "../bindings/SettingsDraft";

export async function getSettings(): Promise<PublicSettings> {
  return await invoke<PublicSettings>("get_settings");
}

export async function saveSettings(
  draft: SettingsDraft,
): Promise<PublicSettings> {
  return await invoke<PublicSettings>("save_settings", { draft });
}

export async function sendTestEmail(): Promise<number> {
  return await invoke<number>("send_test_email");
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
