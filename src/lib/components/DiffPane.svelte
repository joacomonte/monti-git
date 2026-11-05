<!-- Bottom pane: one file at a time, before on the left, after on the right. -->
<script lang="ts">
  import { ChevronLeft, ChevronRight, FileSearch } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";
  import { api, WORKTREE_ID, type Blame, type Side } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import { ago } from "$lib/format";
  import DiscardButton from "./DiscardButton.svelte";

  let { model }: { model: RepoModel } = $props();
  const diff = $derived(model.diff?.path === model.fileFilter ? model.diff : null);
  const rows = $derived(diff?.rows ?? []);
  const index = $derived(model.fileFilter ? model.changedOrder.indexOf(model.fileFilter) : -1);
  const counts = $derived(model.fileFilter ? model.counts[model.fileFilter] : undefined);
  const single = $derived(rows.every((row) => !row.left)); // a file shown whole (or a new one): no "before" column

  // Blame of the file as of the selected commit, for the tooltip on each line of the "after" side. By id: a commit's
  // blame never changes, so only a new id re-blames; the uncommitted row re-blames on every reload.
  let blame = $state.raw<Blame | null>(null);
  $effect(() => {
    const repo = model.repo, path = model.fileFilter, id = model.selectedId;
    if (id === WORKTREE_ID) void model.history; // uncommitted: the file may have changed
    blame = null;
    if (!repo || !path || !id) return;
    let stale = false;
    api.blame(repo, path, id === WORKTREE_ID ? null : id).then((result) => { if (!stale) blame = result; }, () => {}); // deleted / binary: no blame
    return () => (stale = true);
  });
  const blameOf = (line: number) => blame?.commits[blame.lines[line - 1]];
  const uncommitted = (id: string) => /^0+$/.test(id);
  function blameTitle(line: number) {
    const commit = blameOf(line);
    if (!commit) return undefined;
    return uncommitted(commit.id) ? "Not committed yet" : `${commit.author} · ${ago(commit.date)} · ${commit.id.slice(0, 7)}\n${commit.subject}`;
  }

  // A search result: its row, scrolled into view and highlighted, with who wrote it.
  const jumpRow = $derived.by(() => {
    const jump = model.jump;
    if (!jump || jump.path !== diff?.path) return -1;
    return rows.findIndex((row) => row.hunk == null && (jump.old ? row.left : row.right)?.number === jump.line);
  });
  $effect(() => {
    const row = jumpRow;
    if (row >= 0) requestAnimationFrame(() => document.getElementById(`diff-row-${row}`)?.scrollIntoView({ block: "center" }));
  });
  const jumpBlame = $derived(jumpRow >= 0 && model.jump && !model.jump.old ? blameOf(model.jump.line) : undefined);

  /** ⌥← / ⌥→: previous/next changed file. */
  function keydown(e: KeyboardEvent) {
    if (!e.altKey || (e.key !== "ArrowLeft" && e.key !== "ArrowRight")) return;
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return; // word jumps while typing
    e.preventDefault();
    model.stepFile(e.key === "ArrowLeft" ? -1 : 1);
  }

  const parts = (side: Side) =>
    side.changed && side.changed[1] > side.changed[0]
      ? [side.text.slice(0, side.changed[0]), side.text.slice(...side.changed), side.text.slice(side.changed[1])]
      : [side.text, "", ""];
</script>

<svelte:window onkeydown={keydown} />

<div class="pane flex h-full flex-col">
  <div class="flex flex-col gap-[3px] border-b px-2.5 py-[7px]">
    <div class="flex items-center gap-2 text-[11px] font-medium tabular-nums">
      {#if model.fileFilter}
        <span class="truncate text-[12px] font-semibold" style="direction: rtl; text-align: left">{model.fileFilter}</span>
        {#if counts}
          <span class="text-green-500/70">+{counts[0]}</span>
          <span class="text-red-500/70">−{counts[1]}</span>
        {/if}
      {/if}
      <span class="flex-1"></span>
      {#if index >= 0}<span class="text-muted-foreground">{index + 1} / {model.changedOrder.length}</span>{/if}
      <Button variant="ghost" size="icon-xs" disabled={index <= 0} onclick={() => model.stepFile(-1)} title="Previous file (⌥←)" id="diff-previous-file"><ChevronLeft /></Button>
      <Button variant="ghost" size="icon-xs" disabled={index < 0 || index >= model.changedOrder.length - 1} onclick={() => model.stepFile(1)} title="Next file (⌥→)" id="diff-next-file"><ChevronRight /></Button>
    </div>
    <!-- Subject of the commit behind this file's change. -->
    {#if model.selected && model.selected.id !== WORKTREE_ID}
      <div class="flex gap-1.5">
        <span class="font-mono text-[11px] text-tertiary">{model.selected.id.slice(0, 7)}</span>
        <span class="truncate text-[12px] text-muted-foreground" title={model.selected.subject}>{model.selected.subject}</span>
      </div>
    {/if}
  </div>
  {#if rows.length === 0}
    <div class="flex flex-1 flex-col items-center justify-center gap-2 text-muted-foreground">
      <FileSearch size={28} strokeWidth={1.5} />
      <span class="text-[13px] font-semibold">{model.diffMessage}</span>
    </div>
  {:else}
    {#key model.fileFilter}
    <div class="enter-fade flex-1 overflow-auto font-mono text-[12px] select-text" id="diff-rows">
      {#each rows as row, r (r)}
        {#if row.hunk != null}
          <div class="flex items-center gap-2 bg-quinary px-2 py-[3px] text-[11px] text-tertiary">
            <span class="min-w-0 flex-1 truncate">{row.header}</span>
            {#if model.isWorktreeSelected && row.header}
              <DiscardButton title="Discard block" id="discard-block-{row.hunk}" onconfirm={() => model.discardHunk(row.hunk!, row.header!)}
                hint="Discard this block: throws away only this chunk of changes. Ej: you added a debug log here by mistake." />
            {/if}
          </div>
        {:else}
          <div class="line flex" id="diff-row-{r}" class:jumped={r === jumpRow}>
            {#each single ? [row.right] : [row.left, row.right] as side, i}
              {@const after = single || i === 1}
              {@const tint = after ? "var(--add)" : "var(--del)"}
              <!-- Blame on hover only: computing it for every row on every render would be wasted work. -->
              <div class="flex flex-1 py-px" class:border-r={!after} role="presentation"
                onmouseenter={(e) => { if (after && side) e.currentTarget.title = blameTitle(side.number) ?? ""; }}
                style:background={row.isChange ? (side ? `color-mix(in srgb, ${tint} 12%, transparent)` : "var(--quinary)") : undefined}>
                <span class="w-11 shrink-0 pr-2 text-right text-tertiary">{side?.number ?? ""}</span>
                {#if side}
                  {@const [before, changed, rest] = parts(side)}
                  <span class="min-w-0 flex-1 whitespace-pre-wrap break-all">{before}<mark style:background="color-mix(in srgb, {tint} 35%, transparent)">{changed}</mark>{rest}</span>
                  {#if r === jumpRow && after && jumpBlame}
                    <!-- Who wrote the searched line: hash (opens it in the graph), author, when. -->
                    <span class="blame" title={jumpBlame.subject}>
                      {#if uncommitted(jumpBlame.id)}Not committed yet{:else}
                        <button class="hash" id="diff-jump-blame-commit" title="Show {jumpBlame.id.slice(0, 7)} in the graph: {jumpBlame.subject}"
                          onclick={() => model.revealCommit(model.repo!, jumpBlame.id)}>{jumpBlame.id.slice(0, 7)}</button>
                        · {jumpBlame.author} · {ago(jumpBlame.date)}{/if}
                    </span>
                  {/if}
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      {/each}
      {#if diff?.truncated}
        <div class="px-3 py-2 font-sans text-[11.5px] text-tertiary" id="diff-truncated">Only the first {rows.length} lines are shown: the file is too long for the pane.</div>
      {/if}
    </div>
    {/key}
  {/if}
</div>

<style>
  .pane { --add: rgb(52 199 89); --del: rgb(255 59 48); }
  mark { color: inherit; }
  .jumped { background: color-mix(in oklab, var(--primary) 16%, transparent); box-shadow: inset 2px 0 0 var(--primary); }
  .blame { flex-shrink: 0; align-self: flex-start; padding: 0 8px; font-family: var(--font-sans, system-ui); font-size: 11px; color: var(--muted-foreground); white-space: nowrap; }
  .hash { font-family: ui-monospace, monospace; color: var(--primary); }
  .hash:hover { text-decoration: underline; }
  .line { content-visibility: auto; contain-intrinsic-size: auto 18px; } /* long diffs: only rows on screen get laid out */
</style>
