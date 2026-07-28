import { invoke } from "@tauri-apps/api/core";

import type { BuildInfo } from "../bindings/BuildInfo";

export function loadBuildInfo(): Promise<BuildInfo> {
  return invoke<BuildInfo>("get_build_info");
}
