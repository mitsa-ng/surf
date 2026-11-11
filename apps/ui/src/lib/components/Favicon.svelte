<script lang="ts">
  // Honest placeholder, not a fake favicon: the domain's first letter on
  // a deterministic muted tint from the same domain hash. The real
  // favicon pipeline is a later round.
  let { url, size = 16 }: { url: string | null | undefined; size?: number } = $props();

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
</script>

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

<style>
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
