// a2ui renderer wiring. The processor's lifecycle belongs to the panel
// component (mounted = one agent turn); this module only holds the
// shared stream control that Rust needs.
import { invoke } from "@tauri-apps/api/core";

/** Panel visibility drives the Rust provider stream. */
export function setAgentPanelOpen(open: boolean) {
  void invoke("agent_panel_set_open", { open }).catch((e) => console.error(String(e)));
}

/** A component action (Button click) forwarded to the Rust action loop. */
export function sendAgentAction(name: string, context: Record<string, unknown>) {
  void invoke("agent_action", { name, context }).catch((e) => console.error(String(e)));
}

export interface AgentLlmConfig {
  baseUrl: string;
  model: string;
  hasKey: boolean;
}

export function agentLlmConfigGet() {
  return invoke<AgentLlmConfig>("agent_llm_config_get");
}

export function agentLlmConfigSet(baseUrl: string, model: string, apiKey: string | null, clearKey: boolean) {
  return invoke<void>("agent_llm_config_set", {
    baseUrl,
    model,
    apiKey: apiKey ?? null,
    clearKey,
  });
}
