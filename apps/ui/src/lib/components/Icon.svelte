<script lang="ts" module>
  // Inline SVG set: five glyphs, one stroke weight. No icon library so
  // the set stays a deliberate choice (DESIGN.md: quiet, no decoration).
  const paths = {
    back: "M9 4 5 8l4 4",
    forward: "M7 4l4 4-4 4",
    reload: "M12.5 8a4.5 4.5 0 1 1-1.3-3.2M12.5 2.5v2.6h-2.6",
    plus: "M8 4.5v7M4.5 8h7",
    close: "M5 5l6 6M11 5l-6 6",
    tabs: "M3.5 5.5h9v6h-9zM5.5 3.5h7v2",
  } as const;

  export type IconName = keyof typeof paths;
</script>

<script lang="ts">
  let { name }: { name: IconName } = $props();
</script>

<svg class="icon" viewBox="0 0 16 16" aria-hidden="true">
  <path d={paths[name]} />
</svg>
