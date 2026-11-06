<!-- A branch's menu in the graph (click or right-click its badge): check out, pull, new branch, copy, change base. -->
<script lang="ts">
  import { ArrowDownCircle, ArrowLeftRight, Check, Crosshair, GitBranchPlus, ChevronLeft, Copy, House, Sparkles } from "@lucide/svelte";
  import type { RepoModel } from "$lib/model.svelte";
  import ActionRow from "./ActionRow.svelte";

  type Props = {
    name: string;
    local: string; // name without "origin/"
    hash?: string | null; // tip commit, for "Copy commit hash"
    isBase: boolean;
    isCurrent: boolean;
    onNewBranch: () => void; // "New branch from here"
    onFocus: () => void; // "Focus view"
    model: RepoModel;
    dismiss: () => void;
  };
  let { name, local, hash = null, isBase, isCurrent, onNewBranch, onFocus, model, dismiss }: Props = $props();
  let choosingBase = $state(false);
  let copying = $state(false); // "Copy" row unfolded: name, hash, or all of it for an AI agent

  // Action first: dismissing can clear the props this reads (the graph's menu derives `name` from its open state).
  const run = (action: () => void) => () => { action(); dismiss(); };
</script>

<div class="flex w-[340px] flex-col gap-1.5">
  {#if choosingBase}
    <button class="flex items-center gap-1 px-1.5 pb-0.5 text-[12px] font-semibold text-muted-foreground" id="branch-actions-back"
      onclick={() => (choosingBase = false)}>
      <ChevronLeft size={12} strokeWidth={2.5} />Base branch
    </button>
    <div class="flex max-h-[260px] flex-col gap-0.5 overflow-auto">
      {#each model.history.local as branch (branch)}
        <button class="base flex h-[30px] items-center justify-between gap-2 rounded-[10px] px-2.5 text-[13px]" id="branch-actions-base-{branch}"
          onclick={run(() => model.setBaseBranch(branch))}>
          <span class="truncate">{branch}</span>
          {#if name === branch || name.endsWith("/" + branch)}<Check size={11} strokeWidth={3} class="shrink-0 text-primary" />{/if}
        </button>
      {/each}
    </div>
  {:else}
    <span class="truncate px-1.5 pb-0.5 text-[12px] font-semibold text-muted-foreground">{local}</span>
    <!-- Grouped: look at it, work with it, copy it. -->
    <span class="group">View</span>
    <ActionRow title="Focus view" detail={isBase ? "Just this branch: its own line of commits" : "Just this branch: its commits and where it merged"} icon={Crosshair} color="#a78bfa"
      id="branch-actions-focus" onclick={run(onFocus)} />
    <div class="sep"></div>
    <span class="group">Branch</span>
    {#if !isCurrent}
      <ActionRow title="Check out" detail="Switch to this branch and pull its latest" icon={ArrowLeftRight} color="#4f8cff" id="branch-actions-check-out"
        onclick={run(() => model.checkout(name))} />
    {/if}
    <ActionRow title="Pull" detail="Fetch and fast-forward, without switching" icon={ArrowDownCircle} color="#2fc4b2" id="branch-actions-pull"
      disabled={model.isSyncing} onclick={run(() => model.pull(name))} />
    <ActionRow title="New branch from here…" detail="Up to date with origin first, then checked out" icon={GitBranchPlus} color="#34c77b"
      id="branch-actions-new-branch" onclick={run(onNewBranch)}
      example="you're on main and start ticket MOA-900: it pulls main first, so you start from the latest code." />
    <div class="sep"></div>
    <ActionRow title="Copy" detail={copying ? "Pick what to copy" : "Name, commit hash, or all of it for your AI agent"} icon={Copy} grey id="branch-actions-copy"
      onclick={() => (copying = !copying)} />
    {#if copying}
      <div class="flex flex-col gap-0.5 pl-6">
        <ActionRow title="Name" detail={local} icon={Copy} grey id="branch-actions-copy-name" onclick={run(() => navigator.clipboard.writeText(local))} />
        {#if hash}
          <ActionRow title="Commit hash" detail={hash.slice(0, 7)} icon={Copy} grey id="branch-actions-copy-hash" onclick={run(() => navigator.clipboard.writeText(hash))} />
        {/if}
        <!-- Repo, branch and hash: enough for an agent with git to look at the rest itself. -->
        <ActionRow title="All, for AI" detail="Repo, branch and commit hash, to paste in your agent" icon={Sparkles} color="#a78bfa" id="branch-actions-copy-metadata"
          onclick={run(() => navigator.clipboard.writeText([`Repo: ${model.repo}`, `Branch: ${local}`, hash ? `Commit: ${hash}` : null].filter((line) => line != null).join("\n")))} />
      </div>
    {/if}
    {#if isBase}
      <div class="sep"></div>
      <button class="small" id="branch-actions-change-base" title="The branch the graph compares every other one against" onclick={() => (choosingBase = true)}>
        <House size={12} />Change base</button>
    {/if}
  {/if}
</div>

<style>
  .base:hover { background: oklch(0.5 0 0 / 0.1); }
  .sep { height: 1px; margin: 2px 6px; background: var(--border); }
  .group { padding: 2px 8px 0; font-size: 10.5px; font-weight: 600; letter-spacing: 0.04em; text-transform: uppercase; color: var(--tertiary); }
  .small { display: flex; align-self: flex-start; align-items: center; gap: 5px; height: 26px; padding: 0 8px; border-radius: 7px; font-size: 12px; color: var(--muted-foreground); }
  .small:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.1); }
</style>
