import { invoke } from "@tauri-apps/api/core";

export async function hideMainWindow(): Promise<void> {
  await invoke("hide_main_window");
}

export async function requestManualRefresh(): Promise<number> {
  return await invoke<number>("request_manual_refresh");
}

export async function beginModalActivity(): Promise<void> {
  await invoke("begin_modal_activity");
}

export async function endModalActivity(): Promise<void> {
  await invoke("end_modal_activity");
}
