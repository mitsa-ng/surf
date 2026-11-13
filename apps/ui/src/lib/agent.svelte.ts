// a2ui renderer wiring. The processor's lifecycle belongs to the panel
// component (mounted = one agent turn); this module only holds the
// shared stream control that Rust needs.
import { invoke } from "@tauri-apps/api/core";

/** Panel visibility drives the Rust provider stream. */
export function setAgentPanelOpen(open: boolean) {
  void invoke("agent_panel_set_open", { open }).catch((e) => console.error(String(e)));
}
