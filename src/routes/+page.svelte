<script lang="ts">
  import { LayoutGrid, TriangleAlert, Waypoints } from "@lucide/svelte";
  import { Switch } from "$lib/components/ui/switch";
  import * as Tabs from "$lib/components/ui/tabs";
  import CheckoutPrompt from "$lib/components/CheckoutPrompt.svelte";
  import CommitList from "$lib/components/CommitList.svelte";
  import Dashboard from "$lib/components/Dashboard.svelte";
  import DiffPane from "$lib/components/DiffPane.svelte";
  import FileTree from "$lib/components/FileTree.svelte";
  import Loader from "$lib/components/Loader.svelte";
  import Onboarding from "$lib/components/Onboarding.svelte";
  import PaneDivider from "$lib/components/PaneDivider.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import UpdateGate from "$lib/components/UpdateGate.svelte";
  import WhatsNew from "$lib/components/WhatsNew.svelte";
  import { prefs, RepoModel } from "$lib/model.svelte";

  const model = new RepoModel();
  let treeWidth = $state(prefs.get("treeWidth", 360)); // remembered pane sizes
  let diffHeight = $state(prefs.get("diffHeight", 380));
  let space = $state({ width: 1200, height: 800 });
  $effect(() => prefs.set("treeWidth", treeWidth));
  $effect(() => prefs.set("diffHeight", diffHeight));
</script>

<!-- Back from the editor/terminal: pick up new commits, checkouts and uncommitted changes. -->
<svelte:window onfocus={() => model.reload()} />
<UpdateGate>
<div class="grid h-full grid-cols-[220px_1fr] grid-rows-1">
  <div class="border-r"><Sidebar {model} /></div>

  <main class="relative flex min-w-0 flex-col">
    {#if model.isRefreshing}<div class="refresh-bar" title="Refreshing: checking the repo for anything new"></div>{/if}
    <!-- Top row: view switch centered; the graph's "My branches only" filter on the right. The strip drags the window. -->
    <header data-tauri-drag-region class="relative flex h-[56px] shrink-0 items-center gap-2 px-3.5 pt-1">
      {#if model.repo && !model.error && model.hasSources && !model.onboarding}
        <Tabs.Root bind:value={() => (model.showGraph ? "graph" : "dashboard"), (v) => model.setShowGraph(v === "graph")}
          class="absolute left-1/2 -translate-x-1/2" id="view-switch">
          <Tabs.List class="h-10 w-[340px]">
            <Tabs.Trigger value="dashboard" id="view-dashboard" class="text-[14px] [&_svg:not([class*='size-'])]:size-[17px]"
              title="Dashboard: an overview of your branches. Ej: what's in qa, what's pending.">
              <LayoutGrid /> Dashboard
            </Tabs.Trigger>
            <Tabs.Trigger value="graph" id="view-graph" class="text-[14px] [&_svg:not([class*='size-'])]:size-[17px]"
              title="Graph: every commit over time. Ej: see where a branch split off.">
              <Waypoints /> Graph
            </Tabs.Trigger>
          </Tabs.List>
        </Tabs.Root>
        {#if model.showGraph}
          <label class="ml-auto flex items-center gap-2 text-[12px] font-medium text-muted-foreground"
            title="Shows only branches whose last commit is yours, plus main. Ej: hides your teammates' branches so the list stays clean.">
            My branches only
            <Switch checked={model.onlyMine} onCheckedChange={(on) => model.setOnlyMine(on)} id="only-mine-toggle" />
          </label>
        {/if}
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
      {:else if !model.showGraph}
        {#key model.repo}<div class="enter h-full"><Dashboard {model} /></div>{/key}
      {:else if model.commits.length === 0 && model.isLoading}
        <Loader title="Loading history" detail="Reading the last {model.limit} commits (git log)" />
      {:else}
        <!-- Graph and file tree on top, diff below; with nothing selected the graph takes the whole pane. -->
        <div class="enter-fade flex h-full flex-col">
          <div class="flex min-h-0 flex-1">
            <div class="min-w-0 flex-1"><CommitList {model} /></div>
            {#if model.selected}
              <PaneDivider vertical bind:size={treeWidth} min={220} max={Math.max(220, space.width - 500)} />
              <div style:width="{Math.min(treeWidth, Math.max(220, space.width - 500))}px" class="enter-fade shrink-0"><FileTree {model} /></div>
            {/if}
          </div>
          {#if model.selected}
            <PaneDivider vertical={false} bind:size={diffHeight} min={150} max={Math.max(150, space.height - 240)} />
            <div style:height="{Math.min(diffHeight, Math.max(150, space.height - 240))}px" class="enter-fade shrink-0"><DiffPane {model} /></div>
          {/if}
        </div>
      {/if}
    </div>
  </main>
</div>
  <WhatsNew />
</UpdateGate>

<style>
  /* Thin sliding line along the top while cached content is being refreshed. */
  .refresh-bar {
    position: absolute; top: 0; left: 0; z-index: 10; height: 2px; width: 30%; pointer-events: none;
    background: linear-gradient(to right, transparent, var(--primary), transparent);
    animation: slide 1.4s linear infinite;
  }
  @keyframes slide { from { transform: translateX(-100%); } to { transform: translateX(333%); } }
</style>
