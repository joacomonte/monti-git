<!-- Repos the user added (one by one or a folder of them): Favorites first, then one section per owner; groups and repos ordered by most recent activity. -->
<script lang="ts">
  import { ChevronRight, EyeOff, FolderGit2, FolderMinus, FolderTree, GitBranch, Plus, RefreshCw, RotateCw, Star, TriangleAlert, X } from "@lucide/svelte";
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
  const run = (action: () => void) => () => { menu = null; action(); };

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
            <button class="flex items-center gap-1 text-[11px] font-medium" id="sync-failure-log-toggle" onclick={() => (showLog = !showLog)}>
              <ChevronRight size={10} strokeWidth={3} class="transition-transform {showLog ? 'rotate-90' : ''}" />Raw log
            </button>
            {#if showLog}
              <pre class="h-[140px] overflow-auto rounded-md bg-quinary p-2 font-mono text-[11px] whitespace-pre-wrap select-text">{failure.log}</pre>
            {/if}
          {/if}
          <div class="flex gap-2">
            {#if failure.log}
              <Button variant="secondary" size="sm" onclick={() => copyLog(failure.log)} id="sync-failure-copy">{copied ? "Copied" : "Copy log"}</Button>
            {/if}
            <span class="flex-1"></span>
            <Button size="sm" onclick={() => (model.syncFailure = null)} id="sync-failure-dismiss">Dismiss</Button>
          </div>
        </Popover.Content>
      </Popover.Root>
    {/if}
    <!-- Always here: add a repo, or a folder whose repos are all listed. -->
    <Popover.Root bind:open={showAdd}>
      <Popover.Trigger class="grid size-7 place-items-center rounded-md text-muted-foreground hover:bg-quinary hover:text-foreground" id="add-repos"
        title="Add repositories"><Plus size={15} strokeWidth={2.5} /></Popover.Trigger>
      <Popover.Content side="right" align="start" class="w-[300px] gap-1 p-1.5">
        <ActionRow title="Add repository" detail="One or more git repos, picked one by one" icon={FolderGit2} color="#8b7bff" id="add-repository" onclick={() => add("repo")} />
        <ActionRow title="Add folder of repos" detail="Every repo inside it; new ones show up by themselves" icon={FolderTree} color="#2fc4b2" id="add-folder" onclick={() => add("folder")} />
      </Popover.Content>
    </Popover.Root>
    <Button variant="ghost" size="icon-sm" onclick={() => model.sync()} disabled={!model.repo || model.isSyncing} id="sync-button"
      title="Pull everything: downloads the latest from the server and updates every branch that's behind. Safe: it skips branches where you have unpushed commits or uncommitted changes. Ej: main is 3 behind → it catches up.">
      <RefreshCw class={model.isSyncing ? "animate-spin" : ""} />
    </Button>
    <Button variant="ghost" size="icon-sm" onclick={() => { refresh(); model.reload(); }} disabled={model.isLoading} title="Reload" id="sidebar-reload">
      <RotateCw class={model.isLoading ? "animate-spin" : ""} />
    </Button>
  </div>
  <nav class="flex-1 overflow-auto px-2 pb-3" id="repo-sidebar">
    {#each sections as [owner, list] (owner)}
      <div class="px-2 pt-3 pb-1 text-[11px] font-semibold text-tertiary">{owner}</div>
      {#each list as repo (repo.path)}
        {@const isFavorite = model.favorites.includes(repo.path)}
        <button
          class="repo flex h-[26px] w-full items-center gap-2 rounded-md px-2 text-left text-[13px]"
          class:active={model.repo === repo.path}
          title={repo.path}
          id="repo-{repo.path}"
          onclick={() => { if (model.repo !== repo.path) model.setShowGraph(false); model.open(repo.path); }}
          oncontextmenu={(e) => { e.preventDefault(); menu = { x: e.clientX, y: e.clientY, repo }; }}
        >
          <GitBranch size={13} class="shrink-0 text-muted-foreground" />
          <span class="flex-1 truncate">{nameOf(repo.path)}</span>
          <!-- span, not button: it sits inside the row's <button>. -->
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
    <div class="enter-fade fixed z-50 flex w-[230px] flex-col rounded-lg border bg-popover p-1 text-[12.5px] shadow-lg" id="repo-menu"
      style:left="{menu.x}px" style:top="{menu.y}px">
      <button class="item" id="repo-menu-favorite" onclick={run(() => model.toggleFavorite(repo.path))}>
        <Star size={13} />{isFavorite ? "Remove from Favorites" : "Add to Favorites"}
      </button>
      <div class="my-1 h-px bg-border"></div>
      {#if repo.folder}
        <button class="item" id="repo-menu-hide" onclick={run(() => model.hideRepo(repo.path))}><EyeOff size={13} />Hide from sidebar</button>
        <button class="item" id="repo-menu-remove-folder" onclick={run(() => model.removeFolder(repo.folder!))}>
          <FolderMinus size={13} /><span class="truncate">Remove folder "{folderName}"</span>
        </button>
      {:else}
        <button class="item" id="repo-menu-remove" onclick={run(() => model.removeRepo(repo.path))}><X size={13} />Remove from sidebar</button>
      {/if}
    </div>
  {/if}
  {#if model.syncStatus}
    <!-- Last action's result (sync, checkout, pull…). -->
    <div class="enter line-clamp-2 shrink-0 px-2.5 py-2 text-[10px] text-tertiary" title={model.syncStatus} id="sync-status">{model.syncStatus}</div>
  {/if}
  <div class="shrink-0 px-2.5 pb-2 text-[10px] text-tertiary" id="app-version">Monti Git v{version}</div>
</aside>

<style>
  .repo:hover { background: var(--quinary); }
  .repo.active { background: color-mix(in oklch, var(--primary) 20%, transparent); }
  .star { display: none; color: var(--tertiary); }
  .star.on { display: block; color: rgb(234 179 8 / 0.7); }
  .repo:hover .star { display: block; }
  .item { display: flex; align-items: center; gap: 8px; height: 28px; padding: 0 8px; border-radius: 6px; text-align: left; }
  .item:hover { background: var(--quinary); }
</style>
