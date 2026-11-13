<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  // Real favicon first: the Rust side fetches <origin>/favicon.ico once,
  // caches it, and serves it through the favicon:// scheme. Until that
  // resolves (or when the site has none), the honest placeholder shows the
  // domain's first letter on a deterministic muted tint.
  let { url, size = 16 }: { url: string | null | undefined; size?: number } = $props();

  let icon = $state<string | null>(null);
  let failed = $state(false);

  const host = $derived.by(() => {
    if (!url) return "";
    try {
      return new URL(url).hostname.replace(/^www\./, "");
    } catch {
      return "";
    }
  });
  const hue = $derived.by(() => {
    let h = 0;
    for (let i = 0; i < host.length; i++) h = (h * 31 + host.charCodeAt(i)) % 360;
    return h;
  });
  const letter = $derived((host.match(/[a-z0-9]/i)?.[0] ?? "?").toUpperCase());

  // Re-resolve per url; repeated asks hit the in-memory origin cache.
  $effect(() => {
    const target = url ?? null;
    icon = null;
    failed = false;
    if (!target || !target.startsWith("http")) return;
    let alive = true;
    void invoke<string | null>("favicon_url", { url: target })
      .then((src) => {
        if (alive) icon = src ?? null;
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  });
</script>

{#if icon && !failed}
  <span class="img" style:width={`${size}px`} style:height={`${size}px`}>
    <img src={icon} alt="" loading="lazy" onerror={() => (failed = true)} />
  </span>
{:else}
  <span
    class="fav"
    style:width={`${size}px`}
    style:height={`${size}px`}
    style:font-size={`${Math.round(size * 0.62)}px`}
    style:--h={hue}
    aria-hidden="true"
  >
    {letter}
  </span>
{/if}

<style>
  .img {
    flex: none;
    display: inline-grid;
    place-items: center;
  }
  .img img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    border-radius: 3px;
  }
  .fav {
    flex: none;
    display: inline-grid;
    place-items: center;
    border-radius: 4px;
    background: hsl(var(--h) 32% 88%);
    color: hsl(var(--h) 45% 28%);
    font-weight: 600;
    line-height: 1;
    user-select: none;
  }
  @media (prefers-color-scheme: dark) {
    .fav {
      background: hsl(var(--h) 24% 22%);
      color: hsl(var(--h) 34% 76%);
    }
  }
</style>
