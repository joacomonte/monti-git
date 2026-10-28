<!-- Strip under the top row when checking out with uncommitted changes (CheckoutPrompt in BranchBar.swift). -->
<script lang="ts">
  import { ArrowRight, BadgeCheck, CircleAlert, Inbox, X } from "@lucide/svelte";
  import type { Component } from "svelte";
  import type { WipMode } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";

  let { target, model }: { target: string; model: RepoModel } = $props();
  const n = $derived(model.pendingChangeCount);

  /** Enter = Stash (the default), Esc = cancel. */
  function keydown(e: KeyboardEvent) {
    if (e.key === "Escape") model.pendingCheckout = null;
    else if (e.key === "Enter" && !(e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLInputElement)) model.runCheckout(target, "shelve");
  }
</script>

<svelte:window onkeydown={keydown} />

{#snippet option(title: string, Icon: Component, mode: WipMode, primary: boolean)}
  <button class="option" class:primary id="checkout-prompt-{title.toLowerCase().replaceAll(' ', '-')}" onclick={() => model.runCheckout(target, mode)}>
    <Icon size={12} strokeWidth={2.5} />{title}
  </button>
{/snippet}

<div class="flex items-center gap-2 border-b bg-primary/8 px-3.5 py-[7px]">
  <CircleAlert size={14} class="shrink-0 text-primary" />
  <span class="shrink-0 text-[12px] font-semibold">{n} modified file{n === 1 ? "" : "s"}</span>
  <span class="truncate text-[12px] text-muted-foreground">checking out {target}</span>
  <span class="flex-1"></span>
  {@render option("Stash", Inbox, "shelve", true)}
  {@render option("WIP commit", BadgeCheck, "commit", false)}
  {@render option("Move to branch", ArrowRight, "carry", false)}
  <button class="grid size-6 place-items-center text-muted-foreground" title="Cancel" id="checkout-prompt-cancel" onclick={() => (model.pendingCheckout = null)}>
    <X size={11} strokeWidth={3} />
  </button>
</div>

<style>
  .option {
    display: flex; align-items: center; gap: 5px; height: 26px; padding: 0 10px; border-radius: 999px; flex-shrink: 0;
    font-size: 12px; font-weight: 600; background: oklch(0.5 0 0 / 0.12);
  }
  .option:hover { background: oklch(0.5 0 0 / 0.2); }
  .option.primary { color: white; background: var(--primary); }
  .option.primary:hover { background: color-mix(in oklab, var(--primary) 85%, black); }
</style>
