import { invoke } from "@tauri-apps/api/core";

import type { PublicResetCredits } from "../bindings/PublicResetCredits";

export function getResetCredits(): Promise<PublicResetCredits> {
  return invoke<PublicResetCredits>("get_reset_credits");
}
