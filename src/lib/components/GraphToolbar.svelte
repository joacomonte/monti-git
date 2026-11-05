<!-- Thin strip over the graph with its toggles (blue when on, the full explanation on hover), and under it, while
     "Only this branch" is on, the way back to the whole graph. -->
<script lang="ts">
  import { ArrowLeft, GitBranch, History, UserRound, X } from "@lucide/svelte";
  import type { RepoModel } from "$lib/model.svelte";
  import { baseName } from "$lib/format";
  import EnvironmentsPicker from "./EnvironmentsPicker.svelte";
  import FilterPicker from "./FilterPicker.svelte";

  let { model }: { model: RepoModel } = $props();
  // The focus view and a file's history show one line: nothing to collapse or filter there.
  const narrowed = $derived(model.focus != null || model.historyPath != null);
</script>

<div class="flex h-[46px] shrink-0 items-center gap-1 overflow-x-auto border-b px-3" id="toolbar">
  <EnvironmentsPicker {model} />
  <span class="mx-1 h-4 w-px shrink-0 bg-border"></span>
  <button class="tool" class:on={model.onlyMine} aria-pressed={model.onlyMine} id="only-mine-toggle"
    onclick={() => model.setOnlyMine(!model.onlyMine)}
    title={`My branches only: ${model.onlyMine ? "on" : "off"}\nShows only branches whose last commit is yours, plus main and the environments (qa, develop…). Ej: hides your teammates' branches so the graph stays clean.`}>
    <UserRound size={14} />My branches
  </button>
  <button class="tool" class:on={model.branchesOnly} aria-pressed={model.branchesOnly} id="branches-only-toggle" disabled={narrowed}
    onclick={() => model.setBranchesOnly(!model.branchesOnly)}
    title={`Commits collapsed: ${model.branchesOnly ? "on" : "off"}\nOne row per branch with its commit count, the rest collapsed: a quick look at every branch. Click "N commits" to open one.`}>
    <GitBranch size={14} />Commits collapsed
  </button>
  <FilterPicker {model} disabled={!model.branchesOnly || narrowed} />
  {#if model.historyPath}
    <button class="tool on" id="file-history-clear" onclick={() => model.setHistory(null)}
      title={`History of ${model.historyPath}: only the commits that touched it. Click to show every commit again.`}>
      <History size={14} />History: {baseName(model.historyPath)}<X size={11} />
    </button>
  {/if}
</div>
{#if model.focus}
  <div class="focus-banner" id="focus-banner">
    <button class="back" id="focus-close" title="Back to the whole graph, where you were" onclick={() => model.setFocus(null)}>
      <ArrowLeft size={16} />Back to full graph
    </button>
    <span class="mode">Focus view</span>
    {#if model.history.fastForwarded}
      <span class="mode" title="The base was moved up to this branch (fast-forward): there's no merge commit, the base simply went on from its last commit.">
        · Fast-forwarded into {model.history.base}: no merge, it went on from here</span>
    {/if}
  </div>
{/if}

<style>
  .tool { display: flex; flex-shrink: 0; align-items: center; gap: 6px; height: 30px; padding: 0 10px; border-radius: 8px; font-size: 12.5px; font-weight: 600; color: var(--muted-foreground); }
  .tool:hover:not(:disabled) { color: var(--foreground); background: oklch(0.5 0 0 / 0.1); }
  .tool:disabled { opacity: 0.4; }
  .tool.on, .tool.on:hover:not(:disabled) { color: var(--primary); }
  .focus-banner { display: flex; flex-shrink: 0; align-items: center; gap: 14px; height: 48px; padding: 0 10px; border-bottom: 1px solid var(--border);
    background: color-mix(in oklab, var(--primary) 8%, transparent); }
  .focus-banner .back { display: flex; align-items: center; gap: 7px; height: 34px; padding: 0 14px; border-radius: 9px; font-size: 14px; font-weight: 600; color: var(--primary); background: color-mix(in oklab, var(--primary) 12%, transparent); }
  .focus-banner .back:hover { background: color-mix(in oklab, var(--primary) 22%, transparent); }
  .focus-banner .mode { font-size: 13px; color: var(--muted-foreground); }
</style>
