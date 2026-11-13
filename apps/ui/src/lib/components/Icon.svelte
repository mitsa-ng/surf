<script lang="ts" module>
  // Inline SVG set: one stroke weight, no icon library so the set stays a
  // deliberate choice (DESIGN.md: quiet, no decoration). Glyphs are added
  // only when a control genuinely needs them.
  const paths = {
    back: "M9 4 5 8l4 4",
    forward: "M7 4l4 4-4 4",
    reload: "M12.5 8a4.5 4.5 0 1 1-1.3-3.2M12.5 2.5v2.6h-2.6",
    plus: "M8 4.5v7M4.5 8h7",
    close: "M5 5l6 6M11 5l-6 6",
    tabs: "M3.5 5.5h9v6h-9zM5.5 3.5h7v2",
    star: "M8 2.5l1.7 3.6 3.8.5-2.8 2.6.7 3.8L8 11.1l-3.4 1.9.7-3.8L2.5 6.6l3.8-.5z",
    search: "M11 7a4 4 0 1 1-8 0 4 4 0 0 1 8 0zM10.5 10.5 13 13",
    download: "M8 2.5v7.5M5 7.5l3 3 3-3M3.5 13h9",
    history: "M13 8A5 5 0 1 1 3 8a5 5 0 0 1 10 0zM8 5.5V8l1.8 1.4",
    settings: "M3 4.5h10M3 8h10M3 11.5h10M6.8 3.2a1.3 1.3 0 1 0 0 2.6a1.3 1.3 0 1 0 0 -2.6M10.6 6.7a1.3 1.3 0 1 0 0 2.6a1.3 1.3 0 1 0 0 -2.6M5.6 10.2a1.3 1.3 0 1 0 0 2.6a1.3 1.3 0 1 0 0 -2.6",
    lock: "M5.5 7V5.5a2.5 2.5 0 0 1 5 0V7M3.5 7h9v6.5h-9z",
    chevronUp: "M4.5 9.5 8 6l3.5 3.5",
    chevronDown: "M4.5 6.5 8 10l3.5-3.5",
    list: "M5.5 4.5h8M5.5 8h8M5.5 11.5h8",
    puzzle: "M4 4h3v1.5a1.5 1.5 0 1 0 2 0V4h3v3h-1.5a1.5 1.5 0 1 0 0 2H12v3H9v-1.5a1.5 1.5 0 1 0-2 0V12H4V9h1.5a1.5 1.5 0 1 0 0-2H4z",
    agent: "M2.5 3.5h11v7.5h-6.5L4 13.5v-2.5H2.5zM5 6.5h6M5 8.5h4",
    mute: "M3 6.5h2.5L9 3.5v9L5.5 9.5H3zM11 6l3 3M14 6l-3 3",
  } as const;

  export type IconName = keyof typeof paths;
</script>

<script lang="ts">
  let { name }: { name: IconName } = $props();
</script>

<svg class="icon" viewBox="0 0 16 16" aria-hidden="true">
  <path d={paths[name]} />
</svg>
