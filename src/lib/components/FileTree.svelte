<!-- Files of the selected commit: only the changed ones, or the whole repo with changes highlighted. -->
<script lang="ts">
  import { CaseSensitive, Check, ChevronDown, FolderGit2, Trash2, ChevronRight, ChevronsDownUp, ChevronsUpDown, Copy, Diff, FileDiff, Files, FileText, Folder, FolderOpen, History, Layers, List, ListTree, LoaderCircle, Search, SquareArrowOutUpRight, Sparkles, Square, SquareCheck, SquareMinus, TextSearch, X } from "@lucide/svelte";
  import { api, WORKTREE_ID, type Editor, type LineHits, type OpenWith } from "$lib/api";
  import * as Popover from "$lib/components/ui/popover";
  import type { Component } from "svelte";
  import type { RepoModel } from "$lib/model.svelte";
  import { CopyFlash } from "$lib/clipboard.svelte";
  import { fileTree, type FileNode } from "$lib/fileTree";
  import { prefs, type TreeSearchMode } from "$lib/prefs";
  import { isHead, refName } from "$lib/refs";
  import CommitBox from "./CommitBox.svelte";
  import ConflictBox from "./ConflictBox.svelte";
  import PaneDivider from "./PaneDivider.svelte";
  import CommitDetails from "./CommitDetails.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import DiscardButton from "./DiscardButton.svelte";
  import Segmented from "./Segmented.svelte";
  import { fade, scale } from "svelte/transition";

  let { model }: { model: RepoModel } = $props();
  // The icon theme's manifest loads on first use, not with the app.
  let fileIcon = $state<((path: string) => string) | null>(null);
  import("$lib/fileIcon").then((m) => (fileIcon = m.fileIcon));
  let flat = $state(prefs.get("treeFlat", false));
  let commitHeight = $state(prefs.get("commitBoxHeight", 240));
  $effect(() => prefs.set("commitBoxHeight", commitHeight)); // one row per file with its folder dimmed, instead of nested folders
  const flash = new CopyFlash();
  // Virtualized: only the rows in view (plus a margin) are in the DOM, so the whole project scrolls smoothly.
  const ROW = 26; // matches the row's h-[26px]
  let scrollTop = $state(0);
  let viewHeight = $state(0);
  let menu = $state<{ path: string; isFolder: boolean; deleted: boolean; x: number; y: number } | null>(null);

  // Search: file names, lines (diff lines or whole files) or both. Names filter as you type; lines run in git, debounced.
  let query = $state("");
  let mode = $state<TreeSearchMode>(prefs.get("treeSearchMode", "both"));
  let diffOnly = $state(prefs.get("treeSearchDiffOnly", true)); // only with "Show changes only": changed lines vs whole changed files
  let lineHits = $state.raw<Record<string, LineHits>>({});
  let searchingLines = $state(false);
  const words = $derived(query.trim().toLowerCase().split(/\s+/).filter(Boolean));
  const searching = $derived(words.length > 0);
  $effect(() => {
    const q = query.trim(), wantsLines = mode !== "names", onlyDiff = diffOnly && !model.showAllFiles;
    void model.paths; // new selection or tree mode: search again
    lineHits = {};
    if (!wantsLines || q.length < 2) return void (searchingLines = false);
    searchingLines = true;
    let stale = false;
    const timer = setTimeout(async () => {
      const hits = await model.searchLines(q, onlyDiff).catch(() => ({}));
      if (!stale) {
        lineHits = hits;
        searchingLines = false;
      }
    }, 250);
    return () => { stale = true; clearTimeout(timer); };
  });
  const lowerPaths = $derived(model.paths.map((p) => p.toLowerCase()));
  const visiblePaths = $derived.by(() => {
    if (!searching) return model.paths;
    const names = mode !== "lines", lines = mode !== "names";
    return model.paths.filter((p, i) => (names && words.every((w) => lowerPaths[i].includes(w))) || (lines && p in lineHits));
  });
  // Uncommitted row with untracked files: two sections, the changes git follows and, under them, the new files it doesn't
  // (deleting those can't be undone, so they get their own header and a dialog).
  const sectioned = $derived(model.untrackedApart);
  const visibleTree = $derived(!searching && !sectioned ? model.tree : fileTree(sectioned ? visiblePaths.filter((p) => !model.untracked.has(p)) : visiblePaths));
  const untrackedTree = $derived(sectioned ? fileTree(visiblePaths.filter((p) => model.untracked.has(p))) : []);
  // The search is a compact icon until clicked; it stays open while it has text, has focus, or its mode menu is in use.
  let searchBox = $state<HTMLTextAreaElement | null>(null);
  let focused = $state(false);
  let modeMenu = $state(false); // the mode menu open: the search stays open behind it
  let mirrorHeight = $state(20);
  const searchOpen = $derived(focused || modeMenu || query.length > 0);

  const MODES: Record<TreeSearchMode, { label: string; icon: Component; placeholder: string }> = {
    names: { label: "File names", icon: CaseSensitive, placeholder: "Search file names" },
    lines: { label: "Lines", icon: TextSearch, placeholder: "Search lines" },
    both: { label: "File names and lines", icon: Layers, placeholder: "Search files and lines" },
  };
  function setMode(value: TreeSearchMode) { mode = value; prefs.set("treeSearchMode", value); }
  function setDiffOnly(value: boolean) { diffOnly = value; prefs.set("treeSearchDiffOnly", value); }
  function setFlat(value: boolean) { flat = value; prefs.set("treeFlat", value); }

  /** The tree flattened to the rows on screen (children of collapsed folders skipped; flat: every file, no folders). */
  // `key` is unique per row: a folder can show in both sections ("src" holding a changed file and a new one).
  type Row = { key: string; node: FileNode; depth: number; header?: never } | { key: string; header: "untracked"; node?: never; depth?: never };
  const rows = $derived.by(() => {
    const out: Row[] = [];
    const add = (nodes: FileNode[], depth: number, section: string) => {
      for (const node of nodes) {
        if (flat && node.children) add(node.children, 0, section);
        else out.push({ key: section + node.path, node, depth });
        if (!flat && node.children && (searching || model.expandedDirs.has(section + node.path))) add(node.children, depth + 1, section);
      }
    };
    add(visibleTree, 0, "");
    if (untrackedTree.length) {
      out.push({ key: "untracked:", header: "untracked" });
      add(untrackedTree, 0, "untracked:");
    }
    return out;
  });
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 10));
  const shown = $derived(rows.slice(first, first + Math.ceil(viewHeight / ROW) + 20));

  /** Repo, checked-out branch, commit and file in one block, to paste into an AI agent so it looks at exactly this. */
  function aiContext(path: string, isFolder: boolean) {
    const head = model.history.commits.find((c) => c.refs.some((r) => r === "HEAD" || isHead(r)));
    const branch = head?.refs.find(isHead);
    const selected = model.selected;
    const commit = !selected || selected.id === WORKTREE_ID ? `${head?.id ?? "HEAD"} + uncommitted changes in the working tree` : `${selected.id} "${selected.subject}"`;
    return [`Repo: ${model.repo}`, `Branch: ${branch ? refName(branch) : "(detached HEAD)"} (checked out)`, `Commit: ${commit}`,
      `${isFolder ? "Folder" : "File"}: ${model.repo}/${path}`].join("\n");
  }

  const EXPLORER = navigator.userAgent.includes("Windows") ? "Explorer" : "Finder";

  async function openFile(path: string, how: OpenWith) {
    try {
      await api.openFile(model.repo!, path, how);
    } catch (error) {
      model.syncStatus = `Couldn't open ${path}: ${error}`;
    }
  }

  // "Open": in the editor last picked from the menu (⌥-click or right-click); the default app until then, or once it's gone.
  let editors = $state.raw<Editor[]>([]);
  api.listEditors().then((found) => (editors = found));
  let openWith = $state(prefs.get("openWith", null));
  const editor = $derived(editors.find((e) => e.id === openWith) ?? null);
  const openHow = (e: Editor | null): OpenWith => (e ? { editor: e.id } : "default");
  function openIn(path: string, choice: Editor | null) {
    openWith = choice?.id ?? null;
    prefs.set("openWith", openWith);
    openFile(path, openHow(choice));
  }

  const everything = $derived(model.stageState(""));

  // New files in subtle green, modified in subtle yellow; deletes red, renames blue.
  const COLORS: Record<string, string> = { A: "rgb(52 199 89 / 0.85)", M: "rgb(230 180 0 / 0.9)", D: "rgb(255 59 48 / 0.6)", R: "rgb(0 122 255 / 0.85)" };
</script>

{#snippet checkbox(path: string)}
  {@const { all, some, files } = model.stageState(path)}
  <!-- span, not button: rows are <button>s. -->
  <span role="checkbox" aria-checked={all ? "true" : some ? "mixed" : "false"} tabindex="0" id="stage-{path || 'all'}"
    class="flex shrink-0 {some ? 'text-primary' : 'text-tertiary hover:text-muted-foreground'}"
    title={all ? "Staged: goes into the next commit. Click to leave it out." : "Not staged. Click to include it in the next commit."}
    onclick={(e) => { e.stopPropagation(); model.setStaged(files, !all); }}
    onkeydown={(e) => { if (e.key === " " || e.key === "Enter") { e.preventDefault(); e.stopPropagation(); model.setStaged(files, !all); } }}>
    {#if all}<SquareCheck size={13} />{:else if some}<SquareMinus size={13} />{:else}<Square size={13} />{/if}
  </span>
{/snippet}

{#snippet option(id: string, Icon: Component, label: string, on: boolean, onclick: () => void)}
  <button class="opt" class:on {id} {onclick} role="menuitemradio" aria-checked={on}>
    <Icon size={14} /><span class="flex-1">{label}</span>{#if on}<Check size={13} class="text-primary" />{/if}
  </button>
{/snippet}

{#snippet tool(id: string, title: string, action: (e: MouseEvent | KeyboardEvent) => void, children: import("svelte").Snippet)}
  <!-- A span, not a button: it sits inside the row's <button>. -->
  <span role="button" tabindex="-1" class="tool-icon" {id} {title} onclick={(e) => { e.stopPropagation(); action(e); }}
    onkeydown={(e) => { if (e.key === "Enter") { e.stopPropagation(); action(e); } }}>{@render children()}</span>
{/snippet}

<div class="flex h-full flex-col">
  {#if model.isWorktreeSelected}
    <!-- The commit box takes the room you give it: drag the line under it. -->
    <!-- Its height is what it wants, not what it takes: in a short pane it shrinks instead of spilling under the diff. -->
    <!-- A merge or rebase stopped on conflicts: resolving them comes first, so it takes the commit form's place. -->
    <div class="min-h-0" style:flex="0 1 {commitHeight}px">
      {#if model.history.conflict}<ConflictBox {model} conflict={model.history.conflict} />{:else}<CommitBox {model} />{/if}
    </div>
    <PaneDivider vertical={false} before bind:size={commitHeight} min={210} max={640} id="commit-box-divider" />
  {:else if model.selected}{#key model.selected.id}<CommitDetails {model} id={model.selected.id} />{/key}{/if}
  <!-- One row: the search (a compact icon until clicked, then it takes the row and grows to more lines with long text; its
       mode in a small menu at its left), what to list (changed files or all), collapse, and View (tree or flat).
       Under it, only on the uncommitted row, how many files changed and the actions on them. -->
  <div class="toolbar" class:search-open={searchOpen}>
    <div class="flex items-start gap-1.5 p-2">
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div class="search" class:open={searchOpen} title={searchOpen ? undefined : MODES[mode].placeholder} onclick={() => searchBox?.focus()}>
        {#if searchOpen}
          <span class="flex" transition:fade={{ duration: 150 }}>
          <Popover.Root bind:open={modeMenu}>
            <Popover.Trigger class="search-mode" id="file-tree-search-mode" onpointerdown={(e: PointerEvent) => e.preventDefault()} title="Search in: {MODES[mode].label}{mode !== 'names' && !model.showAllFiles ? ` · ${diffOnly ? 'changed lines only' : 'whole files'}` : ''}">
              {#if searchingLines}<LoaderCircle size={14} class="animate-spin" />{:else}{@const Icon = MODES[mode].icon}<Icon size={14} />{/if}<ChevronDown size={11} />
            </Popover.Trigger>
            <Popover.Content align="start" class="w-[250px] p-1.5" id="file-tree-search-mode-menu">
              <span class="menu-title">Search in</span>
              {#each Object.entries(MODES) as [value, m] (value)}
                {@render option(`file-tree-search-mode-${value}`, m.icon, m.label, mode === value, () => setMode(value as TreeSearchMode))}
              {/each}
              {#if mode !== "names" && !model.showAllFiles}
                <span class="menu-title mt-1.5">Lines to look at</span>
                {@render option("file-tree-search-scope-diff", Diff, "Only the changed lines", diffOnly, () => setDiffOnly(true))}
                {@render option("file-tree-search-scope-whole", FileText, "Anywhere in the changed files", !diffOnly, () => setDiffOnly(false))}
              {/if}
            </Popover.Content>
          </Popover.Root>
          </span>
        {:else}
          <span class="flex shrink-0 px-[7px]">{#if searchingLines}<LoaderCircle size={15} class="animate-spin" />{:else}<Search size={15} />{/if}</span>
        {/if}
        <!-- A textarea that grows with the text (up to a few lines): its height comes from the invisible copy beside it, so it animates. -->
        <span class="grow-box">
          <span class="mirror" aria-hidden="true" bind:clientHeight={mirrorHeight}>{query + " "}</span>
          <textarea id="file-tree-search" rows="1" placeholder={MODES[mode].placeholder} bind:this={searchBox} bind:value={query}
            style:height="{Math.min(Math.max(mirrorHeight, 20), 110)}px"
            onfocus={() => (focused = true)} onblur={() => (focused = false)}
            onkeydown={(e) => {
              if (e.key === "Enter") e.preventDefault(); // one search, however long: it wraps, it doesn't break into lines
              if (e.key === "Escape") { query = ""; searchBox?.blur(); }
            }}></textarea>
        </span>
        {#if query}<button id="file-tree-search-clear" title="Clear" transition:fade={{ duration: 120 }} onclick={(e) => { e.stopPropagation(); query = ""; searchBox?.focus(); }}><X size={13} /></button>{/if}
      </div>
      <Segmented id="file-tree-files" value={model.showAllFiles ? "all" : "changed"} onchange={(v) => model.setShowAllFiles(v === "all")}
        options={[{ value: "changed", label: "Changed", icon: FileDiff, title: "Only the changed files" }, { value: "all", label: "All files", icon: Files, title: "Every file in the repo, changed ones marked" }]} />
      {#if !flat}
        <button class="icon-button" id="file-tree-collapse-all" title="Collapse all folders" disabled={!model.expandedDirs.size}
          transition:scale={{ duration: 150, start: 0.6 }} onclick={() => (model.expandedDirs = new Set())}><ChevronsDownUp size={16} /></button>
        <button class="icon-button" id="file-tree-expand-all" title={model.showAllFiles ? "Expand all folders" : "Expand the folders with changes"}
          transition:scale={{ duration: 150, start: 0.6 }} onclick={() => model.expandAll()}><ChevronsUpDown size={16} /></button>
      {/if}
      <Popover.Root>
        <Popover.Trigger class="view-button" id="file-tree-view" title="List as a folder tree or a flat list">
          {#if flat}<List size={15} />{:else}<ListTree size={15} />{/if}<ChevronDown size={12} />
        </Popover.Trigger>
        <Popover.Content align="end" class="w-[220px] p-1.5" id="file-tree-view-menu">
          <span class="menu-title">List as</span>
          {@render option("file-tree-layout-tree", ListTree, "Folder tree", !flat, () => setFlat(false))}
          {@render option("file-tree-layout-flat", List, "Flat list: one row per file", flat, () => setFlat(true))}
        </Popover.Content>
      </Popover.Root>
    </div>
    {#if model.isWorktreeSelected && model.changedOrder.length}
      {@const count = sectioned ? model.trackedChanges.length : model.changedOrder.length}
      <div class="flex items-center gap-1.5 px-2 pb-2">
        <span class="mr-auto truncate text-[12.5px] font-semibold text-muted-foreground"
          title={sectioned ? "Changes to files git follows: Discard puts them back as in the last commit" : undefined}>
          {sectioned ? `Changes · ${count}` : count === 1 ? "1 changed file" : `${count} changed files`}</span>
        {#if everything.files.length}
        <button class="small-action" id="stage-all" onclick={() => model.setStaged(everything.files, !everything.all)}
          title={everything.all ? "Unstage all: nothing goes into the next commit" : "Stage all: every changed file goes into the next commit"}>
          {#if everything.all}<SquareMinus size={15} /><span class="label">Unstage all</span>{:else}<SquareCheck size={15} /><span class="label">Stage all</span>{/if}
        </button>
        {/if}
        {#if model.trackedChanges.length}
          <DiscardButton title="Discard all" label id="discard-all" onconfirm={() => model.discardAll()}
            hint="Discard all: every change to the files git follows goes back to the last commit (files you staged as new get deleted). Untracked files stay." />
        {/if}
      </div>
    {/if}
  </div>
  {#if searching && !rows.length && !searchingLines}<span class="px-3 py-2 text-[12px] text-muted-foreground">No matches</span>{/if}
  <div class="flex-1 overflow-auto px-1.5 pb-2" id="file-tree" role="tree" bind:clientHeight={viewHeight}
    onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
    <div style:height="{rows.length * ROW}px" style:padding-top="{first * ROW}px" class="box-border">
    {#each shown as row (row.key)}
      {#if row.header}
        <!-- Untracked: new files git has no copy of. Their own header; deleting them always goes through the dialog. -->
        <div class="section" id="file-tree-untracked-header">
          <span class="flex-1 truncate" title="New files git doesn't follow yet: git has no copy, so deleting them can't be undone">Untracked · {model.untracked.size}</span>
          <button class="section-action" id="delete-untracked-all" title="Delete every untracked file from disk (asks first)" onclick={() => (model.doomed = [...model.untracked])}>
            <Trash2 size={13} /><span class="label">Delete all</span></button>
        </div>
      {:else}
      {@const { node, depth } = row}
      {@const status = model.changes[node.path]}
      {@const isFolder = node.children != null}
      {@const isChanged = isFolder ? model.changedDirs.has(node.path) : status != null}
      {@const dirKey = row.key.startsWith("untracked:") ? row.key : node.path}
      {@const isOpen = model.expandedDirs.has(dirKey)}
      {@const hits = searching ? lineHits[node.path] : undefined}
      <button class="file flex h-[26px] w-full items-center gap-2 rounded-md pr-2 text-left text-[12px]" class:active={model.fileFilter === node.path}
        style:padding-left="{depth * 16 + 4}px" style:--depth={depth} id="file-tree-row-{row.key}"
        onclick={() => (isFolder ? model.toggleDir(dirKey) : model.showFile(node.path, mode !== "names" && hits ? { line: hits.line, old: hits.old } : undefined))}
        oncontextmenu={(e) => { e.preventDefault(); menu = { path: node.path, isFolder, deleted: status === "D", x: e.clientX, y: e.clientY }; }}>
        <span class="flex w-3 shrink-0 justify-center text-tertiary">
          {#if isFolder}<ChevronRight size={10} strokeWidth={3} class="transition-transform {isOpen ? 'rotate-90' : ''}" />{/if}
        </span>
        {#if model.isWorktreeSelected && isChanged}{@render checkbox(node.path)}{/if}
        {#if model.nested.has(node.path)}<span title="A git repo of its own, inside this one" class="flex shrink-0"><FolderGit2 size={13} class="text-orange-500" /></span>
        {:else if isFolder}<Folder size={13} class="shrink-0 text-tertiary" />{:else}<img src={fileIcon?.(node.path)} alt="" class="size-[14px] shrink-0" />{/if}
        <span class="truncate" class:name-first={flat} class:font-semibold={isChanged} class:line-through={status === "D"}
          style:color={status ? COLORS[status] : isChanged ? "var(--foreground)" : "var(--muted-foreground)"}>{flat ? node.path.split("/").pop() : node.name}</span>
        {#if flat && node.path.includes("/")}<span class="truncate text-[11px] text-tertiary">{node.path.slice(0, node.path.lastIndexOf("/"))}</span>{/if}
        <span class="flex-1"></span>
        <!-- Hover tools. -->
        <span class="tools">
          {#snippet copyIcon()}{#if flash.copied === node.path}<Check size={12} class="text-green-500" />{:else}<Copy size={12} />{/if}{/snippet}
          {@render tool(`file-copy-${node.path}`, "Copy full path", () => flash.copy(node.path, `${model.repo}/${node.path}`), copyIcon)}
          {#if status !== "D"}
            {#snippet revealIcon()}<FolderOpen size={12} />{/snippet}
            {@render tool(`file-reveal-${node.path}`, `Show in ${EXPLORER}`, () => openFile(node.path, "reveal"), revealIcon)}
            {#if !isFolder}
              {#snippet openIcon()}<SquareArrowOutUpRight size={12} />{/snippet}
              {@render tool(`file-open-${node.path}`, `Open in ${editor?.name ?? "the default app"} (⌥-click: choose)`, (e) => {
                if (!e.altKey || !(e instanceof MouseEvent)) return openFile(node.path, openHow(editor));
                menu = { path: node.path, isFolder, deleted: false, x: e.clientX, y: e.clientY };
              }, openIcon)}
            {/if}
          {/if}
        </span>
        {#if !isFolder && model.untracked.has(node.path) && model.isWorktreeSelected}
          <!-- Untracked: deleting it can't be undone, so the dialog says so (and what a nested repo would lose). -->
          <span class="discard"><span role="button" tabindex="0" class="tool-icon delete-untracked" id="delete-untracked-{node.path}" title="Delete from disk (asks first)"
            onclick={(e) => { e.stopPropagation(); model.doomed = [node.path]; }}
            onkeydown={(e) => { if (e.key === "Enter") { e.stopPropagation(); model.doomed = [node.path]; } }}><Trash2 size={12} /></span></span>
        {:else if !isFolder && status && model.isWorktreeSelected}
          <span class="discard"><DiscardButton title="Discard file" id="discard-file-{node.path}" onconfirm={() => model.discardFile(node.path)}
            hint="Discard this file: throws away your changes to it, back to the last commit. Ej: you edited it by mistake." /></span>
        {/if}
        {#if hits}
          <span class="hits" title="{hits.count} matching line{hits.count === 1 ? '' : 's'}: click to go to the first one">{hits.count}</span>
        {/if}
        {#if model.counts[node.path]}
          {@const [added, removed] = model.counts[node.path]}
          <span class="flex shrink-0 gap-1 text-[10px] font-medium tabular-nums">
            {#if added}<span class="text-green-500/70">+{added}</span>{/if}
            {#if removed}<span class="text-red-500/70">−{removed}</span>{/if}
          </span>
        {/if}
      </button>
      {/if}
    {/each}
    </div>
  </div>
</div>

{#if menu}
  {@const m = menu}
  {@const close = () => (menu = null)}
  <!-- Right-click menu: same actions as the hover icons. -->
  <ContextMenu x={m.x} y={m.y} id="file-tree-menu" onclose={close}>
    <button role="menuitem" class="menu-item" id="file-menu-copy" onclick={() => { flash.copy(m.path, `${model.repo}/${m.path}`); close(); }}>
      <Copy size={13} />Copy full path
    </button>
    <button role="menuitem" class="menu-item" id="file-menu-copy-ai" onclick={() => { navigator.clipboard.writeText(aiContext(m.path, m.isFolder)); close(); }}
      title="Repo, branch, commit hash and path in one block, ready to paste into your AI agent">
      <Sparkles size={13} />Copy for AI<span class="menu-hint">repo · branch · hash</span>
    </button>
    {#if !m.deleted}
      <button role="menuitem" class="menu-item" id="file-menu-history" onclick={() => { model.setHistory(m.path); close(); }}
        title="The graph shows only the commits that touched it">
        <History size={13} />History
      </button>
      <button role="menuitem" class="menu-item" id="file-menu-reveal" onclick={() => { openFile(m.path, "reveal"); close(); }}>
        <FolderOpen size={13} />Show in {EXPLORER}
      </button>
      {#if !m.isFolder}
        <div class="menu-separator"></div>
        {#each [...editors, null] as choice (choice?.id ?? "default")}
          <button role="menuitem" class="menu-item" id="file-menu-open-{choice?.id ?? 'default'}" onclick={() => { openIn(m.path, choice); close(); }}>
            <SquareArrowOutUpRight size={13} />{choice ? `Open in ${choice.name}` : "Open with default app"}
            {#if (choice?.id ?? null) === (editor?.id ?? null)}<Check size={12} class="ml-auto" />{/if}
          </button>
        {/each}
      {/if}
    {/if}
  </ContextMenu>
{/if}

<style>
  .file:hover { background: var(--quinary); }
  .toolbar { container-type: inline-size; }
  @container (max-width: 360px) { .toolbar :global(.label) { display: none; } }
  /* Compact (just the icon) until clicked; open, it takes the row's free room. The width animates. */
  .search { position: relative; display: flex; flex: 0 0 34px; min-width: 0; min-height: 30px; align-items: flex-start; gap: 4px; padding: 5px 8px 5px 2px;
    border-radius: 8px; overflow: hidden; cursor: text; color: var(--tertiary); background: oklch(0.5 0 0 / 0.08);
    transition: flex-grow 0.28s var(--ease), flex-basis 0.28s var(--ease), background 0.2s, box-shadow 0.2s; }
  .search:not(.open) { cursor: pointer; padding-right: 2px; }
  .search:not(.open):hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.15); }
  .search.open { flex: 1 1 0%; box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--primary) 45%, transparent); }
  .search > span:first-child { align-self: center; }
  .grow-box { position: relative; display: block; flex: 1; min-width: 0; }
  .search:not(.open) .grow-box { opacity: 0; }
  .mirror, .search textarea { display: block; width: 100%; font-size: 13px; line-height: 20px; white-space: pre-wrap; overflow-wrap: anywhere; }
  .mirror { position: absolute; inset: 0 0 auto 0; visibility: hidden; pointer-events: none; }
  .search textarea { resize: none; overflow-y: auto; color: var(--foreground); background: none; outline: none; transition: height 0.18s var(--ease); }
  .search button { align-self: center; }
  .search button:hover { color: var(--foreground); }
  .search :global(.search-mode) { display: flex; flex-shrink: 0; align-items: center; gap: 2px; height: 22px; padding: 0 5px; border-radius: 6px; color: var(--muted-foreground); }
  .search :global(.search-mode:hover) { color: var(--foreground); background: oklch(0.5 0 0 / 0.12); }
  /* With the search open, what's beside it keeps only its icons (names stay as tooltips). */
  .toolbar :global(.seg .label) { transition: opacity 0.2s; }
  .toolbar.search-open > div:first-child :global(.seg .label) { display: none; }
  .toolbar :global(.seg button) { height: 30px; }
  .icon-button { display: grid; flex-shrink: 0; place-items: center; width: 30px; height: 30px; border-radius: 8px; color: var(--muted-foreground); }
  .icon-button:hover:not(:disabled) { color: var(--foreground); background: oklch(0.5 0 0 / 0.12); }
  .icon-button:disabled { opacity: 0.35; }
  .toolbar :global(.view-button) { display: flex; flex-shrink: 0; align-items: center; gap: 4px; height: 30px; margin-left: auto; padding: 0 8px; border-radius: 8px;
    font-size: 12.5px; font-weight: 600; color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.08); }
  .toolbar :global(.view-button:hover) { color: var(--foreground); background: oklch(0.5 0 0 / 0.16); }
  .menu-title { display: block; padding: 4px 8px 4px; font-size: 10.5px; font-weight: 600; text-transform: uppercase; color: var(--tertiary); }
  .opt { display: flex; width: 100%; align-items: center; gap: 8px; height: 30px; padding: 0 8px; border-radius: 6px; font-size: 12.5px; text-align: left; color: var(--muted-foreground); }
  .opt:hover:not(:disabled) { color: var(--foreground); background: oklch(0.5 0 0 / 0.12); }
  .opt.on { color: var(--foreground); }
  .opt:disabled { opacity: 0.4; }
  .hits { flex-shrink: 0; padding: 0 5px; border-radius: 99px; font-size: 10px; font-weight: 600; color: var(--primary); background: color-mix(in oklab, var(--primary) 16%, transparent); }
  .name-first { flex-shrink: 0; max-width: 75%; } /* flat list: the file name wins over its dimmed folder */
  /* Stage/Unstage all and Discard all: real buttons, same size as the segments; only the icon when narrow. */
  /* The Untracked section's header, a row of the list. */
  .section { display: flex; height: 26px; align-items: center; gap: 6px; margin-top: 0; padding: 0 4px 0 8px; border-top: 1px solid oklch(0.5 0 0 / 0.15);
    font-size: 11px; font-weight: 700; letter-spacing: 0.03em; text-transform: uppercase; color: var(--tertiary); }
  .section-action { display: flex; align-items: center; gap: 5px; height: 22px; padding: 0 8px; border-radius: 6px; font-size: 11.5px; font-weight: 600;
    letter-spacing: 0; text-transform: none; color: var(--muted-foreground); }
  .section-action:hover { color: #ef4444; background: rgb(239 68 68 / 0.12); }
  .delete-untracked:hover { color: #ef4444 !important; }
  .small-action, .toolbar :global(#discard-all) { display: flex; flex-shrink: 0; align-items: center; gap: 6px; height: 28px; padding: 0 10px; border-radius: 7px;
    font-size: 12.5px; font-weight: 600; color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.1); }
  .small-action:hover, .toolbar :global(#discard-all:hover) { color: var(--foreground); background: oklch(0.5 0 0 / 0.18); }
  .toolbar :global(#discard-all svg) { width: 15px; height: 15px; stroke-width: 2; }
  .toolbar :global([id^="discard-all-"]) { width: 28px; height: 28px; } /* its ✕ ✓ confirm, as big as the button it replaced */
  .toolbar :global([id^="discard-all-"] svg) { width: 13px; height: 13px; }
  /* Only on hover, like the Swift row; stays while armed (✕ ✓) so the confirm isn't lost when the mouse moves. */
  .discard { display: none; }
  .file:hover .discard, .discard:has(:global([id$="-confirm"])) { display: contents; }
  /* Copy / open: hidden until hover, then fade in; the icon itself brightens under the pointer. */
  .tools { display: flex; flex-shrink: 0; gap: 1px; opacity: 0; width: 0; overflow: hidden; transition: opacity 0.15s; }
  .file:hover .tools { opacity: 1; width: auto; }
  .tools :global(.tool-icon) { display: grid; place-items: center; width: 20px; height: 20px; border-radius: 5px; color: var(--tertiary); }
  .tools :global(.tool-icon:hover) { color: var(--foreground); background: oklch(0.5 0 0 / 0.14); }
  .file.active { background: color-mix(in oklch, var(--primary) 18%, transparent); }
  /* One faint vertical guide per ancestor level, centered under that level's chevron. */
  .file {
    background-image: repeating-linear-gradient(to right, var(--border) 0 1px, transparent 1px 16px);
    background-size: calc(var(--depth) * 16px) 100%;
    background-position: 9.5px 0;
    background-repeat: no-repeat;
  }
</style>
