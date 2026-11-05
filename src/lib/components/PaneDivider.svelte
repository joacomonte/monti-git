<!-- Draggable line between two panes; `size` is the pane after it (width when vertical, height otherwise). -->
<script lang="ts">
  let { vertical, size = $bindable(), min, max, id }: { vertical: boolean; size: number; min: number; max: number; id: string } = $props();
  let drag: { start: number; initial: number } | null = null;

  function down(e: PointerEvent) {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId); // moves keep coming here even off the line
    drag = { start: vertical ? e.clientX : e.clientY, initial: size };
  }
  function move(e: PointerEvent) {
    if (drag) size = Math.min(max, Math.max(min, drag.initial - ((vertical ? e.clientX : e.clientY) - drag.start)));
  }
</script>

<div
  role="separator"
  aria-orientation={vertical ? "vertical" : "horizontal"}
  {id}
  class="relative shrink-0 bg-border {vertical ? 'w-px cursor-col-resize' : 'h-px cursor-row-resize'}"
  onpointerdown={down}
  onpointermove={move}
  onpointerup={() => (drag = null)}
  onpointercancel={() => (drag = null)}
>
  <div class="absolute {vertical ? '-inset-x-1 inset-y-0' : '-inset-y-1 inset-x-0'}"></div>
</div>
