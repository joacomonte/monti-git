<!-- Files of the selected commit: only the changed ones, or the whole repo with changes highlighted. -->
<script lang="ts">
  import { ChevronRight, File, Folder } from "@lucide/svelte";
  import * as Tabs from "$lib/components/ui/tabs";
  import type { FileNode, RepoModel } from "$lib/model.svelte";
  import DiscardButton from "./DiscardButton.svelte";

  let { model }: { model: RepoModel } = $props();

  /** The tree flattened to the rows on screen (children of collapsed folders skipped). */
  const rows = $derived.by(() => {
    const out: { node: FileNode; depth: number }[] = [];
    const add = (nodes: FileNode[], depth: number) => {
      for (const node of nodes) {
        out.push({ node, depth });
        if (node.children && model.expandedDirs.has(node.path)) add(node.children, depth + 1);
      }
    };
    add(model.tree, 0);
    return out;
  });

  function toggleDir(path: string) {
    const dirs = new Set(model.expandedDirs);
    if (!dirs.delete(path)) dirs.add(path);
    model.expandedDirs = dirs;
  }

  // New files in subtle green, modified in subtle yellow; deletes red, renames blue.
  const COLORS: Record<string, string> = { A: "rgb(52 199 89 / 0.85)", M: "rgb(230 180 0 / 0.9)", D: "rgb(255 59 48 / 0.6)", R: "rgb(0 122 255 / 0.85)" };
  const LABELS: Record<string, string> = { A: "Added", M: "Modified", D: "Deleted", R: "Renamed" };
</script>

<div class="flex h-full flex-col">
  <div class="flex items-center justify-between p-2">
    <Tabs.Root value={model.showAllFiles ? "all" : "changed"} onValueChange={(v) => model.setShowAllFiles(v === "all")} id="file-tree-mode">
      <Tabs.List>
        <Tabs.Trigger value="changed" id="file-tree-changed">Changed</Tabs.Trigger>
        <Tabs.Trigger value="all" id="file-tree-all">All</Tabs.Trigger>
      </Tabs.List>
    </Tabs.Root>
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
        {#if isFolder}<Folder size={13} class="shrink-0 text-tertiary" />{:else}<File size={13} class="shrink-0 text-tertiary" />{/if}
        <span
          class="truncate"
          class:font-semibold={isChanged}
          class:line-through={status === "D"}
          style:color={status ? COLORS[status] : isChanged ? "var(--foreground)" : "var(--muted-foreground)"}
        >{node.name}</span>
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
