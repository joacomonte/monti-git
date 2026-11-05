<!-- A dialog over a dimmed window: on <body>, closed by Escape or a click outside the panel. -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { fade } from "svelte/transition";

  type Props = { id: string; onclose: () => void; top?: boolean; label?: string; class?: string; children: Snippet };
  let { id, onclose, top = false, label, class: className = "", children }: Props = $props();

  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div use:portal class="backdrop enter-fade" class:top out:fade|global={{ duration: 150 }} id="{id}-backdrop" onclick={onclose} onmousedown={(e) => e.stopPropagation()}>
  <div class="panel enter-pop {className}" role="dialog" tabindex="-1" aria-label={label} {id} onclick={(e) => e.stopPropagation()}>
    {@render children()}
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 1000; display: grid; place-items: center; background: rgb(0 0 0 / 0.35); }
  .backdrop.top { place-items: start center; padding-top: 12vh; background: rgb(0 0 0 / 0.4); }
  .panel { background: var(--popover); color: var(--popover-foreground); box-shadow: 0 0 0 1px oklch(0.5 0 0 / 0.18), 0 16px 40px rgb(0 0 0 / 0.4); }
</style>
