<!-- Virtualized commit list: only the rows on screen are in the DOM. Rows touch so the graph lines stay continuous. -->
<script lang="ts">
  import { WORKTREE_ID, type Commit, type Row } from "$lib/api";
  import type { Range, RepoModel } from "$lib/model.svelte";
  import GraphCell, { LANE, PAD } from "./GraphCell.svelte";
  import RefBadge from "./RefBadge.svelte";

  let { model }: { model: RepoModel } = $props();

  const ROW = 62;
  const QUIET = 26; // merge without refs: a short single-line row, so real commits lead
  const isQuietMerge = (c: Commit) => c.parents.length > 1 && c.refs.length === 0;

  let scrollTop = $state(0);
  let viewHeight = $state(600);
  let scroller: HTMLDivElement;

  const width = $derived(model.graphLanes * LANE + PAD * 2);
  const heightOf = (r: Range) => (r[0] === r[1] && isQuietMerge(model.commits[r[0]]) ? QUIET : ROW);
  const offsets = $derived.by(() => {
    const out = new Float64Array(model.visible.length + 1);
    model.visible.forEach((r, i) => (out[i + 1] = out[i] + heightOf(r)));
    return out;
  });

  /** First row whose bottom is below `y`. */
  function rowAt(y: number) {
    let lo = 0, hi = model.visible.length;
    while (lo < hi) {
      const m = (lo + hi) >> 1;
      if (offsets[m + 1] <= y) lo = m + 1;
      else hi = m;
    }
    return lo;
  }
  const start = $derived(Math.max(0, rowAt(scrollTop) - 5));
  const end = $derived(Math.min(model.visible.length, rowAt(scrollTop + viewHeight) + 6));

  const highlight = $derived(new Set(model.commits.flatMap((c, i) => (model.highlightChain.has(c.id) ? [i] : []))));

  // Reveal request from the dashboard: bring that row to the upper third, once.
  $effect(() => {
    const request = model.scrollRow;
    if (!request || !scroller) return;
    scroller.scrollTop = Math.max(0, offsets[request.row] - viewHeight / 3);
    model.scrollRow = null;
  });

  // Last rows about to show: load the next page.
  $effect(() => {
    if (end >= model.visible.length - 30) model.loadMore();
  });

  /** A collapsed group is drawn as one commit: lanes entering the newest, leaving the oldest. */
  function rowFor([first, last]: Range): Row {
    if (first === last) return model.rows[first];
    const top = model.rows[first], bottom = model.rows[last];
    return { ...top, after: bottom.after, parents: bottom.parents, isCurrent: bottom.isCurrent };
  }

  /** One badge per branch: a remote on the same commit as its local branch folds into the local badge. */
  function badges(refs: string[]) {
    const locals = new Set(refs.map((r) => r.replace("HEAD -> ", "")));
    const remote = (r: string) => (/^(origin|upstream)\//.test(r) ? r.slice(r.indexOf("/") + 1) : null);
    const folded = new Set(refs.map(remote).filter((n): n is string => n != null && locals.has(n)));
    return refs.flatMap((ref) => {
      const name = remote(ref);
      if (name != null) return name === "HEAD" || folded.has(name) ? [] : [{ ref, isSynced: false }];
      return [{ ref, isSynced: folded.has(ref.replace("HEAD -> ", "")) }];
    });
  }

  const relative = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
  /** "5 hours ago" for the last week, a plain date after that. */
  function when(seconds: number) {
    const diff = seconds - Date.now() / 1000;
    if (diff > -7 * 86400) {
      for (const [unit, size] of [["day", 86400], ["hour", 3600], ["minute", 60]] as const) {
        if (-diff >= size) return relative.format(Math.round(diff / size), unit);
      }
      return relative.format(0, "second");
    }
    const date = new Date(seconds * 1000);
    const sameYear = date.getFullYear() === new Date().getFullYear();
    return date.toLocaleDateString(undefined, { day: "numeric", month: "short", year: sameYear ? undefined : "numeric" });
  }

  const isSelected = (r: Range) => model.selected?.[0] === r[0] && model.selected?.[1] === r[1];

  /** Clicking the already-selected row deselects it, which hides the files and diff panes. */
  const click = (r: Range) => model.select(isSelected(r) ? null : r);

  function scrollToRow(i: number) {
    if (offsets[i] < scroller.scrollTop) scroller.scrollTop = offsets[i];
    else if (offsets[i + 1] > scroller.scrollTop + viewHeight) scroller.scrollTop = offsets[i + 1] - viewHeight;
  }

  /** ↑/↓ move the selection; → expands the selected group, ← collapses the group the selected commit belongs to. */
  function keydown(e: KeyboardEvent) {
    const i = model.visible.findIndex(isSelected);
    if (i < 0) return;
    const range = model.visible[i];
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      const next = model.visible[i + (e.key === "ArrowDown" ? 1 : -1)];
      if (next) {
        model.select(next);
        scrollToRow(i + (e.key === "ArrowDown" ? 1 : -1));
      }
    } else if (e.key === "ArrowRight" && !e.altKey) {
      if (range[1] > range[0]) model.toggle(range[0]);
    } else if (e.key === "ArrowLeft" && !e.altKey) {
      const group = [...model.runs].find(([first, last]) => first <= range[0] && range[0] <= last);
      if (range[0] === range[1] && group) model.toggle(group[0]);
    } else return;
    e.preventDefault();
  }
</script>

<div
  bind:this={scroller}
  bind:clientHeight={viewHeight}
  onscroll={() => (scrollTop = scroller.scrollTop)}
  onkeydown={keydown}
  tabindex="0"
  role="listbox"
  id="commit-table"
  class="relative h-full overflow-auto outline-none"
>
  <div style:height="{offsets[model.visible.length]}px" class="relative">
    {#each model.visible.slice(start, end) as range, k (model.commits[range[0]].id)}
      {@const commit = model.commits[range[0]]}
      {@const groupSize = model.runs.has(range[0]) ? model.runs.get(range[0])! - range[0] + 1 : null}
      {@const quiet = range[0] === range[1] && isQuietMerge(commit)}
      <div
        class="row absolute inset-x-0 flex"
        class:selected={isSelected(range)}
        style:top="{offsets[start + k]}px"
        style:height="{heightOf(range)}px"
        onclick={() => click(range)}
        role="option"
        aria-selected={isSelected(range)}
        tabindex="-1"
        onkeydown={() => {}}
      >
        <div class="shrink-0" style:width="{width}px">
          <GraphCell index={range[0]} row={rowFor(range)} height={heightOf(range)} {width} isGroup={range[1] > range[0]}
            isWorktree={commit.id === WORKTREE_ID} isMerge={commit.parents.length > 1} names={model.names} {highlight} />
        </div>
        {#if quiet}
          <div class="flex min-w-0 items-center px-2 text-[11px] text-tertiary opacity-70"><span class="truncate">{commit.subject}</span></div>
        {:else}
          <div class="flex min-w-0 flex-1 flex-col justify-center gap-[5px] px-2" class:opacity-55={commit.parents.length > 1}>
            {#if commit.id === WORKTREE_ID}
              <div class="text-[13px] font-semibold">Uncommitted changes</div>
            {:else}
              <!-- Left to right: commit count, branches, then author/date (least relevant). -->
              <div class="flex min-w-0 items-center gap-1.5 overflow-hidden whitespace-nowrap">
                {#if groupSize}
                  <button
                    class="pill shrink-0"
                    class:dim={range[1] === range[0]}
                    id="group-toggle-{commit.id}"
                    title={range[1] > range[0] ? `Show ${groupSize - 1} more commits by ${commit.author} (→)` : `Collapse ${groupSize} commits (←)`}
                    onclick={(e) => { e.stopPropagation(); model.toggle(range[0]); }}
                  >{groupSize}</button>
                {/if}
                {#each badges(commit.refs) as badge (badge.ref)}
                  <RefBadge ref={badge.ref} isSynced={badge.isSynced} />
                {/each}
                <span class="shrink-0 text-[12px] font-medium text-muted-foreground">{commit.author}</span>
                <span class="shrink-0 text-[12px] text-tertiary" title={new Date(commit.date * 1000).toLocaleString()}>{when(commit.date)}</span>
              </div>
            {/if}
            <div class="truncate text-[12px] text-muted-foreground">{commit.subject}</div>
          </div>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .row.selected { background: var(--quinary); } /* faint tint instead of the strong system blue */
  .pill {
    min-width: 20px; height: 20px; padding: 0 4px; border-radius: 999px; background: var(--quinary);
    font-size: 11px; font-weight: 700; font-variant-numeric: tabular-nums;
  }
  .pill.dim { color: var(--muted-foreground); }
</style>
