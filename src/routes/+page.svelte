<script lang="ts">
  import { LayoutGrid, Search, TriangleAlert, Waypoints } from "@lucide/svelte";
  import CommitSearch from "$lib/components/CommitSearch.svelte";
  import CheckoutPrompt from "$lib/components/CheckoutPrompt.svelte";
  import CommitList from "$lib/components/CommitList.svelte";
  import DiffPane from "$lib/components/DiffPane.svelte";
  import FileTree from "$lib/components/FileTree.svelte";
  import Loader from "$lib/components/Loader.svelte";
  import QuickTooltip from "$lib/components/QuickTooltip.svelte";
  import PaneDivider from "$lib/components/PaneDivider.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import UpdateGate from "$lib/components/UpdateGate.svelte";
  import WhatsNew from "$lib/components/WhatsNew.svelte";
  import ActionToast from "$lib/components/ActionToast.svelte";
  import LogsPanel from "$lib/components/LogsPanel.svelte";
  import GraphToolbar from "$lib/components/GraphToolbar.svelte";
  import { RepoModel } from "$lib/model.svelte";
  import { prefs } from "$lib/prefs";
  import { fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";

  const model = new RepoModel();
  let treeWidth = $state(prefs.get("treeWidth", 360)); // remembered pane sizes
  let diffHeight = $state(prefs.get("diffHeight", 380));
  let space = $state({ width: 1200, height: 800 });
  // Panes keep room for the graph (500px) and the tree (240px); the remembered size applies when it fits.
  const treeMax = $derived(Math.max(220, space.width - 500));
  const diffMax = $derived(Math.max(150, space.height - 240));
  let sliding = $state(false); // Dashboard ↔ Graph slide in progress
  $effect(() => prefs.set("treeWidth", treeWidth));
  $effect(() => prefs.set("diffHeight", diffHeight));
  let searching = $state(false); // ⌘K commit search open
  // Keeps every branch up to date with origin: on open, every 5 min, on focus and on any activity (below).
  $effect(() => {
    if (!model.repo) return;
    // A few seconds in, not right away: the first fetch would compete with the graph's first load.
    const first = setTimeout(() => model.autoSync(), 3000);
    const timer = setInterval(() => model.autoSync(), 5 * 60_000);
    return () => { clearTimeout(first); clearInterval(timer); };
  });
</script>

<!-- Back from the editor/terminal: pick up new commits, checkouts and uncommitted changes. -->
<!-- Any activity also syncs with origin (at most once a minute, see autoSync). -->
<!-- oncontextmenu: no browser menu (Reload / Inspect Element) on right click; text fields and selected text keep it for copy/paste. -->
<svelte:window onfocus={() => { model.reload(); model.refreshRepos(); model.autoSync(); }} onpointerdown={() => model.autoSync()} onwheel={() => model.autoSync()}
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
            title={model.showGraph ? "Dashboard: coming soon" : "Back to the graph"} onclick={() => (model.showGraph = !model.showGraph)}>
            <LayoutGrid size={14} />Dashboard
          </button>
          <LogsPanel {model} />
        </div>
      {/if}
    </header>
    {#if model.repo && model.pendingCheckout}<CheckoutPrompt checkout={model.pendingCheckout} {model} />{/if}

    <div class="min-h-0 flex-1" bind:clientWidth={space.width} bind:clientHeight={space.height}>
      {#if !model.hasSources || model.onboarding}
        {#await import("$lib/components/Onboarding.svelte") then { default: Onboarding }}<Onboarding {model} />{/await}<!-- first run only: not in the startup bundle -->
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
      {:else if model.history.commits.length === 0 && model.isLoading}
        <Loader title="Loading history" detail="Reading the latest commits (git log)" />
      {:else}
        <!-- Graph and file tree on top, diff below; with nothing selected the graph takes the whole pane. -->
        <div class="enter-fade flex h-full flex-col">
          <GraphToolbar {model} />
          <div class="flex min-h-0 flex-1">
            <div class="relative min-w-0 flex-1">
              <CommitList {model} />
            </div>
            {#if model.selected}
              <PaneDivider vertical bind:size={treeWidth} min={220} max={treeMax} id="tree-divider" />
              <div style:width="{Math.min(treeWidth, treeMax)}px" class="enter-side shrink-0"><FileTree {model} /></div>
            {/if}
          </div>
          {#if model.selected && model.diffOpen}
            <PaneDivider vertical={false} bind:size={diffHeight} min={150} max={diffMax} id="diff-divider" />
            <div style:height="{Math.min(diffHeight, diffMax)}px" class="enter-up shrink-0"><DiffPane {model} /></div>
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
  .side-btn { display: flex; align-items: center; gap: 6px; height: 30px; padding: 0 10px; border-radius: 8px; font-size: 12.5px; font-weight: 600; color: var(--muted-foreground); }
  .side-btn:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.1); }
  .side-btn.on { color: var(--primary); background: color-mix(in oklab, var(--primary) 12%, transparent); }
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
</style>
