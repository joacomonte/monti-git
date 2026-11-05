<!-- One choice out of a few, side by side: the app's single segmented control (tree layout, search modes, Logs kinds…). -->
<script lang="ts" generics="T extends string">
  import type { Component } from "svelte";

  type Option = { value: T; label?: string; icon?: Component; title?: string; disabled?: boolean };
  type Props = { options: Option[]; value: T; onchange: (value: T) => void; id: string; stretch?: boolean };
  let { options, value, onchange, id, stretch = false }: Props = $props();
</script>

<div class="seg" class:stretch role="radiogroup" {id}>
  {#each options as option (option.value)}
    <button role="radio" aria-checked={value === option.value} class:on={value === option.value} id="{id}-{option.value}"
      title={option.title} disabled={option.disabled} onclick={() => onchange(option.value)}>
      {#if option.icon}<option.icon size={13} />{/if}{#if option.label}<span class="label">{option.label}</span>{/if}
    </button>
  {/each}
</div>

<style>
  .seg { display: flex; flex-shrink: 0; gap: 1px; padding: 2px; border-radius: 8px; background: oklch(0.5 0 0 / 0.08); }
  .seg.stretch > button { flex: 1; }
  button { display: flex; align-items: center; justify-content: center; gap: 5px; height: 24px; padding: 0 9px; border-radius: 6px;
    font-size: 11.5px; font-weight: 600; white-space: nowrap; color: var(--muted-foreground); transition: color 0.15s, background 0.15s; }
  button:hover:not(:disabled):not(.on) { color: var(--foreground); }
  button.on { color: var(--foreground); background: var(--popover); box-shadow: 0 1px 3px rgb(0 0 0 / 0.25); }
  button:disabled { opacity: 0.4; }
</style>
