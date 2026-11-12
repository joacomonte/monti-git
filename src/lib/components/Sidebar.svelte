<!-- Repos the user added (one by one or a folder of them): Favorites first, then one section per owner or one list, by
     recent activity or by name; repos hidden from a folder are listed in the Add or edit popover. -->
<script lang="ts">
  import { whatsNew } from "./WhatsNew.svelte";
  import { Search, ArrowDownAZ, ArrowUpDown, LayoutDashboard, Check, ChevronRight, ArrowUpRight, Clock, CloudDownload, Eye, EyeOff, Sparkles, Copy, FolderGit2, FolderMinus, FolderTree, GitBranch, Layers, List, Plus, RefreshCw, Star, TriangleAlert, X } from "@lucide/svelte";
  import ActionRow from "./ActionRow.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import { Button } from "$lib/components/ui/button";
  import * as Popover from "$lib/components/ui/popover";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { getVersion } from "@tauri-apps/api/app";
  import type { RepoEntry, SkipReason } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import { CopyFlash } from "$lib/clipboard.svelte";
  import { pickFolders } from "$lib/dialogs";
  import { prefs } from "$lib/prefs";
  import { ago, baseName } from "$lib/format";

  let { model, onsearch }: { model: RepoModel; onsearch: () => void } = $props();
  let version = $state("");
  getVersion().then((v) => (version = v));
  // Ticks so "2m ago" next to Auto sync stays current.
  let now = $state(Date.now());
  $effect(() => {
    const timer = setInterval(() => (now = Date.now()), 20_000);
    return () => clearInterval(timer);
  });

  let showAdd = $state(false);
  async function add(kind: "repo" | "folder") {
    showAdd = false;
    model.addRepos(kind, await pickFolders(kind));
  }

  // Right-click menu on a repo row.
  let menu = $state<{ x: number; y: number; repo: RepoEntry } | null>(null);
  // Act first: `repo` in the menu reads `menu.repo`, so closing first would leave it null.
  const run = (action: () => void) => () => { action(); menu = null; };
  const flash = new CopyFlash();

  // How the list is laid out (Favorites always first): grouped by owner or one list; by recent activity or by name.
  let group = $state(prefs.get("sidebarPrefs", {}).group ?? "owner");
  let sort = $state(prefs.get("sidebarPrefs", {}).sort ?? "recent");
  function layout(next: { group?: "owner" | "none"; sort?: "recent" | "name" }) {
    group = next.group ?? group;
    sort = next.sort ?? sort;
    prefs.set("sidebarPrefs", { ...prefs.get("sidebarPrefs", {}), group, sort });
  }
  let showLayout = $state(false);

  const sections = $derived.by(() => {
    const favorites = model.favorites;
    // Recent: uncommitted changes count as now (discarding them sends the repo back down, committing keeps it up).
    const when = (r: RepoEntry) => (r.changes ? Infinity : r.lastActivity);
    const order = sort === "name" ? (a: RepoEntry, b: RepoEntry) => baseName(a.path).localeCompare(baseName(b.path)) : (a: RepoEntry, b: RepoEntry) => when(b) - when(a);
    const sorted = [...model.repos].sort(order);
    const pinned = sorted.filter((r) => favorites.includes(r.path));
    const rest = sorted.filter((r) => !favorites.includes(r.path));
    const groups = new Map<string, RepoEntry[]>();
    for (const repo of rest) {
      const key = group === "owner" ? repo.owner : pinned.length ? "All" : "";
      groups.set(key, [...(groups.get(key) ?? []), repo]);
    }
    const byGroup = sort === "name" ? (a: [string, RepoEntry[]], b: [string, RepoEntry[]]) => a[0].localeCompare(b[0]) : (a: [string, RepoEntry[]], b: [string, RepoEntry[]]) => when(b[1][0]) - when(a[1][0]);
    return [...(pinned.length ? [["Favorites", pinned] as const] : []), ...[...groups].sort(byGroup)];
  });

  // The open repo stays in sight: opening one from the dashboard (graph or focus view) scrolls the list to it.
  $effect(() => {
    if (model.repo) document.getElementById(`repo-${model.repo}`)?.scrollIntoView({ block: "nearest" });
  });


  let showLog = $state(false);
  let showFailure = $state(false);
  $effect(() => { showFailure = model.syncFailure != null; }); // opens by itself when a sync skips branches; closing keeps the ⚠

  /** Why sync skipped a branch (SkipReason in src-tauri/src/actions.rs), and what to do about it. */
  const SKIPPED: Record<SkipReason, { label: string; hint: string }> = {
    diverged: { label: "diverged", hint: "You and the server both have new commits. If you still need it: check it out and pull with rebase or merge. If it's an old branch, ignore this." },
    localChanges: { label: "local changes", hint: "Commit or stash your changes, then sync again." },
    checkedOutElsewhere: { label: "checked out elsewhere", hint: "It's open in another worktree. Update it from there." },
  };
</script>

<aside class="flex h-full flex-col bg-sidebar">
  <!-- Room for the window buttons, then the controls; the strip drags the window. -->
  <div data-tauri-drag-region class="flex h-[38px] shrink-0 items-center justify-end gap-1 px-2 pl-[92px]">
    <!-- Commit search (⌘K): the full field when the strip has room, only its icon when it doesn't. -->
    {#if model.repo && model.hasSources && !model.onboarding}
      <div class="search-slot">
        <button class="search" id="commit-search-open" onclick={onsearch}
          title="Find a text like MOA-854 in commit titles and descriptions, in this repo or all of them.">
          <Search size={13} class="shrink-0" /><span class="label">Search</span><kbd>⌘K</kbd>
        </button>
      </div>
    {/if}
    {#if model.syncFailure}
      {@const failure = model.syncFailure}
      <!-- Skipped branches: stays until dismissed; the friendly reasons first, the raw git output one click away. -->
      <Popover.Root bind:open={showFailure}>
        <Popover.Trigger class="grid size-7 place-items-center text-orange-500/80" id="sync-failure-button"
          title="Some branches weren't updated. Click to see which and why."><TriangleAlert size={13} /></Popover.Trigger>
        <Popover.Content side="right" align="start" class="w-[340px] gap-3 p-3.5">
          <div class="flex items-start gap-2.5">
            <TriangleAlert size={20} class="shrink-0 text-orange-500" />
            <div class="flex flex-col gap-0.5">
              <span class="text-[13px] font-semibold">{failure.title}</span>
              <span class="text-[11px] text-muted-foreground">Nothing was lost: sync never rewrites history or touches uncommitted work.</span>
            </div>
          </div>
          <div class="flex flex-col gap-1">
            {#each failure.reasons as { branch, reason } (branch)}
              <span class="flex items-center gap-1.5 text-[12px]"><GitBranch size={12} class="shrink-0" />{branch} ({SKIPPED[reason].label})</span>
              <span class="pl-[18px] text-[11px] text-muted-foreground">{SKIPPED[reason].hint}</span>
            {/each}
          </div>
          {#if failure.log}
            <button class="flex items-center gap-1 text-[11px] font-medium" id="sync-failure-log-toggle" title="Show the raw git output for the skipped branches" onclick={() => (showLog = !showLog)}>
              <ChevronRight size={10} strokeWidth={3} class="transition-transform {showLog ? 'rotate-90' : ''}" />Raw log
            </button>
            {#if showLog}
              <pre class="h-[140px] overflow-auto rounded-md bg-quinary p-2 font-mono text-[11px] whitespace-pre-wrap select-text">{failure.log}</pre>
            {/if}
          {/if}
          <div class="flex gap-2">
            {#if failure.log}
              <Button variant="secondary" size="sm" onclick={() => flash.copy("log", failure.log)} id="sync-failure-copy" title="Copy the raw log, to paste it into an AI agent or a message">{flash.copied === "log" ? "Copied" : "Copy log"}</Button>
            {/if}
            <span class="flex-1"></span>
            <Button size="sm" onclick={() => (model.syncFailure = null)} id="sync-failure-dismiss" title="Hide this until the next sync">Dismiss</Button>
          </div>
        </Popover.Content>
      </Popover.Root>
    {/if}
  </div>
  <nav class="flex-1 overflow-auto px-2 pb-3" id="repo-sidebar">
    <!-- Dashed: add a repo or a folder whose repos are all listed, and edit what was added. -->
    <Popover.Root bind:open={showAdd}>
      <Popover.Trigger class="add-repo" id="add-repos" title="Add or edit repositories: one or more repos, or a folder with several">
        <Plus size={13} strokeWidth={2.5} class="shrink-0" />Add or edit repositories
      </Popover.Trigger>
      <Popover.Content side="right" align="start" class="max-h-[70vh] w-[300px] gap-1 overflow-auto p-1.5">
        <ActionRow title="Add repository" detail="One or more git repos, picked one by one" icon={FolderGit2} color="#8b7bff" id="add-repository" onclick={() => add("repo")} />
        <ActionRow title="Add folder of repos" detail="Every repo inside it; new ones show up by themselves" icon={FolderTree} color="#2fc4b2" id="add-folder" onclick={() => add("folder")}
          example="pick your Dev folder: all 20 repos inside show up, and the one you clone tomorrow too." />
        <!-- What was added, to take it out again (nothing is deleted from disk), and repos hidden from a folder. -->
        {#each [["Folders", model.sources.folders, FolderTree], ["Repositories", model.sources.repos, FolderGit2], ["Hidden", model.hiddenRepos, EyeOff]] as const as [title, paths, Icon] (title)}
          {#if paths.length}
            <span class="source-title">{title}</span>
            {#each paths as path (path)}
              <div class="source" id="source-{title.toLowerCase()}-{baseName(path)}" title={path}>
                <Icon size={12} class="shrink-0" /><span class="flex-1 truncate">{baseName(path)}</span>
                {#if title === "Hidden"}
                  <button id="source-show-{baseName(path)}" onclick={() => model.unhideRepo(path)}><Eye size={12} />Show</button>
                {:else}
                  <button id="source-remove-{baseName(path)}" aria-label="Remove {baseName(path)} from the sidebar"
                    onclick={() => (title === "Folders" ? model.removeFolder(path) : model.removeRepo(path))}><X size={12} /></button>
                {/if}
              </div>
            {/each}
          {/if}
        {/each}
      </Popover.Content>
    </Popover.Root>
    <!-- One pill: the body forces a pull of the open repo now and says when it last synced; the icon on its right
         toggles Auto sync (global: every repo, same setting). -->
    <div class="sync-pill" class:off={!model.autoSyncOn}>
      <button class="pull" onclick={() => model.sync()} disabled={!model.repo || model.isSyncing} id="sync-button"
        title="Pull now: updates every branch that's behind. Skips unpushed or uncommitted work.">
        <CloudDownload size={13} class="shrink-0 {model.isSyncing ? 'animate-pulse' : ''}" />
        <span class="text"><b>Pull now</b>
          <small id="last-synced">{model.isSyncing || model.isAutoSyncing ? "syncing…" : model.lastSynced ? `synced ${ago(model.lastSynced / 1000, now)}` : "not synced yet"}</small></span>
      </button>
      <button class="toggle" aria-pressed={model.autoSyncOn} id="auto-sync" onclick={() => model.setAutoSync(!model.autoSyncOn)}
        title={`Auto sync ${model.autoSyncOn ? "on" : "off"}: fetches and fast-forwards every 5 min.`}>
        <RefreshCw size={13} class={model.isAutoSyncing ? "animate-spin" : ""} />
      </button>
    </div>
    <!-- Right above the list: back to the dashboard. -->
    <button class="dashboard" class:active={!model.showGraph} id="sidebar-dashboard"
      onclick={() => { model.showGraph = false; model.dashboardView = null; }}>
      <LayoutDashboard size={15} />Dashboard
    </button>
    {#each sections as [owner, list], i (owner)}
      {#if i === 0}
        <!-- The first header also holds the one subtle icon for how the list is laid out. -->
        <div class="flex items-center pt-5 pb-1 pl-2 text-[11px] font-semibold text-tertiary">
          <span class="flex-1">{owner}</span>
  <Popover.Root bind:open={showLayout}>
    <Popover.Trigger class="layout-trigger" id="sidebar-layout" aria-label="Group and sort repos"><ArrowUpDown size={13} /></Popover.Trigger>
    <Popover.Content side="right" align="start" class="w-[170px] gap-0.5 p-1">
      {#each [["Group", [["owner", "By owner", Layers], ["none", "No groups", List]]], ["Sort", [["recent", "Recent", Clock], ["name", "Name", ArrowDownAZ]]]] as const as [title, options] (title)}
        <span class="layout-title">{title}</span>
        {#each options as [value, label, Icon] (value)}
          {@const on = title === "Group" ? group === value : sort === value}
          <button class="layout-option" id="sidebar-{title.toLowerCase()}-{value}"
            onclick={() => layout(title === "Group" ? { group: value as "owner" | "none" } : { sort: value as "recent" | "name" })}>
            <Icon size={12} />{label}{#if on}<Check size={12} class="ml-auto" />{/if}
          </button>
        {/each}
      {/each}
    </Popover.Content>
  </Popover.Root>
        </div>
      {:else if owner}<div class="px-2 pt-5 pb-1.5 text-[11px] font-semibold text-tertiary">{owner}</div>{:else}<div class="pt-3"></div>{/if}
      {#each list as repo (repo.path)}
        <button
          class="repo flex h-[30px] w-full items-center gap-2 rounded-md px-2 text-left text-[13px]"
          class:active={model.repo === repo.path}
          id="repo-{repo.path}"
          onclick={() => model.open(repo.path, "graph")}
          oncontextmenu={(e) => { e.preventDefault(); menu = { x: e.clientX, y: e.clientY, repo }; }}
        >
          <!-- Hidden until hover: it grows in from the left and pushes the name right. No tooltip
               (aria-label only). A span, not a button: it sits inside the row's <button>. Favorites: right click. -->
          <span class="tools">
            <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
            <span role="button" tabindex="0" class="row-tool" id="repo-copy-path-{baseName(repo.path)}" aria-label="Copy the full path"
              onclick={(e) => { e.stopPropagation(); flash.copy(repo.path, repo.path); }}>
              {#if flash.copied === repo.path}<Check size={13} class="text-green-500" />{:else}<Copy size={13} />{/if}
            </span>
          </span>
          <span class="flex-1 truncate">{baseName(repo.path)}</span>
          {#if model.changesDot(repo)}
            <!-- Uncommitted changes in that repo: pulsing until you open it, then still; gone once it's clean. -->
            <span class="changes" class:new={model.changesDot(repo) === "new"} id="repo-changes-{baseName(repo.path)}"></span>
          {/if}
        </button>
      {/each}
    {/each}
  </nav>
  {#if menu}
    {@const repo = menu.repo}
    {@const folderName = repo.folder ? baseName(repo.folder) : ""}
    <ContextMenu x={menu.x} y={menu.y} id="repo-menu" class="w-[230px]" onclose={() => (menu = null)}>
      <button class="menu-item" title={repo.path} id="repo-menu-copy-path" onclick={run(() => flash.copy(repo.path, repo.path))}>
        <Copy size={13} />Copy full path
      </button>
      {@const isFavorite = model.favorites.includes(repo.path)}
      <button class="menu-item" id="repo-menu-favorite" onclick={run(() => model.toggleFavorite(repo.path))}>
        <Star size={13} fill={isFavorite ? "currentColor" : "none"} />{isFavorite ? "Remove from Favorites" : "Add to Favorites"}
      </button>
      <div class="menu-separator"></div>
      <button class="menu-item" title={repo.folder ? `Hide it from the sidebar; the rest of "${folderName}" stays. Bring it back from Add or edit repositories. Nothing is deleted from disk.` : "Take it out of the sidebar. Nothing is deleted from disk."}
        id="repo-menu-remove" onclick={run(() => model.removeRepo(repo.path))}><X size={13} />Remove from sidebar</button>
      {#if repo.folder}
        <button class="menu-item" title="Stop listing every repo in this folder. Nothing is deleted from disk." id="repo-menu-remove-folder" onclick={run(() => model.removeFolder(repo.folder!))}>
          <FolderMinus size={13} /><span class="truncate">Remove folder "{folderName}"</span>
        </button>
      {/if}
    </ContextMenu>
  {/if}
  {#if model.syncStatus}
    <!-- Last action's result (sync, checkout, pull…). -->
    <div class="enter line-clamp-2 shrink-0 px-2.5 py-2 text-[10px] text-tertiary" title={model.syncStatus} id="sync-status">{model.syncStatus}</div>
  {/if}
  <!-- Footer: what's new and GitHub; one full-width row each, with a tinted icon tile. -->
  <div class="flex shrink-0 flex-col gap-1 border-t p-2">
    <!-- Opens this version's "What's new"; the dot stays until it's been read. -->
    <button class="foot-row" style:--tint="#a78bfa" id="app-version" onclick={() => whatsNew.open()}>
      <span class="tile"><Sparkles size={14} /></span>
      <span class="text"><b>What's new</b><small>Monti Git v{version}</small></span>
      {#if whatsNew.unread}<span class="news-dot"></span>{/if}
    </button>
    <button class="foot-row" style:--tint="var(--foreground)" id="github-link" onclick={() => openUrl("https://github.com/joacomonte")}>
      <!-- GitHub mark (lucide has no brand icons). -->
      <span class="tile"><svg viewBox="0 0 16 16" width="14" height="14" fill="currentColor" aria-hidden="true"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/></svg></span>
      <span class="text"><b>GitHub</b><small>joacomonte</small></span>
      <ArrowUpRight size={14} class="go" />
    </button>
  </div>
</aside>

<style>
  .foot-row {
    display: flex; width: 100%; align-items: center; gap: 10px; padding: 7px 10px 7px 7px; border-radius: 11px; text-align: left;
    background: oklch(0.5 0 0 / 0.05); transition: background 0.2s, transform 0.2s var(--ease, ease);
  }
  .foot-row:hover { background: oklch(0.5 0 0 / 0.11); }
  .foot-row:active { transform: scale(0.98); }
  .foot-row .tile {
    display: grid; place-items: center; width: 30px; height: 30px; flex-shrink: 0; border-radius: 9px;
    color: var(--tint); background: color-mix(in oklab, var(--tint) 14%, transparent); transition: transform 0.2s var(--ease, ease);
  }
  .foot-row:hover .tile { transform: scale(1.06); }
  .foot-row .text { display: flex; min-width: 0; flex: 1; flex-direction: column; line-height: 1.25; }
  .foot-row b { font-size: 12.5px; font-weight: 600; color: var(--foreground); }
  .foot-row small { font-size: 11px; color: var(--tertiary); }
  .foot-row :global(.go) { flex-shrink: 0; color: var(--tertiary); opacity: 0; transform: translate(-3px, 3px); transition: opacity 0.2s, transform 0.2s; }
  .foot-row:hover :global(.go) { opacity: 1; transform: none; }
  .news-dot { width: 6px; height: 6px; border-radius: 99px; background: var(--primary); box-shadow: 0 0 0 0 var(--primary); animation: news 2s ease-out infinite; }
  @keyframes news { 70% { box-shadow: 0 0 0 5px transparent; } 100% { box-shadow: 0 0 0 0 transparent; } }
  @media (prefers-reduced-motion: reduce) { .news-dot { animation: none; } }
  /* Auto sync (toggle, with the last sync under it) and force pull, in one pill. */
  .sync-pill {
    display: flex; align-items: stretch; margin: 0; border-radius: 9px; overflow: hidden;
    background: oklch(0.5 0 0 / 0.07); box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.12);
  }
  .sync-pill .pull { display: flex; min-width: 0; flex: 1; align-items: center; gap: 8px; padding: 6px 10px; text-align: left; color: var(--muted-foreground); }
  .sync-pill .text { display: flex; min-width: 0; flex-direction: column; line-height: 1.25; }
  .sync-pill b { font-size: 12px; font-weight: 500; }
  .sync-pill small { font-size: 10.5px; color: var(--tertiary); }
  .sync-pill .toggle { display: grid; place-items: center; width: 34px; flex-shrink: 0; color: #34c77b; border-left: 1px solid oklch(0.5 0 0 / 0.12); }
  .sync-pill.off .toggle { color: var(--tertiary); }
  .sync-pill button:hover:not(:disabled) { background: oklch(0.5 0 0 / 0.08); }
  .sync-pill .pull:hover:not(:disabled) { color: var(--foreground); }
  .sync-pill .pull:disabled { opacity: 0.45; }
  .search-slot { min-width: 0; flex: 1; container-type: inline-size; }
  .search {
    display: flex; width: 100%; align-items: center; gap: 6px; height: 26px; padding: 0 8px; border-radius: 7px;
    font-size: 12px; color: var(--tertiary); background: oklch(0.5 0 0 / 0.08); box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.12);
  }
  .search:hover { color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.12); }
  .search kbd { margin-left: auto; font-family: inherit; font-size: 10px; color: var(--tertiary); }
  @container (max-width: 96px) {
    .search { width: 26px; justify-content: center; padding: 0; margin-left: auto; }
    .search .label, .search kbd { display: none; }
  }
  .dashboard {
    display: flex; width: 100%; align-items: center; gap: 8px; height: 36px; margin: 10px 0 0; padding: 0 10px; border-radius: 8px;
    font-size: 13.5px; font-weight: 600; color: var(--muted-foreground); transition: color 0.15s, background 0.15s;
  }
  .dashboard:hover { color: var(--foreground); background: var(--quinary); }
  .dashboard.active { color: var(--foreground); }
  :global(.layout-trigger) { display: grid; place-items: center; width: 26px; height: 26px; border-radius: 6px; color: var(--tertiary); opacity: 0.7; transition: opacity 0.15s, background 0.15s; }
  :global(.layout-trigger:hover), :global(.layout-trigger[data-state="open"]) { opacity: 1; color: var(--foreground); background: oklch(0.5 0 0 / 0.08); }
  .layout-title { padding: 6px 8px 2px; font-size: 10.5px; font-weight: 600; color: var(--tertiary); }
  .layout-option { display: flex; align-items: center; gap: 7px; height: 26px; padding: 0 8px; border-radius: 5px; font-size: 12px; }
  .layout-option:hover { background: oklch(0.5 0 0 / 0.1); }
  .source-title { padding: 10px 8px 2px; font-size: 10.5px; font-weight: 600; color: var(--tertiary); }
  .source { display: flex; height: 28px; align-items: center; gap: 8px; padding: 0 4px 0 8px; border-radius: 5px; font-size: 12.5px; color: var(--muted-foreground); }
  .source:hover { background: oklch(0.5 0 0 / 0.08); }
  .source button { display: flex; align-items: center; gap: 4px; height: 22px; padding: 0 6px; border-radius: 5px; font-size: 11px; color: var(--tertiary); opacity: 0; transition: opacity 0.15s; }
  .source:hover button, .source button:focus-visible { opacity: 1; }
  .source button:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.12); }
  :global(.add-repo) {
    display: flex; width: 100%; align-items: center; justify-content: center; gap: 6px; height: 30px; margin: 10px 0 8px; padding: 0 8px; border-radius: 6px;
    font-size: 12.5px; color: var(--tertiary); border: 1px dashed oklch(0.5 0 0 / 0.3);
  }
  :global(.add-repo:hover), :global(.add-repo[data-state="open"]) { color: var(--foreground); border-color: color-mix(in oklab, var(--primary) 55%, transparent); }
  .repo:hover { background: var(--quinary); }
  .repo.active { background: color-mix(in oklch, var(--primary) 20%, transparent); }
  /* 13–14px with real contrast: at 10px and tertiary grey they vanished on non-retina Windows screens. */
  .changes { width: 7px; height: 7px; flex-shrink: 0; border-radius: 99px; background: var(--muted-foreground); opacity: 0.8; }
  .changes.new { opacity: 1; background: var(--primary); animation: changes 1.8s ease-out infinite; }
  @keyframes changes { 0% { box-shadow: 0 0 0 0 color-mix(in oklab, var(--primary) 70%, transparent); } 70%, 100% { box-shadow: 0 0 0 6px transparent; } }
  @media (prefers-reduced-motion: reduce) { .changes.new { animation: none; } }
  /* The tools take no room (the gap included) until the row is hovered; then each grows in and pops, staggered. */
  .tools { display: flex; min-width: 0; flex-shrink: 0; overflow: hidden; margin: 0 -8px 0 0; transition: margin 0.22s var(--ease, ease); }
  .repo:hover .tools, .repo:focus-within .tools { margin: 0 0 0 -4px; }
  .row-tool {
    display: grid; place-items: center; width: 0; min-width: 0; height: 22px; padding: 0; overflow: hidden; border-radius: 5px; color: var(--muted-foreground);
    opacity: 0; transform: scale(0.3) rotate(-72deg); pointer-events: none;
    transition: width 0.22s var(--ease, ease), opacity 0.18s, transform 0.32s cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .repo:hover .row-tool, .repo:focus-within .row-tool { width: 22px; opacity: 1; transform: none; pointer-events: auto; }
  .row-tool:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.15); }
  .repo .row-tool:active { transform: scale(0.8); }
  @media (prefers-reduced-motion: reduce) { .tools, .row-tool { transition: none; } }
</style>
