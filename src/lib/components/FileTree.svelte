<!-- Files of the selected commit: only the changed ones, or the whole repo with changes highlighted. -->
<script lang="ts">
  import { CaseSensitive, Check, ChevronRight, ChevronsDownUp, Copy, Diff, FileText, Folder, FolderOpen, History, Layers, List, ListTree, LoaderCircle, Search, SquareArrowOutUpRight, Sparkles, Square, SquareCheck, SquareMinus, TextSearch, X } from "@lucide/svelte";
  import { api, WORKTREE_ID, type Editor, type LineHits, type OpenWith } from "$lib/api";
  import { Switch } from "$lib/components/ui/switch";
  import type { RepoModel } from "$lib/model.svelte";
  import { CopyFlash } from "$lib/clipboard.svelte";
  import { fileTree, type FileNode } from "$lib/fileTree";
  import { prefs, type TreeSearchMode } from "$lib/prefs";
  import { isHead, refName } from "$lib/refs";
  import CommitBox from "./CommitBox.svelte";
  import CommitDetails from "./CommitDetails.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import DiscardButton from "./DiscardButton.svelte";
  import Segmented from "./Segmented.svelte";

  let { model }: { model: RepoModel } = $props();
  // The icon theme's manifest loads on first use, not with the app.
  let fileIcon = $state<((path: string) => string) | null>(null);
  import("$lib/fileIcon").then((m) => (fileIcon = m.fileIcon));
  let flat = $state(prefs.get("treeFlat", false)); // one row per file with its folder dimmed, instead of nested folders
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
  const visibleTree = $derived.by(() => {
    if (!searching) return model.tree;
    const names = mode !== "lines", lines = mode !== "names";
    return fileTree(model.paths.filter((p, i) => (names && words.every((w) => lowerPaths[i].includes(w))) || (lines && p in lineHits)));
  });

  function setMode(value: TreeSearchMode) { mode = value; prefs.set("treeSearchMode", value); }
  function setDiffOnly(value: boolean) { diffOnly = value; prefs.set("treeSearchDiffOnly", value); }
  function setFlat(value: boolean) { flat = value; prefs.set("treeFlat", value); }

  /** The tree flattened to the rows on screen (children of collapsed folders skipped; flat: every file, no folders). */
  const rows = $derived.by(() => {
    const out: { node: FileNode; depth: number }[] = [];
    const add = (nodes: FileNode[], depth: number) => {
      for (const node of nodes) {
        if (flat && node.children) add(node.children, 0);
        else out.push({ node, depth });
        if (!flat && node.children && (searching || model.expandedDirs.has(node.path))) add(node.children, depth + 1);
      }
    };
    add(visibleTree, 0);
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

{#snippet tool(id: string, title: string, action: (e: MouseEvent | KeyboardEvent) => void, children: import("svelte").Snippet)}
  <!-- A span, not a button: it sits inside the row's <button>. -->
  <span role="button" tabindex="-1" class="tool-icon" {id} {title} onclick={(e) => { e.stopPropagation(); action(e); }}
    onkeydown={(e) => { if (e.key === "Enter") { e.stopPropagation(); action(e); } }}>{@render children()}</span>
{/snippet}

<div class="flex h-full flex-col">
  {#if model.isWorktreeSelected}<CommitBox {model} />
  {:else if model.selected}{#key model.selected.id}<CommitDetails {model} id={model.selected.id} />{/key}{/if}
  <!-- Narrow pane: texts become icons first (their titles stay as tooltips), then what still doesn't fit wraps. -->
  <div class="toolbar">
    <div class="flex flex-wrap items-center gap-1.5 p-2">
      <!-- View controls on the left (layout, collapse, what to list); actions on the right. -->
      <Segmented id="file-tree-layout" value={flat ? "flat" : "tree"} onchange={(v) => setFlat(v === "flat")}
        options={[{ value: "tree", icon: ListTree, title: "Folder tree" }, { value: "flat", icon: List, title: "Flat list: one row per file, its folder dimmed" }]} />
      {#if !flat}
        <button class="collapse-all" id="file-tree-collapse-all" title="Collapse all folders" disabled={!model.expandedDirs.size}
          onclick={() => (model.expandedDirs = new Set())}><ChevronsDownUp size={14} /></button>
      {/if}
      <label class="ml-1 flex items-center gap-1.5 text-[11px] font-medium whitespace-nowrap text-muted-foreground"
        title="Show changes only. On: only the files that changed. Off: every file in the repo, changed ones marked">
        <Switch checked={!model.showAllFiles} onCheckedChange={(on) => model.setShowAllFiles(!on)} id="file-tree-changes-only" /><span class="label">Show changes only</span>
      </label>
      <div class="ml-auto flex items-center gap-1.5">
        {#if model.isWorktreeSelected && everything.files.length}
          <button class="tool" id="stage-all" onclick={() => model.setStaged(everything.files, !everything.all)}
            title={everything.all ? "Unstage all: nothing goes into the next commit" : "Stage all: every changed file goes into the next commit"}>
            {#if everything.all}<SquareMinus size={10} strokeWidth={2.5} /><span class="label">Unstage all</span>{:else}<SquareCheck size={10} strokeWidth={2.5} /><span class="label">Stage all</span>{/if}
          </button>
        {/if}
        {#if model.isWorktreeSelected && model.changedOrder.length}
          <DiscardButton title="Discard all" label id="discard-all" onconfirm={() => model.discardAll()}
            hint="Discard all: throws away every uncommitted change, back to the last commit. New files get deleted too." />
        {/if}
      </div>
    </div>
    <div class="flex flex-wrap items-center gap-1.5 px-2 pb-1.5">
      <label class="search">
        {#if searchingLines}<LoaderCircle size={12} class="shrink-0 animate-spin" />{:else}<Search size={12} class="shrink-0" />{/if}
        <input id="file-tree-search" placeholder={mode === "names" ? "Search file names" : mode === "lines" ? "Search lines" : "Search files and lines"}
          bind:value={query} onkeydown={(e) => e.key === "Escape" && (query = "")} />
        {#if query}<button id="file-tree-search-clear" title="Clear" onclick={() => (query = "")}><X size={12} /></button>{/if}
      </label>
      <Segmented id="file-tree-search-mode" value={mode} onchange={setMode}
        options={[{ value: "names", label: "Names", icon: CaseSensitive, title: "Search file names" }, { value: "lines", label: "Lines", icon: TextSearch, title: "Search lines" },
          { value: "both", label: "Both", icon: Layers, title: "Search file names and lines" }]} />
      {#if mode !== "names" && !model.showAllFiles}
        <Segmented id="file-tree-search-scope" value={diffOnly ? "diff" : "whole"} onchange={(v) => setDiffOnly(v === "diff")}
          options={[{ value: "diff", label: "Diff", icon: Diff, title: "Only the lines added or removed here" }, { value: "whole", label: "Whole file", icon: FileText, title: "Anywhere in the changed files" }]} />
      {/if}
    </div>
  </div>
  {#if searching && !rows.length && !searchingLines}<span class="px-3 py-2 text-[12px] text-muted-foreground">No matches</span>{/if}
  <div class="flex-1 overflow-auto px-1.5 pb-2" id="file-tree" role="tree" bind:clientHeight={viewHeight}
    onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
    <div style:height="{rows.length * ROW}px" style:padding-top="{first * ROW}px" class="box-border">
    {#each shown as { node, depth } (node.path)}
      {@const status = model.changes[node.path]}
      {@const isFolder = node.children != null}
      {@const isChanged = isFolder ? model.changedDirs.has(node.path) : status != null}
      {@const isOpen = model.expandedDirs.has(node.path)}
      {@const hits = searching ? lineHits[node.path] : undefined}
      <button class="file flex h-[26px] w-full items-center gap-1 rounded-md pr-2 text-left text-[12px]" class:active={model.fileFilter === node.path}
        style:padding-left="{depth * 16 + 4}px" style:--depth={depth} id="file-tree-row-{node.path}"
        onclick={() => (isFolder ? model.toggleDir(node.path) : model.showFile(node.path, mode !== "names" && hits ? { line: hits.line, old: hits.old } : undefined))}
        oncontextmenu={(e) => { e.preventDefault(); menu = { path: node.path, isFolder, deleted: status === "D", x: e.clientX, y: e.clientY }; }}>
        <span class="flex w-3 shrink-0 justify-center text-tertiary">
          {#if isFolder}<ChevronRight size={10} strokeWidth={3} class="transition-transform {isOpen ? 'rotate-90' : ''}" />{/if}
        </span>
        {#if model.isWorktreeSelected && isChanged}{@render checkbox(node.path)}{/if}
        {#if isFolder}<Folder size={13} class="shrink-0 text-tertiary" />{:else}<img src={fileIcon?.(node.path)} alt="" class="size-[14px] shrink-0" />{/if}
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
        {#if !isFolder && status && model.isWorktreeSelected}
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
  /* Wide: segments that carry an icon for the narrow layout show just their text. */
  .toolbar :global(.seg button:has(.label) svg) { display: none; }
  @container (max-width: 440px) {
    .toolbar :global(.label) { display: none; }
    .toolbar :global(.seg button:has(.label) svg) { display: block; }
  }
  .search { display: flex; flex: 1; min-width: 240px; align-items: center; gap: 6px; height: 26px; padding: 0 8px; border-radius: 7px; color: var(--tertiary); background: oklch(0.5 0 0 / 0.08); }
  .search input { flex: 1; min-width: 0; font-size: 12px; color: var(--foreground); background: none; outline: none; }
  .search button:hover { color: var(--foreground); }
  .hits { flex-shrink: 0; padding: 0 5px; border-radius: 99px; font-size: 10px; font-weight: 600; color: var(--primary); background: color-mix(in oklab, var(--primary) 16%, transparent); }
  .name-first { flex-shrink: 0; max-width: 75%; } /* flat list: the file name wins over its dimmed folder */
  /* Same look as DiscardButton's labeled state, so the actions on the right match. */
  .tool { display: flex; flex-shrink: 0; align-items: center; gap: 4px; height: 18px; padding: 0 7px; font-size: 10px; font-weight: 600; color: var(--muted-foreground); }
  .tool:hover { color: var(--foreground); }
  /* Only on hover, like the Swift row; stays while armed (✕ ✓) so the confirm isn't lost when the mouse moves. */
  .discard { display: none; }
  .file:hover .discard, .discard:has(:global([id$="-confirm"])) { display: contents; }
  .collapse-all { display: grid; place-items: center; width: 28px; height: 28px; border-radius: 8px; color: var(--muted-foreground); }
  .collapse-all:hover:not(:disabled) { color: var(--foreground); background: oklch(0.5 0 0 / 0.12); }
  .collapse-all:disabled { opacity: 0.35; }
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
