<!-- Big row of the card popovers (ActionRow in BranchBar.swift): tinted icon tile (each action its own color), title, subtle description,
     and for choices that may not be obvious, an example that unfolds on hover. -->
<script lang="ts">
  import type { Component } from "svelte";
  import Example from "./Example.svelte";

  type Props = { title: string; detail: string; example?: string; icon: Component; id: string; color?: string; grey?: boolean; disabled?: boolean; onclick: () => void };
  let { title, detail, example, icon: Icon, id, color = "var(--primary)", grey = false, disabled = false, onclick }: Props = $props();
</script>

<button class="row" class:grey {id} {disabled} {onclick} style:--tint={color}>
  <span class="tile"><Icon size={15} /></span>
  <span class="flex min-w-0 flex-col gap-0.5 text-left">
    <span class="text-[13px] font-semibold">{title}</span>
    <span class="text-[11px] text-muted-foreground">{detail}</span>
    {#if example}<Example text={example} />{/if}
  </span>
</button>

<style>
  .row { display: flex; width: 100%; align-items: center; gap: 11px; padding: 8px; border-radius: 10px; }
  .row:hover:not(:disabled) { background: oklch(0.5 0 0 / 0.1); }
  .row:active:not(:disabled) { background: oklch(0.5 0 0 / 0.16); }
  .row:disabled { opacity: 0.5; }
  .tile {
    display: grid; place-items: center; width: 32px; height: 32px; flex-shrink: 0; border-radius: 8px;
    color: var(--tint); background: color-mix(in oklab, var(--tint) 16%, transparent);
  }
  .grey .tile { color: var(--muted-foreground); background: color-mix(in oklab, var(--muted-foreground) 15%, transparent); }
</style>
