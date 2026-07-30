import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type UpdateStatus =
  | "idle"
  | "checking"
  | "up_to_date"
  | "available"
  | "installing"
  | "error";

export type PublicUpdateState = {
  status: UpdateStatus;
  currentVersion: string;
  availableVersion: string | null;
  notes: string | null;
  lastCheckedAtUnixMs: number | null;
  errorCode: string | null;
};

export function getUpdateState(): Promise<PublicUpdateState> {
  return invoke<PublicUpdateState>("get_update_state");
}

export function requestUpdateCheck(): Promise<PublicUpdateState> {
  return invoke<PublicUpdateState>("request_update_check");
}

export function installPendingUpdate(): Promise<PublicUpdateState> {
  return invoke<PublicUpdateState>("install_pending_update");
}

export async function onUpdateState(
  callback: (state: PublicUpdateState) => void,
): Promise<() => void> {
  return await listen<PublicUpdateState>("quotatide://update-state", (event) => {
    callback(event.payload);
  });
}
