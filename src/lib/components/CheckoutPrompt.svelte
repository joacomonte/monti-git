<!-- Dialog when checking out with uncommitted changes: what to do with them, one big row per choice (like the card menus). -->
<script lang="ts">
  import { ArrowRight, BadgeCheck, CircleAlert, Inbox } from "@lucide/svelte";
  import type { Component } from "svelte";
  import type { RepoModel } from "$lib/model.svelte";
  import type { WipMode } from "$lib/api";
  import Example from "./Example.svelte";
  import { fade } from "svelte/transition";

  let { target, model }: { target: string; model: RepoModel } = $props();
  const n = $derived(model.pendingChangeCount);
  const from = $derived(model.dashboard?.current ?? "this branch");
  const cancel = () => (model.pendingCheckout = null);

  /** Enter = Stash (the default), Esc = cancel. */
  function keydown(e: KeyboardEvent) {
    if (e.key === "Escape") cancel();
    else if (e.key === "Enter" && !(e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLInputElement)) model.runCheckout(target, "shelve");
  }
</script>

<svelte:window onkeydown={keydown} />

<!-- Spacious choice card: big tinted icon, title, the full explanation (never cut). -->
{#snippet choice(id: string, mode: WipMode, Icon: Component, color: string, title: string, detail: string, example: string, recommended = false)}
  <button class="choice" {id} style:--tint={color} onclick={() => model.runCheckout(target, mode)}>
    <span class="tile"><Icon size={20} /></span>
    <span class="flex min-w-0 flex-1 flex-col gap-1 text-left">
      <span class="flex items-center gap-2 text-[14px] font-semibold">{title}{#if recommended}<span class="badge">Recommended</span>{/if}</span>
      <span class="text-[12.5px] leading-relaxed text-muted-foreground">{detail}</span>
      <Example text={example} />
    </span>
  </button>
{/snippet}

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop enter-fade" out:fade|global={{ duration: 150 }} id="checkout-prompt" onclick={cancel}>
  <div class="panel enter-pop" role="dialog" tabindex="-1" aria-labelledby="checkout-prompt-title" onclick={(e) => e.stopPropagation()}>
    <div class="flex flex-col gap-3 px-1 pb-4">
      <div class="flex items-center gap-2.5">
        <CircleAlert size={18} class="shrink-0 text-primary" />
        <span class="text-[14px] font-semibold" id="checkout-prompt-title">{n} file{n === 1 ? "" : "s"} not committed yet</span>
      </div>
      <!-- Where you are → where you're going. -->
      <div class="route">
        <span class="ref" title={from}>{from}</span>
        <ArrowRight size={13} class="shrink-0 text-tertiary" />
        <span class="ref to" title={target}>{target}</span>
      </div>
      <span class="text-[12px] text-muted-foreground">What should happen to your changes?</span>
    </div>
    <div class="flex flex-col gap-2.5">
      {@render choice("checkout-prompt-stash", "shelve", Inbox, "#4f8cff", "Stash them",
        "Put aside on the branch you're leaving. They come back by themselves when you return to it.",
        "you're halfway through a fix and must review a teammate's branch: switch, review, come back, and your changes are there again.", true)}
      {@render choice("checkout-prompt-wip-commit", "commit", BadgeCheck, "#34c77b", "Save as a WIP commit",
        "A temporary commit on the branch you're leaving, undone automatically when you return.",
        "same, but you'll be away for days: a commit is harder to lose than a stash.")}
      {@render choice("checkout-prompt-move", "carry", ArrowRight, "#f5a524", "Take them with me",
        "They come with you to the new branch. Handy if you started working on the wrong one.",
        "you started coding on main by mistake: take the changes to your feature branch.")}
    </div>
    <div class="mt-4 flex items-center justify-between px-1">
      <span class="text-[11px] text-tertiary"><kbd>Enter</kbd> stash · <kbd>Esc</kbd> cancel</span>
      <button class="cancel" id="checkout-prompt-cancel" onclick={cancel}>Cancel</button>
    </div>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 1000; display: grid; place-items: center; background: rgb(0 0 0 / 0.35); }
  .panel {
    display: flex; flex-direction: column; width: 520px; padding: 22px 18px 14px; border-radius: 18px;
    background: var(--popover); color: var(--popover-foreground); box-shadow: 0 0 0 1px oklch(0.5 0 0 / 0.18), 0 16px 40px rgb(0 0 0 / 0.4);
  }
  .choice {
    display: flex; align-items: center; gap: 16px; padding: 16px 18px; border-radius: 14px;
    box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.16); transition: box-shadow 0.15s, background 0.15s;
  }
  .choice:hover { background: color-mix(in oklab, var(--tint) 6%, transparent); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--tint) 55%, transparent); }
  .tile {
    display: grid; place-items: center; width: 48px; height: 48px; flex-shrink: 0; border-radius: 12px;
    color: var(--tint); background: color-mix(in oklab, var(--tint) 15%, transparent);
  }
  .badge { padding: 1px 7px; border-radius: 999px; font-size: 10.5px; font-weight: 600; color: var(--tint); background: color-mix(in oklab, var(--tint) 15%, transparent); }
  .route { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .ref {
    min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding: 3px 9px; border-radius: 7px;
    font: 500 11.5px var(--font-mono, ui-monospace, monospace); color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.1);
  }
  .ref.to { color: var(--primary); background: color-mix(in oklab, var(--primary) 14%, transparent); }
  kbd { padding: 0 4px; border-radius: 4px; font: inherit; background: oklch(0.5 0 0 / 0.12); }
  .cancel { height: 28px; padding: 0 12px; border-radius: 8px; font-size: 12px; color: var(--muted-foreground); }
  .cancel:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.08); }
</style>
