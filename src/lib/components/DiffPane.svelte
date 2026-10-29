<!-- Bottom pane: one file at a time, before on the left, after on the right. -->
<script lang="ts">
  import { ChevronLeft, ChevronRight, FileSearch } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";
  import type { Side } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import DiscardButton from "./DiscardButton.svelte";

  let { model }: { model: RepoModel } = $props();
  const index = $derived(model.fileFilter ? model.changedOrder.indexOf(model.fileFilter) : -1);
  const counts = $derived(model.fileFilter ? model.counts[model.fileFilter] : undefined);
  const single = $derived(model.diffRows.every((row) => !row.left)); // a file shown whole (or a new one): no "before" column

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

<div class="flex h-full flex-col">
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
    <!-- Subject of the commit(s) behind this file's change; a group lists only the ones that touched it. -->
    {#each model.fileCommits.slice(0, 5) as commit (commit.id)}
      <div class="flex gap-1.5">
        <span class="font-mono text-[11px] text-tertiary">{commit.id.slice(0, 7)}</span>
        <span class="truncate text-[12px] text-muted-foreground" title={commit.subject}>{commit.subject}</span>
      </div>
    {/each}
    {#if model.fileCommits.length > 5}<span class="text-[11px] text-tertiary">+{model.fileCommits.length - 5} more commits</span>{/if}
  </div>
  {#if model.diffRows.length === 0}
    <div class="flex flex-1 flex-col items-center justify-center gap-2 text-muted-foreground">
      <FileSearch size={28} strokeWidth={1.5} />
      <span class="text-[13px] font-semibold">{model.diffMessage}</span>
    </div>
  {:else}
    {#key model.fileFilter}
    <div class="enter-fade flex-1 overflow-auto font-mono text-[12px] select-text" id="diff-rows">
      {#each model.diffRows as row}
        {#if row.hunk != null}
          <div class="flex items-center gap-2 bg-quinary px-2 py-[3px] text-[11px] text-tertiary">
            <span class="min-w-0 flex-1 truncate">{row.right?.text}</span>
            {#if model.isWorktreeSelected}
              <DiscardButton title="Discard block" id="discard-block-{row.hunk}" onconfirm={() => model.discardHunk(row.hunk!)}
                hint="Discard this block: throws away only this chunk of changes. Ej: you added a debug log here by mistake." />
            {/if}
          </div>
        {:else}
          <div class="line flex">
            {#each single ? [row.right] : [row.left, row.right] as side, i}
              {@const i2 = single ? 1 : i} <!-- 1 = the "after" side -->
              {@const tint = i2 === 0 ? "var(--del)" : "var(--add)"}
              <div class="flex flex-1 py-px" class:border-r={i2 === 0}
                style:background={row.isChange ? (side ? `color-mix(in srgb, ${tint} 12%, transparent)` : "var(--quinary)") : undefined}>
                <span class="w-11 shrink-0 pr-2 text-right text-tertiary">{side?.number ?? ""}</span>
                {#if side}
                  {@const [before, changed, after] = parts(side)}
                  <span class="min-w-0 flex-1 whitespace-pre-wrap break-all">{before}<mark style:background="color-mix(in srgb, {tint} 35%, transparent)">{changed}</mark>{after}</span>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      {/each}
    </div>
    {/key}
  {/if}
</div>

<style>
  div { --add: rgb(52 199 89); --del: rgb(255 59 48); }
  mark { color: inherit; }
  .line { content-visibility: auto; contain-intrinsic-size: auto 18px; } /* long diffs: only rows on screen get laid out */
</style>
