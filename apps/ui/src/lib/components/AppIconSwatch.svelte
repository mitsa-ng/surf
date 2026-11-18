<script lang="ts">
  // The app icon variants, drawn from the same masters that render
  // icons/icon-{light,blue,dark}.png (path data kept in sync with
  // apps/surf/src-tauri/icons/masters/*.svg).
  let { variant, size = 44 }: { variant: "light" | "blue" | "dark"; size?: number } = $props();

  const themes = {
    light: { base: "#ecebe9", wave: "#0a7fa8" },
    blue: { base: "#0a7fa8", wave: "#f6f5f4" },
    dark: { base: "#1d1d1f", wave: "#4fb3d9" },
  } as const;
</script>

<svg viewBox="0 0 1024 1024" width={size} height={size} aria-hidden="true">
  <rect x="100" y="100" width="824" height="824" rx="186" fill={themes[variant].base} />
  <path
    d="M 168 626 C 288 454 408 454 512 552 C 616 650 736 650 856 472"
    fill="none"
    stroke={themes[variant].wave}
    stroke-width="96"
    stroke-linecap="round"
  />
  {#if variant === "light"}
    <path
      d="M 200 786 C 310 638 420 638 512 724 C 604 810 714 810 824 658"
      fill="none"
      stroke="#1d1d1f"
      stroke-width="56"
      stroke-linecap="round"
      opacity="0.12"
    />
  {/if}
</svg>
