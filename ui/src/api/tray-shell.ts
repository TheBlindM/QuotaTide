import { invoke } from "@tauri-apps/api/core";

export async function hideMainWindow(): Promise<void> {
  await invoke("hide_main_window");
}

export async function requestManualRefresh(): Promise<void> {
  await invoke("request_manual_refresh");
}
