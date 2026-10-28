<!-- Content of a card's ⌄ popover (BranchActions in BranchBar.swift). All card actions live here. -->
<script lang="ts">
  import { ArrowDownCircle, ArrowLeftRight, Check, ChevronLeft, Copy, Eye, EyeOff, House, Layers, Pin, PinOff, Server, Waypoints } from "@lucide/svelte";
  import type { RepoModel } from "$lib/model.svelte";
  import ActionRow from "./ActionRow.svelte";

  type Props = {
    name: string;
    local: string; // name without "origin/"
    isBase: boolean;
    isCurrent: boolean;
    featureIsOld?: boolean | null; // set on feature cards: adds the Hide / Show row
    onSquash?: (() => void) | null; // feature cards with 2+ commits: adds the Squash row
    model: RepoModel;
    dismiss: () => void;
  };
  let { name, local, isBase, isCurrent, featureIsOld = null, onSquash = null, model, dismiss }: Props = $props();
  let choosingBase = $state(false);
  const isPinned = $derived(model.pinnedBranches.includes(name));
  const isEnvironment = $derived(model.isEnvironment(local));

  const run = (action: () => void) => () => { dismiss(); action(); };
</script>

<div class="flex w-[340px] flex-col gap-1.5">
  {#if choosingBase}
    <button class="flex items-center gap-1 px-1.5 pb-0.5 text-[12px] font-semibold text-muted-foreground" id="branch-actions-back"
      onclick={() => (choosingBase = false)}>
      <ChevronLeft size={12} strokeWidth={2.5} />Base branch
    </button>
    <div class="flex max-h-[260px] flex-col gap-0.5 overflow-auto">
      {#each model.dashboard?.local ?? [] as branch (branch)}
        <button class="base flex h-[30px] items-center justify-between gap-2 rounded-[10px] px-2.5 text-[13px]" id="branch-actions-base-{branch}"
          onclick={run(() => model.setBaseBranch(branch))}>
          <span class="truncate">{branch}</span>
          {#if name === branch || name.endsWith("/" + branch)}<Check size={11} strokeWidth={3} class="shrink-0 text-primary" />{/if}
        </button>
      {/each}
    </div>
  {:else}
    <span class="truncate px-1.5 pb-0.5 text-[12px] font-semibold text-muted-foreground">{local}</span>
    {#if !isCurrent}
      <ActionRow title="Check out" detail="Switch to this branch and pull its latest" icon={ArrowLeftRight} color="#4f8cff" id="branch-actions-check-out"
        onclick={run(() => model.checkout(name))} />
    {/if}
    <ActionRow title="Pull" detail="Fetch and fast-forward, without switching" icon={ArrowDownCircle} color="#2fc4b2" id="branch-actions-pull"
      disabled={model.isSyncing} onclick={run(() => model.pull(name))} />
    <ActionRow title="Show in graph" detail="Jump to its latest commit in the history" icon={Waypoints} color="#8b7bff" id="branch-actions-show-in-graph"
      onclick={run(() => model.reveal(name))} />
    <ActionRow title="Copy name" detail={local} icon={Copy} color="#5ec8f2" id="branch-actions-copy-name"
      onclick={run(() => navigator.clipboard.writeText(local))} />
    {#if isBase}
      <ActionRow title="Change base branch" detail="The one the dashboard and the graph compare against" icon={House} color="#ff8a4c" id="branch-actions-change-base"
        onclick={() => (choosingBase = true)} />
    {:else}
      <ActionRow title={isPinned ? "Unpin" : "Pin"} detail={isPinned ? "Take it out of Pinned" : "Keep it in Pinned, however old it gets"}
        icon={isPinned ? PinOff : Pin} color="#f5a524" id="branch-actions-pin" onclick={run(() => model.togglePin(name))} />
      <ActionRow title={isEnvironment ? "Not an environment" : "Mark as environment"}
        detail={isEnvironment ? "Move it back out of the Environments section" : "Show it in the dashboard's Environments section"}
        icon={Server} color="#34c77b" id="branch-actions-environment" onclick={run(() => model.toggleEnvironment(local))} />
      {#if onSquash}
        <ActionRow title="Squash commits" detail={'Combine them into one clean commit. Ej: 5 "fix" commits → 1.'} icon={Layers} color="#ec5fa8"
          id="branch-actions-squash-commits" onclick={run(onSquash)} />
      {/if}
      {#if featureIsOld != null}
        <ActionRow title={featureIsOld ? "Show in Recent activity" : "Hide"}
          detail={featureIsOld ? "Bring it back with the branches you're working on" : "Move it to Hidden, at the bottom of the dashboard"}
          icon={featureIsOld ? Eye : EyeOff} color="#94a3b8" id="branch-actions-hide" onclick={run(() => model.setFeatureOld(local, !featureIsOld))} />
      {/if}
    {/if}
  {/if}
</div>

<style>
  .base:hover { background: oklch(0.5 0 0 / 0.1); }
</style>
