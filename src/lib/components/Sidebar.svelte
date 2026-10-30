<!-- Repos the user added (one by one or a folder of them): Favorites first, then one section per owner; groups and repos ordered by most recent activity. -->
<script lang="ts">
  import { whatsNew } from "./WhatsNew.svelte";
  import { fade } from "svelte/transition";
  import { Check, ChevronRight, CloudDownload, History, Undo2, Copy, EyeOff, FolderGit2, FolderMinus, FolderTree, GitBranch, Plus, RefreshCw, Star, TriangleAlert, X } from "@lucide/svelte";
  import ActionRow from "./ActionRow.svelte";
  import { Button } from "$lib/components/ui/button";
  import * as Popover from "$lib/components/ui/popover";
  import { getVersion } from "@tauri-apps/api/app";
  import { api, type RepoEntry } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";

  let { model }: { model: RepoModel } = $props();
  let repos = $state<RepoEntry[]>([]);
  let version = $state("");
  getVersion().then((v) => (version = v));
  // Ticks so "2m ago" next to Auto sync stays current.
  let now = $state(Date.now());
  $effect(() => {
    const timer = setInterval(() => (now = Date.now()), 20_000);
    return () => clearInterval(timer);
  });
  const ago = (ms: number) => {
    const s = (now - ms) / 1000;
    return s < 60 ? "just now" : s < 3600 ? `${Math.floor(s / 60)}m ago` : s < 86400 ? `${Math.floor(s / 3600)}h ago` : `${Math.floor(s / 86400)}d ago`;
  };

  async function refresh() {
    const { folders, repos: added, hidden } = model.sources;
    repos = (await api.listRepos(folders, added)).filter((r) => !hidden.includes(r.path));
  }
  $effect(() => {
    model.sources; // relist whenever a folder or repo is added, removed or hidden
    refresh();
  });

  let showAdd = $state(false);
  const add = (kind: "repo" | "folder") => { showAdd = false; model.addRepos(kind); };

  // Right-click menu on a repo row.
  let menu = $state<{ x: number; y: number; repo: RepoEntry } | null>(null);
  // Act first: `repo` in the menu reads `menu.repo`, so closing first would leave it null.
  const run = (action: () => void) => () => { action(); menu = null; };

  let copiedPath = $state<string | null>(null);
  async function copyPath(path: string) {
    await navigator.clipboard.writeText(path);
    copiedPath = path;
    setTimeout(() => { if (copiedPath === path) copiedPath = null; }, 1200);
  }

  const sections = $derived.by(() => {
    const favorites = model.favorites;
    const sorted = [...repos].sort((a, b) => b.lastActivity - a.lastActivity);
    const groups = new Map<string, RepoEntry[]>();
    for (const repo of sorted.filter((r) => !favorites.includes(r.path))) {
      groups.set(repo.owner, [...(groups.get(repo.owner) ?? []), repo]);
    }
    const pinned = sorted.filter((r) => favorites.includes(r.path));
    return [...(pinned.length ? [["Favorites", pinned] as const] : []),
      ...[...groups].sort((a, b) => b[1][0].lastActivity - a[1][0].lastActivity)];
  });

  const nameOf = (path: string) => path.split(/[\\/]/).pop();

  let showLog = $state(false);
  let showFailure = $state(false);
  $effect(() => { showFailure = model.syncFailure != null; }); // opens by itself when a sync skips branches; closing keeps the ⚠
  let copied = $state(false);

  /** What to do about each skip reason (the labels come from `sync` in src-tauri/src/actions.rs). */
  function hint(reason: string) {
    if (reason.endsWith("(diverged)")) return "You and the server both have new commits. If you still need it: check it out and pull with rebase or merge. If it's an old branch, ignore this.";
    if (reason.endsWith("(local changes)")) return "Commit or stash your changes, then sync again.";
    if (reason.endsWith("(checked out elsewhere)")) return "It's open in another worktree. Update it from there.";
    return null;
  }

  async function copyLog(log: string) {
    await navigator.clipboard.writeText(log);
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }
</script>

<svelte:window onclick={() => (menu = null)} onkeydown={(e) => { if (e.key === "Escape") menu = null; }} onblur={() => (menu = null)} />

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
            {#each failure.reasons as reason (reason)}
              <span class="flex items-center gap-1.5 text-[12px]"><GitBranch size={12} class="shrink-0" />{reason}</span>
              {#if hint(reason)}<span class="pl-[18px] text-[11px] text-muted-foreground">{hint(reason)}</span>{/if}
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
              <Button variant="secondary" size="sm" onclick={() => copyLog(failure.log)} id="sync-failure-copy" title="Copy the raw log, to paste it into an AI agent or a message">{copied ? "Copied" : "Copy log"}</Button>
            {/if}
            <span class="flex-1"></span>
            <Button size="sm" onclick={() => (model.syncFailure = null)} id="sync-failure-dismiss" title="Hide this until the next sync">Dismiss</Button>
          </div>
        </Popover.Content>
      </Popover.Root>
    {/if}
  </div>
  <!-- Global (every repo, same setting), so it lives in the sidebar. On by default; the line under it forces a pull right now. -->
  <!-- Toggle button, same look on or off: the label says which. -->
  <button class="sync-button" aria-pressed={model.autoSyncOn} id="auto-sync"
    onclick={() => model.setAutoSync(!model.autoSyncOn)}
    title={`Auto sync: ${model.autoSyncOn ? "on" : "off"}\nKeeps every branch up to date with origin on its own: fetch and safe fast-forwards when you use the app (at most once a minute) and every 5 minutes. Never merges or rewrites anything.`}>
    <RefreshCw size={12} class="shrink-0 {model.isSyncing ? 'animate-spin' : ''}" />Auto sync {model.autoSyncOn ? "on" : "off"}
  </button>
  <button class="sync-button pull" onclick={() => model.sync()} disabled={!model.repo || model.isSyncing} id="sync-button"
    title="Force pull now: downloads the latest from the server and updates every branch that's behind. Safe: it skips branches where you have unpushed commits or uncommitted changes. Ej: main is 3 behind → it catches up.">
    <CloudDownload size={11} class={model.isSyncing ? "animate-pulse" : ""} />Force pull now
  </button>
  <span class="synced" id="last-synced">{model.isSyncing ? "syncing…" : model.lastSynced ? `synced ${ago(model.lastSynced)}` : ""}</span>
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
    {#each sections as [owner, list] (owner)}
      <div class="px-2 pt-5 pb-1.5 text-[11px] font-semibold text-tertiary">{owner}</div>
      {#each list as repo (repo.path)}
        {@const isFavorite = model.favorites.includes(repo.path)}
        <button
          class="repo flex h-[30px] w-full items-center gap-2 rounded-md px-2 text-left text-[13px]"
          class:active={model.repo === repo.path}
          title={repo.path}
          id="repo-{repo.path}"
          onclick={() => model.open(repo.path)}
          oncontextmenu={(e) => { e.preventDefault(); menu = { x: e.clientX, y: e.clientY, repo }; }}
        >
          <GitBranch size={13} class="shrink-0 text-muted-foreground" />
          <span class="flex-1 truncate">{nameOf(repo.path)}</span>
          <!-- spans, not buttons: they sit inside the row's <button>. -->
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <span role="button" tabindex="0" class="copy shrink-0" class:on={copiedPath === repo.path} id="copy-path-{nameOf(repo.path)}"
            title="Copy full path: {repo.path}"
            onclick={(e) => { e.stopPropagation(); copyPath(repo.path); }}>
            {#if copiedPath === repo.path}<Check size={10} />{:else}<Copy size={10} />{/if}
          </span>
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <span role="button" tabindex="0" class="star shrink-0" class:on={isFavorite} id="favorite-toggle-{nameOf(repo.path)}"
            title={isFavorite ? "Remove from Favorites" : "Add to Favorites: keeps this repo at the top of the list."}
            onclick={(e) => { e.stopPropagation(); model.toggleFavorite(repo.path); }}>
            <Star size={10} fill={isFavorite ? "currentColor" : "none"} />
          </span>
        </button>
      {/each}
    {/each}
  </nav>
  {#if menu}
    {@const repo = menu.repo}
    {@const isFavorite = model.favorites.includes(repo.path)}
    {@const folderName = repo.folder ? nameOf(repo.folder) : ""}
    <div out:fade={{ duration: 120 }} class="enter-pop fixed z-50 flex w-[230px] flex-col rounded-lg border bg-popover p-1 text-[12.5px] shadow-lg" id="repo-menu"
      style:left="{Math.min(menu.x, innerWidth - 240)}px" style:top="{Math.min(menu.y, innerHeight - 200)}px">
      <button class="item" title="Favorites stay at the top of the sidebar" id="repo-menu-favorite" onclick={run(() => model.toggleFavorite(repo.path))}>
        <Star size={13} />{isFavorite ? "Remove from Favorites" : "Add to Favorites"}
      </button>
      <button class="item" title={repo.path} id="repo-menu-copy-path" onclick={run(() => copyPath(repo.path))}>
        <Copy size={13} />Copy full path
      </button>
      <div class="my-1 h-px bg-border"></div>
      {#if repo.folder}
        <button class="item" title="Take it out of the list; it comes back if you add it again" id="repo-menu-hide" onclick={run(() => model.hideRepo(repo.path))}><EyeOff size={13} />Hide from sidebar</button>
        <button class="item" title="Stop listing every repo in this folder. Nothing is deleted from disk." id="repo-menu-remove-folder" onclick={run(() => model.removeFolder(repo.folder!))}>
          <FolderMinus size={13} /><span class="truncate">Remove folder "{folderName}"</span>
        </button>
      {:else}
        <button class="item" title="Take it out of the sidebar. Nothing is deleted from disk." id="repo-menu-remove" onclick={run(() => model.removeRepo(repo.path))}><X size={13} />Remove from sidebar</button>
      {/if}
    </div>
  {/if}
  {#if model.syncStatus}
    <!-- Last action's result (sync, checkout, pull…). -->
    <div class="enter line-clamp-2 shrink-0 px-2.5 py-2 text-[10px] text-tertiary" title={model.syncStatus} id="sync-status">{model.syncStatus}</div>
  {/if}
  <div class="flex shrink-0 items-center justify-between px-2.5 pb-2 text-[10px] text-tertiary">
    <!-- Opens this version's "What's new"; the dot stays until it's been read. -->
    <button class="version" id="app-version" onclick={() => whatsNew.open()} title={whatsNew.unread ? "New in this version: see what changed" : "What's new in this version"}>
      Monti Git v{version}{#if whatsNew.unread}<span class="news-dot"></span>{/if}
    </button>
    <Popover.Root>
      <Popover.Trigger class="activity-trigger" id="activity-open" title="What you did from the app in this repo, with Undo where it's safe"><History size={11} />Activity</Popover.Trigger>
      <Popover.Content side="top" align="end" class="w-[340px] p-1.5" id="activity-panel">
        <span class="block px-2 pt-1 pb-1.5 text-[11px] font-semibold text-muted-foreground uppercase">Activity · {model.repo ? nameOf(model.repo) : ""}</span>
        {#each model.activity.filter((e) => e.repo === model.repo) as entry, i (entry.at + entry.text)}
          <div class="activity-row" id="activity-{i}">
            <span class="min-w-0 flex-1">
              <span class="block truncate" class:line-through={entry.undone} title={entry.text}>{entry.text}</span>
              <span class="text-[10.5px] text-tertiary">{ago(entry.at)}{entry.undone ? " · undone" : ""}</span>
            </span>
            {#if entry.undo && !entry.undone}
              <button class="undo" id="activity-undo-{i}" disabled={model.isSyncing}
                title={entry.undo.kind === "checkout" ? `Switch back to ${entry.undo.branch}` : entry.undo.expected ? `Put ${entry.undo.branch} back where it was` : `Recreate ${entry.undo.branch} (local)`}
                onclick={() => model.undo(entry)}><Undo2 size={12} />Undo</button>
            {/if}
          </div>
        {:else}
          <p class="px-2 py-4 text-center text-[12px] text-tertiary">Nothing yet. Checkouts, commits, squashes, deletes… show up here.</p>
        {/each}
      </Popover.Content>
    </Popover.Root>
  </div>
</aside>

<style>
  .version { display: flex; align-items: center; gap: 5px; padding: 1px 5px; margin-left: -5px; border-radius: 5px; }
  .version:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.1); }
  .news-dot { width: 6px; height: 6px; border-radius: 99px; background: var(--primary); box-shadow: 0 0 0 0 var(--primary); animation: news 2s ease-out infinite; }
  @keyframes news { 70% { box-shadow: 0 0 0 5px transparent; } 100% { box-shadow: 0 0 0 0 transparent; } }
  @media (prefers-reduced-motion: reduce) { .news-dot { animation: none; } }
  /* Auto sync and Force pull: two stacked buttons, same width and look. */
  .sync-button {
    display: flex; flex-shrink: 0; align-items: center; gap: 7px; height: 30px; margin: 0 8px 6px; padding: 0 10px; border-radius: 8px;
    font-size: 12px; font-weight: 500; color: var(--muted-foreground);
    background: oklch(0.5 0 0 / 0.07); box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.12);
  }
  #auto-sync { margin-top: 10px; }
  .sync-button:hover:not(:disabled) { color: var(--foreground); background: oklch(0.5 0 0 / 0.12); }
  :global(.add-repo) {
    display: flex; width: 100%; align-items: center; justify-content: center; gap: 6px; height: 30px; margin: 10px 0 6px; padding: 0 8px; border-radius: 6px;
    font-size: 12.5px; color: var(--tertiary); border: 1px dashed oklch(0.5 0 0 / 0.3);
  }
  :global(.add-repo:hover), :global(.add-repo[data-state="open"]) { color: var(--foreground); border-color: color-mix(in oklab, var(--primary) 55%, transparent); }
  /* Second line under Auto sync, quiet until hovered. */
  .synced { flex-shrink: 0; height: 14px; margin: -2px 18px 10px; font-size: 10.5px; color: var(--tertiary); }
  .pull:disabled { opacity: 0.45; }
  :global(.activity-trigger) { display: flex; align-items: center; gap: 4px; padding: 1px 5px; border-radius: 5px; }
  :global(.activity-trigger:hover), :global(.activity-trigger[data-state="open"]) { color: var(--foreground); background: oklch(0.5 0 0 / 0.1); }
  .activity-row { display: flex; align-items: center; gap: 8px; padding: 5px 8px; border-radius: 6px; font-size: 12px; }
  .activity-row:hover { background: oklch(0.5 0 0 / 0.07); }
  .undo { display: flex; flex-shrink: 0; align-items: center; gap: 4px; height: 22px; padding: 0 7px; border-radius: 6px; font-size: 11px; color: var(--muted-foreground); box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.2); }
  .undo:hover:not(:disabled) { color: var(--foreground); background: oklch(0.5 0 0 / 0.1); }
  .undo:disabled { opacity: 0.45; }
  .repo:hover { background: var(--quinary); }
  .repo.active { background: color-mix(in oklch, var(--primary) 20%, transparent); }
  .star { display: none; color: var(--tertiary); }
  .star.on { display: block; color: rgb(234 179 8 / 0.7); }
  .repo:hover .star { display: block; }
  .copy { display: none; color: var(--tertiary); }
  .copy:hover { color: var(--foreground); }
  .copy.on { display: block; color: oklch(0.72 0.17 150); }
  .repo:hover .copy { display: block; }
  .item { display: flex; align-items: center; gap: 8px; height: 28px; padding: 0 8px; border-radius: 6px; text-align: left; }
  .item:hover { background: var(--quinary); }
</style>
