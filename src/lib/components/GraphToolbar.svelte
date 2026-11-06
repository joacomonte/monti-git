<!-- Thin strip over the graph with its toggles (blue when on, the full explanation on hover), and under it, while
     "Only this branch" is on, the way back to the whole graph. -->
<script lang="ts">
  import { ArrowLeft, FoldVertical, History, X } from "@lucide/svelte";
  import type { RepoModel } from "$lib/model.svelte";
  import { baseName } from "$lib/format";
  import EnvironmentsPicker from "./EnvironmentsPicker.svelte";
  import FilterPicker from "./FilterPicker.svelte";

  let { model }: { model: RepoModel } = $props();
  // The focus view and a file's history show one line: nothing to collapse or filter there.
  const narrowed = $derived(model.focus != null || model.historyPath != null);
</script>

<div class="toolbar flex h-[58px] shrink-0 items-center gap-1 overflow-x-auto border-b px-3" id="toolbar">
  <!-- Three labeled groups: what branches are compared against, how the graph shows them, which ones it shows. -->
  <div class="group"><span class="glabel">Environments</span><div class="controls"><EnvironmentsPicker {model} /></div></div>
  <div class="group">
    <span class="glabel">View</span>
    <div class="controls">
      <button class="tool" class:on={model.branchesOnly && !narrowed} aria-pressed={model.branchesOnly && !narrowed} id="view-collapsed" disabled={narrowed}
        title={narrowed ? "Not in the focus view or a file's history: they show one line" : `Collapsed: ${model.branchesOnly ? "on" : "off"}\nOne row per branch with its status and commit count, the rest folded. Off: every commit.`}
        onclick={() => model.setBranchesOnly(!model.branchesOnly)}><FoldVertical size={14} />Collapsed</button>
    </div>
  </div>
  <div class="group"><span class="glabel">Filters</span><div class="controls"><FilterPicker {model} collapsedOnly={!model.branchesOnly || narrowed} /></div></div>
  {#if model.historyPath}
    <button class="tool on" id="file-history-clear" onclick={() => model.setHistory(null)}
      title={`History of ${model.historyPath}: only the commits that touched it. Click to show every commit again.`}>
      <History size={14} />History: {baseName(model.historyPath)}<X size={11} />
    </button>
  {/if}
</div>
{#if model.focus}
  <div class="focus-banner" id="focus-banner">
    <button class="back" id="focus-close" title={model.focusFrom === "dashboard" ? "Back to the dashboard" : "Back to the whole graph, where you were"}
      onclick={() => model.leaveFocus()}>
      <ArrowLeft size={16} />{model.focusFrom === "dashboard" ? "Back to dashboard" : "Back to full graph"}
    </button>
    <!-- Opened from the dashboard: Back goes there, so the whole graph gets its own way out. -->
    {#if model.focusFrom === "dashboard"}
      <button class="switch" id="focus-to-graph" title="Leave the focus view for this repo's whole graph" onclick={() => model.setFocus(null)}>
        <X size={15} />Exit focus view
      </button>
    {/if}
    <span class="mode">Focus view</span>
    {#if model.history.fastForwarded}
      <span class="mode" title="The base was moved up to this branch (fast-forward): there's no merge commit, the base simply went on from its last commit.">
        · Fast-forwarded into {model.history.base}: no merge, it went on from here</span>
    {/if}
  </div>
{/if}

<style>
  /* Each group: its title on top, its controls under it. */
  .group { display: flex; flex-shrink: 0; flex-direction: column; gap: 2px; padding-right: 16px; margin-right: 14px; border-right: 1px solid var(--border); }
  .controls { display: flex; align-items: center; gap: 2px; }
  .group:last-of-type { border-right: none; }
  .glabel { padding-left: 8px; font-size: 9.5px; font-weight: 700; letter-spacing: 0.04em; text-transform: uppercase; color: var(--tertiary); }
  /* Narrow: the labels go (each control explains itself on hover) and the groups tighten; the filters fold into their menu. */
  @container toolbar (max-width: 620px) {
    .glabel { display: none; }
    .group { padding-right: 8px; margin-right: 6px; }
  }
  .toolbar { container: toolbar / inline-size; } /* FilterPicker shows its toggles inline when this has room */
  .focus-banner { display: flex; flex-shrink: 0; align-items: center; gap: 14px; height: 48px; padding: 0 10px; border-bottom: 1px solid var(--border);
    background: color-mix(in oklab, var(--primary) 8%, transparent); }
  .focus-banner .back { display: flex; align-items: center; gap: 7px; height: 34px; padding: 0 14px; border-radius: 9px; font-size: 14px; font-weight: 600; color: var(--primary); background: color-mix(in oklab, var(--primary) 12%, transparent); }
  .focus-banner .back:hover { background: color-mix(in oklab, var(--primary) 22%, transparent); }
  .focus-banner .switch { display: flex; align-items: center; gap: 7px; height: 34px; padding: 0 12px; margin-left: -6px; border-radius: 9px; font-size: 13.5px; font-weight: 600; color: var(--muted-foreground); }
  .focus-banner .switch:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.1); }
  .focus-banner .mode { font-size: 13px; color: var(--muted-foreground); }
</style>
