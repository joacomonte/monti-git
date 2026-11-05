<!-- A menu at the pointer (right click, or a click on a pill): on <body> so a scrolling pane doesn't clip it, kept inside
     the window, and closed by a click outside, Escape, scrolling or leaving the window. That outside click only closes it:
     whatever is under the pointer doesn't react to it. -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { fade } from "svelte/transition";

  type Props = { x: number; y: number; id: string; onclose: () => void; class?: string; children: Snippet };
  let { x, y, id, onclose, class: className = "", children }: Props = $props();
  let width = $state(0);
  let height = $state(0);
  const left = $derived(Math.max(8, Math.min(x, innerWidth - width - 8)));
  const top = $derived(Math.max(8, Math.min(y, innerHeight - height - 8)));

  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  let swallowing = false;
  function outside(e: MouseEvent) {
    if ((e.target as Element).closest?.(`#${CSS.escape(id)}`)) return;
    swallowing = true;
    onclose();
  }
  function swallow(e: MouseEvent) {
    if (!swallowing) return;
    swallowing = false;
    e.stopPropagation();
    e.preventDefault();
  }
</script>

<svelte:window onmousedowncapture={outside} onclickcapture={swallow} onkeydown={(e) => e.key === "Escape" && onclose()} onwheel={onclose} onblur={onclose} />
<div use:portal class="context-menu enter-pop {className}" out:fade={{ duration: 120 }} role="menu" tabindex="-1" {id}
  bind:clientWidth={width} bind:clientHeight={height} style:left="{left}px" style:top="{top}px">
  {@render children()}
</div>

<style>
  .context-menu {
    position: fixed; z-index: 1000; max-height: calc(100vh - 16px); overflow: auto; padding: 5px; border-radius: 10px;
    background: var(--popover); color: var(--popover-foreground); box-shadow: 0 0 0 1px oklch(0.5 0 0 / 0.18), 0 8px 24px rgb(0 0 0 / 0.3);
  }
  /* Plain rows: icon, text, an optional dimmed hint on the right. */
  .context-menu :global(.menu-item) {
    display: flex; width: 100%; align-items: center; gap: 8px; height: 30px; padding: 0 10px; border-radius: 7px;
    font-size: 12.5px; text-align: left; white-space: nowrap;
  }
  .context-menu :global(.menu-item:hover) { background: oklch(0.5 0 0 / 0.12); }
  .context-menu :global(.menu-hint) { min-width: 0; margin-left: auto; padding-left: 16px; overflow: hidden; text-overflow: ellipsis; font-size: 11px; color: var(--tertiary); }
  .context-menu :global(.menu-separator) { height: 1px; margin: 4px 0; background: var(--border); }
</style>
