<script lang="ts">
  import { ui, savePromptedPassword, dismissPromptedPassword } from "../ui.svelte";
  import Icon from "./Icon.svelte";

  // The save prompt floats under the toolbar like the find bar. It shows
  // the host and the username only; the password never entered the chrome.
  function onKeydown(e: KeyboardEvent) {
    if (!ui.pwPrompt) return;
    if (e.key === "Escape") {
      e.stopPropagation();
      void dismissPromptedPassword();
    } else if (e.key === "Enter") {
      e.preventDefault();
      void savePromptedPassword();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if ui.pwPrompt}
  <div class="pw-bar" role="alertdialog" aria-label="儲存密碼">
    <span class="ask"
      >要儲存 <strong>{ui.pwPrompt.host}</strong> 的密碼嗎？（{ui.pwPrompt.user}，存於 macOS 鑰匙圈）</span
    >
    <button class="primary" onclick={() => void savePromptedPassword()}>儲存</button>
    <button onclick={() => void dismissPromptedPassword()}>不要</button>
  </div>
{/if}

<style>
  .pw-bar {
    position: absolute;
    top: 91px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 8px 0 14px;
    background: var(--toolbar-surface);
    border-radius: 10px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.18);
  }
  .ask {
    font-size: 12px;
    color: var(--text);
    white-space: nowrap;
  }
  button {
    height: 26px;
    padding: 0 12px;
    border-radius: 6px;
    font-size: 12px;
    color: var(--text-secondary);
  }
  button:hover {
    background: var(--hover);
    color: var(--text);
  }
  .primary {
    background: var(--accent);
    color: #fff;
  }
  .primary:hover {
    background: var(--accent);
    opacity: 0.9;
    color: #fff;
  }
</style>
