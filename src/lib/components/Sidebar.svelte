<!-- Repos the user added (one by one or a folder of them): Favorites first, then one section per owner; groups and repos ordered by most recent activity
     (uncommitted changes count as now: discarding them sends the repo back down, committing keeps it up). -->
<script lang="ts">
  import { whatsNew } from "./WhatsNew.svelte";
  import { Check, ChevronRight, ArrowUpRight, CloudDownload, Sparkles, Copy, FolderGit2, FolderMinus, FolderTree, GitBranch, Plus, RefreshCw, Star, TriangleAlert, X } from "@lucide/svelte";
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
  import { ago, baseName } from "$lib/format";

  let { model }: { model: RepoModel } = $props();
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

  const sections = $derived.by(() => {
    const favorites = model.favorites;
    const when = (r: RepoEntry) => (r.changes ? Infinity : r.lastActivity);
    const sorted = [...model.repos].sort((a, b) => when(b) - when(a));
    const groups = new Map<string, RepoEntry[]>();
    for (const repo of sorted.filter((r) => !favorites.includes(r.path))) {
      groups.set(repo.owner, [...(groups.get(repo.owner) ?? []), repo]);
    }
    const pinned = sorted.filter((r) => favorites.includes(r.path));
    return [...(pinned.length ? [["Favorites", pinned] as const] : []),
      ...[...groups].sort((a, b) => when(b[1][0]) - when(a[1][0]))];
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
  <div data-tauri-drag-region class="flex h-[38px] shrink-0 items-center justify-end gap-1 px-2 pl-20">
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
    <!-- First item of the list, dashed: add a repo, or a folder whose repos are all listed. -->
    <Popover.Root bind:open={showAdd}>
      <Popover.Trigger class="add-repo" id="add-repos" title="Add repositories: one or more repos, or a folder with several">
        <Plus size={13} strokeWidth={2.5} class="shrink-0" />Add repository
      </Popover.Trigger>
      <Popover.Content side="right" align="start" class="w-[300px] gap-1 p-1.5">
        <ActionRow title="Add repository" detail="One or more git repos, picked one by one" icon={FolderGit2} color="#8b7bff" id="add-repository" onclick={() => add("repo")} />
        <ActionRow title="Add folder of repos" detail="Every repo inside it; new ones show up by themselves" icon={FolderTree} color="#2fc4b2" id="add-folder" onclick={() => add("folder")}
          example="pick your Dev folder: all 20 repos inside show up, and the one you clone tomorrow too." />
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
    {#each sections as [owner, list] (owner)}
      <div class="px-2 pt-5 pb-1.5 text-[11px] font-semibold text-tertiary">{owner}</div>
      {#each list as repo (repo.path)}
        {@const isFavorite = model.favorites.includes(repo.path)}
        <button
          class="repo flex h-[30px] w-full items-center gap-2 rounded-md px-2 text-left text-[13px]"
          class:active={model.repo === repo.path}
          id="repo-{repo.path}"
          onclick={() => model.open(repo.path, "graph")}
          oncontextmenu={(e) => { e.preventDefault(); menu = { x: e.clientX, y: e.clientY, repo }; }}
        >
          <!-- Hidden until hover: they grow in from the left, one after the other, and push the name right. No tooltips
               (aria-label only). Spans, not buttons: they sit inside the row's <button>. -->
          <span class="tools">
            <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
            <span role="button" tabindex="0" class="row-tool star" class:on={isFavorite} id="favorite-toggle-{baseName(repo.path)}"
              aria-label={isFavorite ? "Remove from Favorites" : "Add to Favorites"}
              onclick={(e) => { e.stopPropagation(); model.toggleFavorite(repo.path); }}>
              <Star size={14} fill={isFavorite ? "currentColor" : "none"} />
            </span>
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
      <div class="menu-separator"></div>
      {#if repo.folder}
        <button class="menu-item" title="Stop listing every repo in this folder. Nothing is deleted from disk." id="repo-menu-remove-folder" onclick={run(() => model.removeFolder(repo.folder!))}>
          <FolderMinus size={13} /><span class="truncate">Remove folder "{folderName}"</span>
        </button>
      {:else}
        <button class="menu-item" title="Take it out of the sidebar. Nothing is deleted from disk." id="repo-menu-remove" onclick={run(() => model.removeRepo(repo.path))}><X size={13} />Remove from sidebar</button>
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
    display: flex; align-items: stretch; margin: 0 0 4px; border-radius: 9px; overflow: hidden;
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
  :global(.add-repo) {
    display: flex; width: 100%; align-items: center; justify-content: center; gap: 6px; height: 30px; margin: 10px 0 6px; padding: 0 8px; border-radius: 6px;
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
  .row-tool:nth-child(2) { transition-delay: 0.04s; }
  .repo:hover .row-tool, .repo:focus-within .row-tool { width: 22px; opacity: 1; transform: none; pointer-events: auto; }
  .row-tool:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.15); }
  .repo .row-tool:active { transform: scale(0.8); }
  .star.on, .star:hover { color: rgb(234 179 8); }
  @media (prefers-reduced-motion: reduce) { .tools, .row-tool { transition: none; } }
</style>
