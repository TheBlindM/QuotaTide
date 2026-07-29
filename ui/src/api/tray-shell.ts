import { invoke } from "@tauri-apps/api/core";

export async function hideMainWindow(): Promise<void> {
  await invoke("hide_main_window");
}

export async function requestManualRefresh(): Promise<number> {
  return await invoke<number>("request_manual_refresh");
}

export async function beginExternalDialog(): Promise<void> {
  await invoke("begin_external_dialog");
}

export async function endExternalDialog(): Promise<void> {
  await invoke("end_external_dialog");
}
