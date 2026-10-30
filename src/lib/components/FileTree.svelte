<!-- Files of the selected commit: only the changed ones, or the whole repo with changes highlighted. -->
<script lang="ts">
  import { fade } from "svelte/transition";
  import { Check, ChevronRight, ChevronsDownUp, Copy, Folder, FolderOpen, History, List, ListTree, SquareArrowOutUpRight, Sparkles, Square, SquareCheck, SquareMinus } from "@lucide/svelte";
  import { api, WORKTREE_ID } from "$lib/api";
  import { fileIcon } from "$lib/fileIcon";
  import { Switch } from "$lib/components/ui/switch";
  import * as Tabs from "$lib/components/ui/tabs";
  import { prefs, type FileNode, type RepoModel } from "$lib/model.svelte";
  import CommitBox from "./CommitBox.svelte";
  import CommitDetails from "./CommitDetails.svelte";
  import DiscardButton from "./DiscardButton.svelte";

  let { model }: { model: RepoModel } = $props();
  let flat = $state(prefs.get("treeFlat", false)); // one row per file with its folder dimmed, instead of nested folders
  let copiedPath = $state<string | null>(null);
  // Virtualized: only the rows in view (plus a margin) are in the DOM, so the whole project scrolls smoothly.
  const ROW = 26; // matches the row's h-[26px]
  let scrollTop = $state(0);
  let viewHeight = $state(0);
  let menu = $state<{ path: string; isFolder: boolean; deleted: boolean; x: number; y: number } | null>(null);

  /** The tree flattened to the rows on screen (children of collapsed folders skipped; flat: every file, no folders). */
  const rows = $derived.by(() => {
    const out: { node: FileNode; depth: number }[] = [];
    const add = (nodes: FileNode[], depth: number) => {
      for (const node of nodes) {
        if (flat && node.children) add(node.children, 0);
        else out.push({ node, depth });
        if (!flat && node.children && model.expandedDirs.has(node.path)) add(node.children, depth + 1);
      }
    };
    add(model.tree, 0);
    return out;
  });

  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 10));
  const shown = $derived(rows.slice(first, first + Math.ceil(viewHeight / ROW) + 20));

  function setFlat(value: boolean) {
    flat = value;
    prefs.set("treeFlat", value);
  }

  async function copyPath(path: string) {
    await navigator.clipboard.writeText(`${model.repo}/${path}`);
    copiedPath = path;
    setTimeout(() => { if (copiedPath === path) copiedPath = null; }, 1200);
  }

  /** Repo, checked-out branch, commit and file in one block, to paste into an AI agent so it looks at exactly this. */
  function copyForAI(path: string, isFolder: boolean) {
    const head = model.commits.find((c) => c.refs.some((r) => r === "HEAD" || r.startsWith("HEAD -> ")));
    const branch = head?.refs.find((r) => r.startsWith("HEAD -> "))?.slice(8) ?? "(detached HEAD)";
    const newest = model.selected ? model.commits[model.selected[0]] : null;
    const commit = !newest || newest.id === WORKTREE_ID
      ? `${head?.id ?? "HEAD"} + uncommitted changes in the working tree`
      : `${newest.id} "${newest.subject}"`;
    navigator.clipboard.writeText(
      [`Repo: ${model.repo}`, `Branch: ${branch} (checked out)`, `Commit: ${commit}`, `${isFolder ? "Folder" : "File"}: ${model.repo}/${path}`].join("\n"),
    );
  }

  /** Moves the node to <body>: a `fixed` menu inside the pane gets clipped and squeezed to the pane's width. */
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  const EXPLORER = navigator.userAgent.includes("Windows") ? "Explorer" : "Finder";

  /** Opens with the system's default app, or `reveal`s it selected in Finder / Explorer. */
  async function openFile(path: string, reveal: boolean) {
    try {
      await api.openFile(model.repo!, path, reveal);
    } catch (error) {
      model.error = String(error);
    }
  }

  function toggleDir(path: string) {
    const dirs = new Set(model.expandedDirs);
    if (!dirs.delete(path)) dirs.add(path);
    model.expandedDirs = dirs;
  }

  /** Changed files at or under `path`, and whether all / some of them are staged (the checkbox state). */
  function stageState(path: string) {
    const files = Object.keys(model.changes).filter((p) => !path || p === path || p.startsWith(path + "/"));
    const staged = files.filter((p) => model.staged.has(p)).length;
    return { files, all: files.length > 0 && staged === files.length, some: staged > 0 };
  }
  const everything = $derived(stageState(""));

  // New files in subtle green, modified in subtle yellow; deletes red, renames blue.
  const COLORS: Record<string, string> = { A: "rgb(52 199 89 / 0.85)", M: "rgb(230 180 0 / 0.9)", D: "rgb(255 59 48 / 0.6)", R: "rgb(0 122 255 / 0.85)" };
</script>

{#snippet checkbox(path: string, all: boolean, some: boolean, files: string[])}
  <!-- span, not button: rows are <button>s. -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <span role="checkbox" aria-checked={all ? "true" : some ? "mixed" : "false"} tabindex="0" id="stage-{path || 'all'}"
    class="flex shrink-0 {some ? 'text-primary' : 'text-tertiary hover:text-muted-foreground'}"
    title={all ? "Staged: goes into the next commit. Click to leave it out." : "Not staged. Click to include it in the next commit."}
    onclick={(e) => { e.stopPropagation(); model.setStaged(files, !all); }}>
    {#if all}<SquareCheck size={13} />{:else if some}<SquareMinus size={13} />{:else}<Square size={13} />{/if}
  </span>
{/snippet}

<div class="flex h-full flex-col">
  {#if model.isWorktreeSelected}<CommitBox {model} />
  {:else if model.selected}{#key model.commits[model.selected[0]].id}<CommitDetails {model} id={model.commits[model.selected[0]].id} />{/key}{/if}
  <div class="flex items-center gap-1.5 p-2">
    <!-- View controls on the left (layout, collapse, what to list); actions on the right. -->
    <Tabs.Root value={flat ? "flat" : "tree"} onValueChange={(v) => setFlat(v === "flat")} id="file-tree-layout">
      <Tabs.List>
        <Tabs.Trigger value="tree" id="file-tree-layout-tree" title="Folder tree"><ListTree /></Tabs.Trigger>
        <Tabs.Trigger value="flat" id="file-tree-layout-flat" title="Flat list: one row per file, its folder dimmed"><List /></Tabs.Trigger>
      </Tabs.List>
    </Tabs.Root>
    {#if !flat}
      <button class="collapse-all" id="file-tree-collapse-all" title="Collapse all folders" disabled={!model.expandedDirs.size}
        onclick={() => (model.expandedDirs = new Set())}><ChevronsDownUp size={14} /></button>
    {/if}
    <label class="ml-1 flex items-center gap-1.5 text-[11px] font-medium whitespace-nowrap text-muted-foreground"
      title="On: only the files that changed. Off: every file in the repo, changed ones marked">
      <Switch checked={!model.showAllFiles} onCheckedChange={(on) => model.setShowAllFiles(!on)} id="file-tree-changes-only" />Show changes only
    </label>
    <span class="flex-1"></span>
    {#if model.isWorktreeSelected && everything.files.length}
      <button class="tool" id="stage-all" onclick={() => model.setStaged(everything.files, !everything.all)}
        title={everything.all ? "Unstage all: nothing goes into the next commit" : "Stage all: every changed file goes into the next commit"}>
        {#if everything.all}<SquareMinus size={10} strokeWidth={2.5} />Unstage all{:else}<SquareCheck size={10} strokeWidth={2.5} />Stage all{/if}
      </button>
    {/if}
    {#if model.isWorktreeSelected && model.changedOrder.length}
      <DiscardButton title="Discard all" label id="discard-all" onconfirm={() => model.discardAll()}
        hint="Discard all: throws away every uncommitted change, back to the last commit. New files get deleted too." />
    {/if}
  </div>
  <div class="flex-1 overflow-auto px-1.5 pb-2" id="file-tree" role="tree" bind:clientHeight={viewHeight}
    onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
    <div style:height="{rows.length * ROW}px" style:padding-top="{first * ROW}px" class="box-border">
    {#each shown as { node, depth } (node.path)}
      {@const status = model.changes[node.path]}
      {@const isFolder = node.children != null}
      {@const isChanged = isFolder ? model.changedDirs.has(node.path) : status != null}
      {@const isOpen = model.expandedDirs.has(node.path)}
      <button
        class="file flex h-[26px] w-full items-center gap-1 rounded-md pr-2 text-left text-[12px]"
        class:active={model.fileFilter === node.path}
        style:padding-left="{depth * 16 + 4}px"
        style:--depth={depth}
        id="file-tree-row-{node.path}"
        onclick={() => (isFolder ? toggleDir(node.path) : model.filter(node.path))}
        oncontextmenu={(e) => { e.preventDefault(); menu = { path: node.path, isFolder, deleted: status === "D", x: e.clientX, y: e.clientY }; }}
      >
        <span
          class="flex w-3 shrink-0 justify-center text-tertiary"
        >
          {#if isFolder}<ChevronRight size={10} strokeWidth={3} class="transition-transform {isOpen ? 'rotate-90' : ''}" />{/if}
        </span>
        {#if model.isWorktreeSelected && isChanged}
          {@const s = stageState(node.path)}
          {@render checkbox(node.path, s.all, s.some, s.files)}
        {/if}
        {#if isFolder}<Folder size={13} class="shrink-0 text-tertiary" />{:else}<img src={fileIcon(node.path)} alt="" class="size-[14px] shrink-0" />{/if}
        <span
          class="truncate"
          class:name-first={flat}
          class:font-semibold={isChanged}
          class:line-through={status === "D"}
          style:color={status ? COLORS[status] : isChanged ? "var(--foreground)" : "var(--muted-foreground)"}
        >{flat ? node.path.split("/").pop() : node.name}</span>
        {#if flat && node.path.includes("/")}<span class="truncate text-[11px] text-tertiary">{node.path.slice(0, node.path.lastIndexOf("/"))}</span>{/if}
        <span class="flex-1"></span>
        <!-- Hover tools; spans, not buttons, since the row is a <button>. -->
        <span class="tools">
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <span role="button" tabindex="-1" class="tool-icon" id="file-copy-{node.path}" title="Copy full path"
            onclick={(e) => { e.stopPropagation(); copyPath(node.path); }}>
            {#if copiedPath === node.path}<Check size={12} class="text-green-500" />{:else}<Copy size={12} />{/if}
          </span>
          {#if status !== "D"}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <span role="button" tabindex="-1" class="tool-icon" id="file-reveal-{node.path}" title="Show in {EXPLORER}"
              onclick={(e) => { e.stopPropagation(); openFile(node.path, true); }}><FolderOpen size={12} /></span>
            {#if !isFolder}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <span role="button" tabindex="-1" class="tool-icon" id="file-open-{node.path}" title="Open with the default app"
                onclick={(e) => { e.stopPropagation(); openFile(node.path, false); }}><SquareArrowOutUpRight size={12} /></span>
            {/if}
          {/if}
        </span>
        {#if !isFolder && status && model.isWorktreeSelected}
          <span class="discard"><DiscardButton title="Discard file" id="discard-file-{node.path}" onconfirm={() => model.discardFile(node.path)}
            hint="Discard this file: throws away your changes to it, back to the last commit. Ej: you edited it by mistake." /></span>
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

<svelte:window onmousedown={() => (menu = null)} onkeydown={(e) => e.key === "Escape" && (menu = null)} onwheel={() => (menu = null)} onblur={() => (menu = null)} />
{#if menu}
  {@const m = menu}
  <!-- Right-click menu: same actions as the hover icons. Any click, Escape or scroll closes it. -->
  <div use:portal class="context-menu enter-pop" out:fade={{ duration: 120 }} role="menu" tabindex="-1" id="file-tree-menu" style:left="{Math.min(m.x, innerWidth - 280)}px" style:top="{Math.min(m.y, innerHeight - 140)}px"
    onmousedown={(e) => e.stopPropagation()}>
    <button role="menuitem" class="menu-item" id="file-menu-copy" onclick={() => { copyPath(m.path); menu = null; }}>
      <Copy size={13} />Copy full path
    </button>
    <button role="menuitem" class="menu-item" id="file-menu-copy-ai" onclick={() => { copyForAI(m.path, m.isFolder); menu = null; }}
      title="Repo, branch, commit hash and path in one block, ready to paste into your AI agent">
      <Sparkles size={13} />Copy for AI<span class="menu-hint">repo · branch · hash</span>
    </button>
    {#if !m.deleted}
      <button role="menuitem" class="menu-item" id="file-menu-history" onclick={() => { model.setHistory(m.path); menu = null; }}
      title="The graph shows only the commits that touched it">
      <History size={13} />History
    </button>
    <button role="menuitem" class="menu-item" id="file-menu-reveal" onclick={() => { openFile(m.path, true); menu = null; }}>
        <FolderOpen size={13} />Show in {EXPLORER}
      </button>
      {#if !m.isFolder}
        <button role="menuitem" class="menu-item" id="file-menu-open" onclick={() => { openFile(m.path, false); menu = null; }}>
          <SquareArrowOutUpRight size={13} />Open with default app
        </button>
      {/if}
    {/if}
  </div>
{/if}

<style>
  .file:hover { background: var(--quinary); }
  .name-first { flex-shrink: 0; max-width: 75%; } /* flat list: the file name wins over its dimmed folder */
  /* Same look as DiscardButton's labeled state, so the actions on the right match. */
  .tool { display: flex; flex-shrink: 0; align-items: center; gap: 4px; height: 18px; padding: 0 7px; font-size: 10px; font-weight: 600; color: var(--muted-foreground); }
  .tool:hover { color: var(--foreground); }
  /* Only on hover, like the Swift row; stays while armed (✕ ✓) so the confirm isn't lost when the mouse moves. */
  .discard { display: none; }
  .file:hover .discard, .discard:has(:global([id$="-confirm"])) { display: contents; }
  /* Copy / open: hidden until hover, then fade in; the icon itself brightens under the pointer. */
  .collapse-all { display: grid; place-items: center; width: 28px; height: 28px; border-radius: 8px; color: var(--muted-foreground); }
  .collapse-all:hover:not(:disabled) { color: var(--foreground); background: oklch(0.5 0 0 / 0.12); }
  .collapse-all:disabled { opacity: 0.35; }
  .tools { display: flex; flex-shrink: 0; gap: 1px; opacity: 0; width: 0; overflow: hidden; transition: opacity 0.15s; }
  .file:hover .tools { opacity: 1; width: auto; }
  .tool-icon { display: grid; place-items: center; width: 20px; height: 20px; border-radius: 5px; color: var(--tertiary); }
  .tool-icon:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.14); }
  .context-menu {
    position: fixed; z-index: 1000; width: max-content; min-width: 200px; padding: 4px; border-radius: 10px;
    background: var(--popover); color: var(--popover-foreground);
    box-shadow: 0 0 0 1px oklch(0.5 0 0 / 0.18), 0 8px 24px rgb(0 0 0 / 0.3);
  }
  .menu-item { display: flex; width: 100%; align-items: center; gap: 8px; height: 28px; padding: 0 10px; border-radius: 6px; font-size: 12px; text-align: left; white-space: nowrap; }
  .menu-hint { margin-left: auto; padding-left: 16px; font-size: 11px; color: var(--tertiary); }
  .menu-item:hover { background: oklch(0.5 0 0 / 0.12); }
  .file.active { background: color-mix(in oklch, var(--primary) 18%, transparent); }
  /* One faint vertical guide per ancestor level, centered under that level's chevron. */
  .file {
    background-image: repeating-linear-gradient(to right, var(--border) 0 1px, transparent 1px 16px);
    background-size: calc(var(--depth) * 16px) 100%;
    background-position: 9.5px 0;
    background-repeat: no-repeat;
  }
</style>
