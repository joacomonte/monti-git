<!-- Content of a card's ⌄ popover (BranchActions in BranchBar.swift). All card actions live here. -->
<script lang="ts">
  import { ArrowDownCircle, ArrowLeftRight, Check, GitBranchPlus, ChevronLeft, Copy, Eye, GitPullRequest, EyeOff, House, Layers, Pin, PinOff, Server, Waypoints } from "@lucide/svelte";
  import type { RepoModel } from "$lib/model.svelte";
  import ActionRow from "./ActionRow.svelte";

  type Props = {
    name: string;
    local: string; // name without "origin/"
    hash?: string | null; // tip commit, for "Copy commit hash"
    isBase: boolean;
    isCurrent: boolean;
    featureIsOld?: boolean | null; // set on feature cards: adds the Hide / Show row
    onSquash?: (() => void) | null; // feature cards with 2+ commits: adds "Squash commits…" (a prompt for an AI agent)
    onOpenPr?: (() => void) | null; // feature cards: adds "Open PR…" (a prompt for an AI agent)
    onNewBranch: () => void; // "New branch from here"
    model: RepoModel;
    dismiss: () => void;
  };
  let { name, local, hash = null, isBase, isCurrent, featureIsOld = null, onSquash = null, onOpenPr = null, onNewBranch, model, dismiss }: Props = $props();
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
    <!-- Main moves: big tiles in one row. -->
    <div class="flex gap-1">
      {#if !isCurrent}
        <button class="tile-button" style:--tint="#4f8cff" id="branch-actions-check-out" title="Switch to this branch and pull its latest"
          onclick={run(() => model.checkout(name))}><span class="tile"><ArrowLeftRight size={15} /></span>Check out</button>
      {/if}
      <button class="tile-button" style:--tint="#2fc4b2" id="branch-actions-pull" title="Fetch and fast-forward, without switching"
        disabled={model.isSyncing} onclick={run(() => model.pull(name))}><span class="tile"><ArrowDownCircle size={15} /></span>Pull</button>
      <button class="tile-button" style:--tint="#8b7bff" id="branch-actions-show-in-graph" title="Jump to its latest commit in the history"
        onclick={run(() => model.reveal(name))}><span class="tile"><Waypoints size={15} /></span>Show in graph</button>
    </div>
    <div class="sep"></div>
    <ActionRow title="New branch from here…" detail="Up to date with origin first, then checked out" icon={GitBranchPlus} color="#34c77b"
      id="branch-actions-new-branch" onclick={run(onNewBranch)}
      example="you're on main and start ticket MOA-900: it pulls main first, so you start from the latest code." />
    {#if onOpenPr || onSquash}
      {#if onOpenPr}
        <ActionRow title="Open PR…" detail="Pick where it goes, copy a prompt for your agent" icon={GitPullRequest} color="#4f8cff" id="branch-actions-open-pr"
          onclick={run(onOpenPr)} example="the feature is ready: choose main (or qa), paste the prompt in Claude Code and it opens the PR." />
      {/if}
      {#if onSquash}
        <ActionRow title="Squash commits…" detail="Pick the commits and the message, copy a prompt for your agent" icon={Layers} color="#ec5fa8"
          id="branch-actions-squash-commits" onclick={run(onSquash)}
          example={'"wip", "fix typo", "fix again" become one clean commit: "feat: export csv".'} />
      {/if}
    {/if}
    <div class="sep"></div>
    <div class="flex items-center gap-1.5 px-1.5 text-[12px] text-muted-foreground">
      <Copy size={12} class="shrink-0" />Copy
      <button class="chip max-w-[180px] truncate" id="branch-actions-copy-name" title="Copy {local}" onclick={run(() => navigator.clipboard.writeText(local))}>name</button>
      {#if hash}
        <button class="chip font-mono" id="branch-actions-copy-hash" title="Copy {hash}" onclick={run(() => navigator.clipboard.writeText(hash))}>{hash.slice(0, 7)}</button>
      {/if}
    </div>
    <div class="sep"></div>
    <!-- Dashboard bookkeeping: small, no descriptions (they're the tooltips). -->
    <div class="flex flex-wrap gap-0.5">
      {#if isBase}
        <button class="small" id="branch-actions-change-base" title="The one the dashboard and the graph compare against" onclick={() => (choosingBase = true)}>
          <House size={12} />Change base</button>
      {:else}
        <button class="small" id="branch-actions-pin" title={isPinned ? "Take it out of Pinned" : "Keep it in Pinned, however old it gets"}
          onclick={run(() => model.togglePin(name))}>{#if isPinned}<PinOff size={12} />Unpin{:else}<Pin size={12} />Pin{/if}</button>
        <button class="small" id="branch-actions-environment" title={isEnvironment ? "Move it back out of the Environments section" : "Show it in the dashboard's Environments section"}
          onclick={run(() => model.toggleEnvironment(local))}><Server size={12} />{isEnvironment ? "Not an environment" : "Environment"}</button>
        {#if featureIsOld != null}
          <button class="small" id="branch-actions-hide" title={featureIsOld ? "Bring it back with the branches you're working on" : "Move it to Hidden, at the bottom of the dashboard"}
            onclick={run(() => model.setFeatureOld(local, !featureIsOld))}>{#if featureIsOld}<Eye size={12} />Show{:else}<EyeOff size={12} />Hide{/if}</button>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .base:hover { background: oklch(0.5 0 0 / 0.1); }
  .sep { height: 1px; margin: 2px 6px; background: var(--border); }
  .tile-button { display: flex; flex: 1; flex-direction: column; align-items: center; gap: 5px; padding: 8px 4px; border-radius: 10px; font-size: 11.5px; font-weight: 600; }
  .tile-button:hover:not(:disabled) { background: oklch(0.5 0 0 / 0.1); }
  .tile-button:disabled { opacity: 0.5; }
  .tile { display: grid; place-items: center; width: 32px; height: 32px; border-radius: 8px; color: var(--tint); background: color-mix(in oklab, var(--tint) 16%, transparent); }
  .chip { height: 22px; padding: 0 8px; border-radius: 999px; font-size: 11.5px; color: var(--foreground); background: oklch(0.5 0 0 / 0.1); }
  .chip:hover { background: oklch(0.5 0 0 / 0.18); }
  .small { display: flex; align-items: center; gap: 5px; height: 26px; padding: 0 8px; border-radius: 7px; font-size: 12px; color: var(--muted-foreground); }
  .small:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.1); }
</style>
