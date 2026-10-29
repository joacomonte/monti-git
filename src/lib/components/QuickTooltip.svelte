<!-- App-wide tooltip for every `title` attribute: the native one waits ~1s (fixed by the OS), this one shows after 250ms.
     On hover the title moves to data-title (so the native tooltip never shows) and comes back on leave. -->
<script lang="ts">
  const DELAY = 250;
  let tip = $state<{ text: string; x: number; y: number; below: boolean } | null>(null);
  let current: HTMLElement | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;

  function hide() {
    clearTimeout(timer);
    if (current?.dataset.title != null) {
      current.setAttribute("title", current.dataset.title);
      delete current.dataset.title;
    }
    current = null;
    tip = null;
  }

  function over(e: MouseEvent) {
    const target = (e.target as Element | null)?.closest<HTMLElement>("[title]");
    if (!target || target === current) return;
    hide();
    const text = target.getAttribute("title");
    if (!text) return;
    current = target;
    target.dataset.title = text;
    target.removeAttribute("title");
    timer = setTimeout(() => {
      const box = target.getBoundingClientRect();
      const below = box.top < 80; // no room above: show it under the element
      tip = { text, x: Math.min(Math.max(box.left + box.width / 2, 160), innerWidth - 160), y: below ? box.bottom + 8 : box.top - 8, below };
    }, DELAY);
  }

  function out(e: MouseEvent) {
    if (current && !current.contains(e.relatedTarget as Node | null)) hide();
  }
</script>

<svelte:document onmouseover={over} onmouseout={out} onmousedown={hide} onkeydown={hide} onwheel={hide} />

{#if tip}
  <div class="tip" class:below={tip.below} style:left="{tip.x}px" style:top="{tip.y}px" role="tooltip" id="quick-tooltip">{tip.text}</div>
{/if}

<style>
  .tip {
    position: fixed; z-index: 1000; max-width: 300px; transform: translate(-50%, -100%); pointer-events: none;
    padding: 6px 10px; border-radius: 8px; font-size: 12px; line-height: 1.4; white-space: pre-line;
    color: var(--popover-foreground); background: var(--popover);
    box-shadow: 0 0 0 1px oklch(0.5 0 0 / 0.18), 0 8px 24px rgb(0 0 0 / 0.3);
    animation: tip-in 0.15s ease-out both;
  }
  .tip.below { transform: translate(-50%, 0); }
  @keyframes tip-in { from { opacity: 0; } }
</style>
