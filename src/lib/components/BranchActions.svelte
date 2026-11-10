<!-- A branch's menu in the graph (click or right-click its badge): check out, pull, new branch, copy, change base. -->
<script lang="ts">
  import { ArrowDownCircle, ArrowUpCircle, ArrowLeftRight, Check, Crosshair, GitBranchPlus, ChevronLeft, Copy, House, ClipboardList, GitBranch, Hash, ListOrdered } from "@lucide/svelte";
  import type { RepoModel } from "$lib/model.svelte";
  import { localName } from "$lib/refs";
  import ActionRow from "./ActionRow.svelte";

  type Props = {
    name: string;
    local: string; // name without "origin/"
    hash?: string | null; // tip commit, for "Copy commit hash"
    isBase: boolean;
    isCurrent: boolean;
    canPush: boolean; // a local branch the server doesn't have at this commit
    onNewBranch: () => void; // "New branch from here"
    onFocus: () => void; // "Focus view"
    onEdit: () => void; // "Edit commits…"
    model: RepoModel;
    dismiss: () => void;
  };
  let { name, local, hash = null, isBase, isCurrent, canPush, onNewBranch, onFocus, onEdit, model, dismiss }: Props = $props();
  const onServer = $derived(model.history.onServer.includes(local)); // Pull has something to bring
  // A local branch of its own (not the base): its commits since the base can be edited.
  const canEdit = $derived(!isBase && !!model.history.base && model.history.local.includes(local));
  let choosingBase = $state(false);
  let copying = $state(false); // "Copy" row under the pointer (or focused): it turns into its three buttons

  // Action first: dismissing can clear the props this reads (the graph's menu derives `name` from its open state).
  const run = (action: () => void) => () => { action(); dismiss(); };

  /** What an AI agent needs to look at this branch: where, which, against what, and its own commits as a git range. */
  function forAgent() {
    const base = model.history.base ? localName(model.history.base) : null;
    return [`Repo: ${model.repo}`, `Branch: ${local}`, ...(base && base !== local ? [`Base: ${base}`, `Its own commits: ${base}..${local}`] : [])].join("\n");
  }
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
          onclick={run(() => model.makeBase(branch))}>
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
    {#if canPush}
      <ActionRow title="Push" detail="Send its commits to the server, without switching" icon={ArrowUpCircle} color="#f5a524" id="branch-actions-push"
        disabled={model.isSyncing} onclick={run(() => model.push(local))} />
    {/if}
    {#if onServer}
      <ActionRow title="Pull" detail="Fetch and fast-forward, without switching" icon={ArrowDownCircle} color="#2fc4b2" id="branch-actions-pull"
        disabled={model.isSyncing} onclick={run(() => model.pull(name))} />
    {/if}
    {#if canEdit}
      <ActionRow title="Edit commits…" detail="Reorder, squash, reword or drop its commits since the base" icon={ListOrdered} color="#8b7bff"
        id="branch-actions-edit-commits" disabled={model.isSyncing} onclick={run(onEdit)} />
    {/if}
    <ActionRow title="New branch from here…" detail="Up to date with origin first, then checked out" icon={GitBranchPlus} color="#34c77b"
      id="branch-actions-new-branch" onclick={run(onNewBranch)}
      example="you're on main and start ticket MOA-900: it pulls main first, so you start from the latest code." />
    <div class="sep"></div>
    <!-- Hover (or focus) turns the row into its three buttons: one click copies. -->
    <div class="copy-slot" role="group" aria-label="Copy" onmouseenter={() => (copying = true)} onmouseleave={() => (copying = false)}
      onfocusin={() => (copying = true)} onfocusout={(e) => { if (!e.currentTarget.contains(e.relatedTarget as Node)) copying = false; }}>
      {#if copying}
        <div class="copy-buttons">
          <button style:--i="0" id="branch-actions-copy-name" title={local} onclick={run(() => navigator.clipboard.writeText(local))}><GitBranch size={14} />Branch name</button>
          {#if hash}
            <button style:--i="1" id="branch-actions-copy-hash" title={hash} onclick={run(() => navigator.clipboard.writeText(hash))}><Hash size={14} />Commit hash</button>
          {/if}
          <!-- Repo, branch, its base and the range of its own commits: an agent with git looks at the rest itself. -->
          <button style:--i={hash ? 2 : 1} class="ai" id="branch-actions-copy-metadata" title="Repo, branch and its base, with the range of its own commits: paste it in your AI agent"
            onclick={run(() => navigator.clipboard.writeText(forAgent()))}><ClipboardList size={14} />Everything</button>
        </div>
      {:else}
        <ActionRow title="Copy" detail="Name, commit hash, or all of it for your AI agent" icon={Copy} grey id="branch-actions-copy" onclick={() => (copying = true)} />
      {/if}
    </div>
    {#if isBase}
      <div class="sep"></div>
      <button class="small" id="branch-actions-change-base" title="The branch the graph compares every other one against" onclick={() => (choosingBase = true)}>
        <House size={12} />Change base</button>
    {/if}
  {/if}
</div>

<style>
  .base:hover { background: oklch(0.5 0 0 / 0.1); }
  /* Same height either way, so the menu doesn't jump when the row turns into buttons. */
  .copy-slot { display: flex; min-height: 64px; flex-direction: column; justify-content: center; }
  .copy-buttons { display: flex; gap: 6px; padding: 4px; }
  .copy-buttons button { display: flex; flex: 1; flex-direction: column; align-items: center; justify-content: center; gap: 4px; height: 56px; border-radius: 12px;
    font-size: 12.5px; font-weight: 600; background: oklch(0.5 0 0 / 0.1);
    animation: pop 0.22s cubic-bezier(0.2, 0.9, 0.3, 1.2) both; animation-delay: calc(var(--i) * 40ms); transition: background 0.15s, transform 0.15s; }
  .copy-buttons button:active { transform: scale(0.96); }
  /* The row splits into its buttons: each one rises and grows in, left to right. */
  @keyframes pop { from { opacity: 0; transform: translateY(4px) scale(0.9); } }
  @media (prefers-reduced-motion: reduce) { .copy-buttons button { animation: none; } }
  .copy-buttons button:hover { background: oklch(0.5 0 0 / 0.18); }
  .copy-buttons .ai { color: #a78bfa; background: color-mix(in oklab, #a78bfa 14%, transparent); }
  .copy-buttons .ai:hover { background: color-mix(in oklab, #a78bfa 24%, transparent); }
  .sep { height: 1px; margin: 2px 6px; background: var(--border); }
  .group { padding: 2px 8px 0; font-size: 10.5px; font-weight: 600; letter-spacing: 0.04em; text-transform: uppercase; color: var(--tertiary); }
  .small { display: flex; align-self: flex-start; align-items: center; gap: 5px; height: 26px; padding: 0 8px; border-radius: 7px; font-size: 12px; color: var(--muted-foreground); }
  .small:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.1); }
</style>
