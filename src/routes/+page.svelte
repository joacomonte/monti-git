<script lang="ts">
  import { Crosshair, GitBranch, History, LayoutGrid, Search, X, TriangleAlert, UserRound, Waypoints } from "@lucide/svelte";
  import CommitSearch from "$lib/components/CommitSearch.svelte";
  import CheckoutPrompt from "$lib/components/CheckoutPrompt.svelte";
  import CommitList from "$lib/components/CommitList.svelte";
  import DiffPane from "$lib/components/DiffPane.svelte";
  import FileTree from "$lib/components/FileTree.svelte";
  import Loader from "$lib/components/Loader.svelte";
  import Onboarding from "$lib/components/Onboarding.svelte";
  import QuickTooltip from "$lib/components/QuickTooltip.svelte";
  import PaneDivider from "$lib/components/PaneDivider.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import UpdateGate from "$lib/components/UpdateGate.svelte";
  import WhatsNew from "$lib/components/WhatsNew.svelte";
  import ActionToast from "$lib/components/ActionToast.svelte";
  import LogsPanel from "$lib/components/LogsPanel.svelte";
  import { prefs, RepoModel } from "$lib/model.svelte";
  import { FILTERS } from "$lib/branchFilters";
  import { featureCards } from "$lib/api";
  import { fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";

  const model = new RepoModel();
  let treeWidth = $state(prefs.get("treeWidth", 360)); // remembered pane sizes
  let diffHeight = $state(prefs.get("diffHeight", 380));
  let space = $state({ width: 1200, height: 800 });
  let sliding = $state(false); // Dashboard ↔ Graph slide in progress
  $effect(() => prefs.set("treeWidth", treeWidth));
  $effect(() => prefs.set("diffHeight", diffHeight));
  let searching = $state(false); // ⌘K commit search open
  // Keeps every branch up to date with origin: on open, every 5 min, on focus and on any activity (below).
  $effect(() => {
    if (!model.repo) return;
    model.autoSync();
    const timer = setInterval(() => model.autoSync(), 5 * 60_000);
    return () => clearInterval(timer);
  });
</script>

<!-- Back from the editor/terminal: pick up new commits, checkouts and uncommitted changes. -->
<!-- Any activity also syncs with origin (at most once a minute, see autoSync). -->
<!-- oncontextmenu: no browser menu (Reload / Inspect Element) on right click; text fields and selected text keep it for copy/paste. -->
<svelte:window onfocus={() => { model.reload(); model.autoSync(); }} onpointerdown={() => model.autoSync()} onwheel={() => model.autoSync()}
  oncontextmenu={(e) => {
    const field = (e.target as Element).closest("input, textarea, [contenteditable]");
    if (!field && !getSelection()?.toString()) e.preventDefault();
  }}
  onkeydown={(e) => {
    model.autoSync();
    if (e.key === "k" && (e.metaKey || e.ctrlKey) && model.hasSources) { e.preventDefault(); searching = !searching; }
  }} />
<UpdateGate>
<div class="grid h-full grid-cols-[220px_1fr] grid-rows-1">
  <div class="border-r"><Sidebar {model} /></div>

  <main class="relative flex min-w-0 flex-col">
    {#if model.isRefreshing}<div class="refresh-bar" title="Refreshing: checking the repo for anything new"></div>{/if}
    <!-- Top row: global search on the left, Dashboard and Logs on the right. The strip drags the window. -->
    <header data-tauri-drag-region class="relative flex h-[56px] shrink-0 items-center gap-2 px-3.5 pt-1">
      {#if model.repo && !model.error && model.hasSources && !model.onboarding}
        <button class="search" id="commit-search-open" onclick={() => (searching = true)}
          title={"Find a text like MOA-854 in commit titles and descriptions, in this repo or all of them."}>
          <Search size={13} />Search commits<kbd>⌘K</kbd>
        </button>
        <!-- The graph is the app; Dashboard is a small toggle on the side for what comes next. -->
        <div class="ml-auto flex items-center gap-1">
          <button class="side-btn" class:on={!model.showGraph} aria-pressed={!model.showGraph} id="view-dashboard"
            title={model.showGraph ? "Dashboard: coming soon" : "Back to the graph"} onclick={() => model.setShowGraph(!model.showGraph)}>
            <LayoutGrid size={14} />Dashboard
          </button>
          <LogsPanel {model} />
        </div>
      {/if}
    </header>
    {#if model.repo && model.pendingCheckout}<CheckoutPrompt target={model.pendingCheckout} {model} />{/if}

    <div class="min-h-0 flex-1" bind:clientWidth={space.width} bind:clientHeight={space.height}>
      {#if !model.hasSources || model.onboarding}
        <Onboarding {model} />
      {:else if model.error}
        <div class="flex h-full flex-col items-center justify-center gap-2 text-center text-muted-foreground">
          <TriangleAlert size={32} strokeWidth={1.5} />
          <span class="text-[15px] font-semibold text-foreground">Not a git repository</span>
          <span class="max-w-md text-[12px]">{model.error}</span>
        </div>
      {:else if !model.repo}
        <div class="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
          <Waypoints size={32} strokeWidth={1.5} />
          <span class="text-[15px] font-semibold text-foreground">No repository</span>
          <span class="text-[12px]">Pick one in the sidebar</span>
        </div>
      {:else}
        <!-- Dashboard ↔ Graph slide sideways like the tabs above, both in the same grid cell while they cross. Their own
             entrance animations (cards rising) are skipped during the slide, so the motion stays purely horizontal. -->
        {@const direction = model.showGraph ? 1 : -1}
        <div class="views">
          {#key model.showGraph}
            <div class="view" class:sliding
              in:fly={{ x: 80 * direction, duration: 380, easing: cubicOut }} out:fly={{ x: -80 * direction, duration: 380, easing: cubicOut }}
              onintrostart={() => (sliding = true)} onintroend={() => (sliding = false)}>
      {#if !model.showGraph}
        <!-- Placeholder: the tab stays for what comes next. -->
        <div class="enter-fade flex h-full flex-col items-center justify-center gap-2 text-muted-foreground" id="dashboard-placeholder">
          <LayoutGrid size={32} strokeWidth={1.5} />
          <span class="text-[15px] font-semibold text-foreground">Coming soon</span>
          <span class="text-[12px]">Everything lives in the graph for now.</span>
        </div>
      {:else if model.commits.length === 0 && model.isLoading}
        <Loader title="Loading history" detail="Reading the last {model.limit} commits (git log)" />
      {:else}
        <!-- Graph and file tree on top, diff below; with nothing selected the graph takes the whole pane. -->
        <div class="enter-fade flex h-full flex-col">
          <!-- Inside the graph view (so it slides with it): thin strip with the graph's toggles (blue when on), short labels; the full explanation on hover. -->
          <div class="flex h-[46px] shrink-0 items-center gap-1 overflow-x-auto border-b px-3" id="toolbar">
            <button class="tool" class:on={model.onlyMine} aria-pressed={model.onlyMine} id="only-mine-toggle"
              onclick={() => model.setOnlyMine(!model.onlyMine)}
              title={`My branches only: ${model.onlyMine ? "on" : "off"}\nShows only branches whose last commit is yours, plus main and the environments (qa, develop…). Ej: hides your teammates' branches so the graph stays clean.`}>
              <UserRound size={14} />My branches
            </button>
            <!-- Only with a commit selected (or while on): it focuses that commit's branch. -->
            {#if model.focusTarget}
              <button class="tool" class:on={model.focus != null} aria-pressed={model.focus != null} id="focus-branch-toggle"
                onclick={() => model.toggleFocus()}
                title={model.focus ? `Only this branch: on · ${model.focus[0]}\nThe graph shows just this branch's own commits. Click to show everything again.`
                  : `Only this branch: off\nShow just ${model.focusTarget}'s own commits, nothing else.`}>
                <Crosshair size={14} />Only this branch
              </button>
            {/if}
            <button class="tool" class:on={model.branchesOnly} aria-pressed={model.branchesOnly} id="branches-only-toggle"
              disabled={model.focus != null || model.historyPath != null}
              onclick={() => model.setBranchesOnly(!model.branchesOnly)}
              title={`Commits collapsed: ${model.branchesOnly ? "on" : "off"}\nOne row per branch with its commit count, the rest collapsed: a quick look at every branch. Click "N commits" to open one.`}>
              <GitBranch size={14} />Commits collapsed
            </button>
            {#if model.historyPath}
              <button class="tool on" id="file-history-clear" onclick={() => model.setHistory(null)}
                title={`History of ${model.historyPath}: only the commits that touched it. Click to show every commit again.`}>
                <History size={14} />History: {model.historyPath.split("/").pop()}<X size={11} />
              </button>
            {/if}
            <!-- Quick filters: the graph keeps only the matching branches, plus main and the environments. -->
            {#if model.dashboard && !model.focus && !model.historyPath}
              {@const d = model.dashboard}
              {@const cards = featureCards(d)}
              <span class="mx-2 h-5 w-px shrink-0 bg-border"></span>
              {#each FILTERS as f (f.id)}
                {@const count = cards.filter((card) => f.matches(card, d.base?.ref ?? null)).length}
                {#if f.id === "all" || count || model.graphFilter === f.id}
                  <button class="chip" class:on={model.graphFilter === f.id} id="graph-filter-{f.id}" title={f.help} onclick={() => model.setGraphFilter(f.id)}>
                    {f.label}<span class="n">{count}</span>
                  </button>
                {/if}
              {/each}
            {/if}
          </div>
          <div class="flex min-h-0 flex-1">
            <div class="relative min-w-0 flex-1">
              <CommitList {model} />
              {#if model.focus}
                <!-- Floating while "Only this branch" is on: click to go back to the graph as it was. -->
                <button class="focus-float enter-pop" id="focus-close" title="Back to the whole graph" onclick={() => model.setFocus(null)}>
                  <Crosshair size={15} />Only this branch<span class="name">{model.focus[0]}</span><X size={15} class="close" />
                </button>
              {/if}
            </div>
            {#if model.selected}
              <PaneDivider vertical bind:size={treeWidth} min={220} max={Math.max(220, space.width - 500)} />
              <div style:width="{Math.min(treeWidth, Math.max(220, space.width - 500))}px" class="enter-side shrink-0"><FileTree {model} /></div>
            {/if}
          </div>
          {#if model.selected && model.diffOpen}
            <PaneDivider vertical={false} bind:size={diffHeight} min={150} max={Math.max(150, space.height - 240)} />
            <div style:height="{Math.min(diffHeight, Math.max(150, space.height - 240))}px" class="enter-up shrink-0"><DiffPane {model} /></div>
          {/if}
        </div>
      {/if}
            </div>
          {/key}
        </div>
      {/if}
    </div>
  </main>
</div>
  <WhatsNew />
  <ActionToast {model} />
</UpdateGate>
<QuickTooltip />
<CommitSearch {model} bind:open={searching} />

<style>
  .views { display: grid; height: 100%; overflow: hidden; }
  .view { grid-area: 1 / 1; min-height: 0; height: 100%; }
  .sliding :global(*) { animation-duration: 0s !important; animation-delay: 0s !important; }
  .tool { display: flex; flex-shrink: 0; align-items: center; gap: 6px; height: 30px; padding: 0 10px; border-radius: 8px; font-size: 12.5px; font-weight: 600; color: var(--muted-foreground); }
  .tool:hover:not(:disabled) { color: var(--foreground); background: oklch(0.5 0 0 / 0.1); }
  .tool:disabled { opacity: 0.4; }
  .tool.on { color: var(--primary); background: color-mix(in oklab, var(--primary) 13%, transparent); }
  .focus-float {
    position: absolute; top: 12px; left: 50%; z-index: 20; translate: -50% 0; display: flex; align-items: center; gap: 8px; max-width: calc(100% - 24px);
    height: 38px; padding: 0 12px 0 14px; border-radius: 999px; font-size: 13.5px; font-weight: 600; color: var(--primary);
    background: color-mix(in oklab, var(--primary) 14%, var(--popover)); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--primary) 40%, transparent), 0 8px 24px rgb(0 0 0 / 0.35);
    transition: background 0.15s;
  }
  .focus-float:hover { background: color-mix(in oklab, var(--primary) 22%, var(--popover)); }
  .focus-float .name { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--foreground); }
  .focus-float :global(.close) { flex-shrink: 0; margin-left: 2px; opacity: 0.7; }
  .focus-float:hover :global(.close) { opacity: 1; }
  .side-btn { display: flex; align-items: center; gap: 6px; height: 30px; padding: 0 10px; border-radius: 8px; font-size: 12.5px; font-weight: 600; color: var(--muted-foreground); }
  .side-btn:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.1); }
  .side-btn.on { color: var(--primary); background: color-mix(in oklab, var(--primary) 12%, transparent); }
  /* Graph quick filters. */
  .chip {
    display: inline-flex; flex-shrink: 0; align-items: center; gap: 6px; height: 28px; padding: 0 6px 0 12px; border-radius: 999px;
    font-size: 12px; font-weight: 600; color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.08);
  }
  .chip:hover:not(:disabled) { color: var(--foreground); background: oklch(0.5 0 0 / 0.14); }
  .chip:disabled { opacity: 0.45; }
  .chip.on { color: white; background: var(--primary); }
  .chip .n { min-width: 18px; padding: 0 5px; border-radius: 999px; font-size: 11px; text-align: center; font-variant-numeric: tabular-nums; background: oklch(0.5 0 0 / 0.14); }
  .chip.on .n { background: rgb(255 255 255 / 0.22); }
  .search {
    display: flex; align-items: center; gap: 7px; height: 30px; width: 220px; padding: 0 10px; border-radius: 8px;
    font-size: 12px; color: var(--tertiary); background: oklch(0.5 0 0 / 0.08); box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.12);
  }
  .search:hover { color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.12); }
  .search kbd { margin-left: auto; }
  kbd { font-family: inherit; font-size: 10px; color: var(--tertiary); }
  /* Thin sliding line along the top while cached content is being refreshed. */
  .refresh-bar {
    position: absolute; top: 0; left: 0; z-index: 10; height: 2px; width: 30%; pointer-events: none;
    background: linear-gradient(to right, transparent, var(--primary), transparent);
    animation: slide 1.4s linear infinite;
  }
  @keyframes slide { from { transform: translateX(-100%); } to { transform: translateX(333%); } }
</style>
