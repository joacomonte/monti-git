<!-- Draggable line between two panes; `size` is the pane after it (width when vertical, height otherwise). -->
<script lang="ts">
  let { vertical, size = $bindable(), min, max }: { vertical: boolean; size: number; min: number; max: number } = $props();

  function down(e: PointerEvent) {
    const start = vertical ? e.clientX : e.clientY, initial = size;
    const move = (m: PointerEvent) => (size = Math.min(max, Math.max(min, initial - ((vertical ? m.clientX : m.clientY) - start))));
    const up = () => removeEventListener("pointermove", move);
    addEventListener("pointermove", move);
    addEventListener("pointerup", up, { once: true });
  }
</script>

<div
  role="separator"
  aria-orientation={vertical ? "vertical" : "horizontal"}
  class="relative shrink-0 bg-border {vertical ? 'w-px cursor-col-resize' : 'h-px cursor-row-resize'}"
  onpointerdown={down}
>
  <div class="absolute {vertical ? '-inset-x-1 inset-y-0' : '-inset-y-1 inset-x-0'}"></div>
</div>
