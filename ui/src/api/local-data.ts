import { invoke } from "@tauri-apps/api/core";

export type StartupMode =
  | "ready"
  | "recovery_required"
  | "unsupported_schema"
  | "storage_permission_denied";

export type PublicStartupState = {
  mode: StartupMode;
  messageKey: string;
  recoveredFromBackup: boolean;
};

export function getStartupState(): Promise<PublicStartupState> {
  return invoke<PublicStartupState>("get_startup_state");
}

export async function retryLocalRecovery(): Promise<void> {
  await invoke("retry_local_recovery");
}

export async function openLocalDataDirectory(): Promise<void> {
  await invoke("open_local_data_directory");
}

export async function exportDiagnostics(): Promise<boolean> {
  return await invoke<boolean>("export_diagnostics");
}

export async function clearAllLocalData(): Promise<void> {
  await invoke("clear_all_local_data");
}
