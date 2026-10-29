<!-- Files of the selected commit: only the changed ones, or the whole repo with changes highlighted. -->
<script lang="ts">
  import { ChevronRight, Folder, List, ListTree, Square, SquareCheck, SquareMinus } from "@lucide/svelte";
  import { fileIcon } from "$lib/fileIcon";
  import * as Tabs from "$lib/components/ui/tabs";
  import { prefs, type FileNode, type RepoModel } from "$lib/model.svelte";
  import CommitBox from "./CommitBox.svelte";
  import DiscardButton from "./DiscardButton.svelte";

  let { model }: { model: RepoModel } = $props();
  let flat = $state(prefs.get("treeFlat", false)); // one row per file with its folder dimmed, instead of nested folders

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

  function setFlat(value: boolean) {
    flat = value;
    prefs.set("treeFlat", value);
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
  const LABELS: Record<string, string> = { A: "Added", M: "Modified", D: "Deleted", R: "Renamed" };
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
  {#if model.isWorktreeSelected}<CommitBox {model} />{/if}
  <div class="flex items-center gap-1.5 p-2">
    <Tabs.Root value={model.showAllFiles ? "all" : "changed"} onValueChange={(v) => model.setShowAllFiles(v === "all")} id="file-tree-mode">
      <Tabs.List>
        <Tabs.Trigger value="changed" id="file-tree-changed" title="Only the files that changed">Changed</Tabs.Trigger>
        <Tabs.Trigger value="all" id="file-tree-all" title="Every file in the repo, changed ones marked">All</Tabs.Trigger>
      </Tabs.List>
    </Tabs.Root>
    <!-- View controls together on the left (what to list, how to lay it out); actions on the right. -->
    <Tabs.Root value={flat ? "flat" : "tree"} onValueChange={(v) => setFlat(v === "flat")} id="file-tree-layout">
      <Tabs.List>
        <Tabs.Trigger value="tree" id="file-tree-layout-tree" title="Folder tree"><ListTree /></Tabs.Trigger>
        <Tabs.Trigger value="flat" id="file-tree-layout-flat" title="Flat list: one row per file, its folder dimmed"><List /></Tabs.Trigger>
      </Tabs.List>
    </Tabs.Root>
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
  <div class="flex-1 overflow-auto px-1.5 pb-2" id="file-tree" role="tree">
    {#each rows as { node, depth } (node.path)}
      {@const status = model.changes[node.path]}
      {@const isFolder = node.children != null}
      {@const isChanged = isFolder ? model.changedDirs.has(node.path) : status != null}
      {@const isOpen = model.expandedDirs.has(node.path)}
      <button
        class="file flex h-[26px] w-full items-center gap-1 rounded-md pr-2 text-left text-[12px]"
        class:active={model.fileFilter === node.path}
        style:padding-left="{depth * 16 + 4}px"
        style:--depth={depth}
        title={status ? `${LABELS[status] ?? status}: ${node.path}` : node.path}
        id="file-tree-row-{node.path}"
        onclick={() => (isFolder && !isChanged ? toggleDir(node.path) : model.filter(node.path))}
      >
        <span
          class="flex w-3 shrink-0 justify-center text-tertiary"
          role="presentation"
          onclick={(e) => { if (isFolder) { e.stopPropagation(); toggleDir(node.path); } }}
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

<style>
  .file:hover { background: var(--quinary); }
  .name-first { flex-shrink: 0; max-width: 75%; } /* flat list: the file name wins over its dimmed folder */
  /* Same look as DiscardButton's labeled state, so the actions on the right match. */
  .tool { display: flex; flex-shrink: 0; align-items: center; gap: 4px; height: 18px; padding: 0 7px; font-size: 10px; font-weight: 600; color: var(--muted-foreground); }
  .tool:hover { color: var(--foreground); }
  /* Only on hover, like the Swift row; stays while armed (✕ ✓) so the confirm isn't lost when the mouse moves. */
  .discard { display: none; }
  .file:hover .discard, .discard:has(:global([id$="-confirm"])) { display: contents; }
  .file.active { background: color-mix(in oklch, var(--primary) 18%, transparent); }
  /* One faint vertical guide per ancestor level, centered under that level's chevron. */
  .file {
    background-image: repeating-linear-gradient(to right, var(--border) 0 1px, transparent 1px 16px);
    background-size: calc(var(--depth) * 16px) 100%;
    background-position: 9.5px 0;
    background-repeat: no-repeat;
  }
</style>
