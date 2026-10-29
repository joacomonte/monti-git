<!-- Virtualized commit list: only the rows on screen are in the DOM. Rows touch so the graph lines stay continuous. -->
<script lang="ts">
  import { Copy, FileText, Sparkles } from "@lucide/svelte";
  import { api, WORKTREE_ID, type Commit, type Row } from "$lib/api";
  import type { Range, RepoModel } from "$lib/model.svelte";
  import GraphCell, { LANE, PAD, laneColor } from "./GraphCell.svelte";
  import RefBadge from "./RefBadge.svelte";

  let { model }: { model: RepoModel } = $props();

  const ROW = 68; // two lines: branch badges (or the uncommitted row) above the subject
  const LINE = 44; // plain commit: date · subject · author on one line
  const QUIET = 32; // merge without refs: a short single-line row, so real commits lead
  const isQuietMerge = (c: Commit) => c.parents.length > 1 && c.refs.length === 0;
  const isTall = (c: Commit) => c.id === WORKTREE_ID || badges(c.refs).length > 0;

  // Right-click menu on a commit: copy it as context for an AI agent.
  let menu = $state<{ commit: Commit; x: number; y: number } | null>(null);
  let copied = $state<string | null>(null); // menu item just copied, for the ✓ before the menu closes

  /** Moves the node to <body>, so the fixed menu isn't clipped by the scrolling list. */
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  async function copy(item: string, text: Promise<string> | string) {
    await navigator.clipboard.writeText(await text);
    copied = item;
    setTimeout(() => { menu = null; copied = null; }, 500);
  }

  /** Enough for an agent with git access: it can `git show` the rest. */
  const minimal = (c: Commit) => `Repo: ${model.repo}\nCommit: ${c.id}`;

  /** Everything: where it is, who and when, the message, files changed and the full diff. */
  async function full(c: Commit) {
    const repo = model.repo!;
    const [info, places, patch] = await Promise.all([api.commitInfo(repo, c.id), api.commitPlaces(repo, c.id), api.commitPatch(repo, c.id)]);
    const date = (s: number) => new Date(s * 1000).toISOString().replace("T", " ").slice(0, 16);
    return [
      `Repo: ${repo}`,
      `Commit: ${info.id}`,
      `Branches containing it: ${places.join(", ") || "none"}`,
      `Author: ${info.author} <${info.authorEmail}>, ${date(info.authorDate)}`,
      info.committer !== info.author || info.committerDate !== info.authorDate ? `Committer: ${info.committer} <${info.committerEmail}>, ${date(info.committerDate)}` : null,
      `Parents: ${info.parents.join(", ") || "none (first commit)"}`,
      `Title: ${info.subject}`,
      info.body ? `Description:\n${info.body}` : null,
      `\nFiles changed and full diff:\n\`\`\`diff\n${patch.trim()}\n\`\`\``,
    ].filter((line) => line != null).join("\n");
  }

  let scrollTop = $state(0);
  let viewHeight = $state(600);
  let scroller: HTMLDivElement;

  const width = $derived(model.graphLanes * LANE + PAD * 2);
  const heightOf = (r: Range) => {
    const c = model.commits[r[0]];
    return r[0] === r[1] && isQuietMerge(c) ? QUIET : isTall(c) ? ROW : LINE;
  };
  /** Subject text tinted with its branch line's color, mixed with the foreground so it stays readable. */
  const tint = (r: Range) => `color-mix(in oklab, ${laneColor(rowFor(r).color)} 28%, var(--foreground))`;
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
  const rowFor = ([first]: Range): Row => model.rows[first];

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

  /** ↑/↓ move the selection. */
  function keydown(e: KeyboardEvent) {
    const i = model.visible.findIndex(isSelected);
    if (i < 0) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      const next = model.visible[i + (e.key === "ArrowDown" ? 1 : -1)];
      if (next) {
        model.select(next);
        scrollToRow(i + (e.key === "ArrowDown" ? 1 : -1));
      }
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
      {@const quiet = range[0] === range[1] && isQuietMerge(commit)}
      <div
        class="row absolute inset-x-0 flex"
        class:selected={isSelected(range)}
        style:top="{offsets[start + k]}px"
        style:height="{heightOf(range)}px"
        onclick={() => click(range)}
        oncontextmenu={(e) => { if (commit.id === WORKTREE_ID) return; e.preventDefault(); menu = { commit, x: e.clientX, y: e.clientY }; }}
        role="option"
        aria-selected={isSelected(range)}
        tabindex="-1"
        onkeydown={() => {}}
      >
        <div class="shrink-0" style:width="{width}px">
          <GraphCell index={range[0]} row={rowFor(range)} height={heightOf(range)} {width}
            isWorktree={commit.id === WORKTREE_ID} isMerge={commit.parents.length > 1} names={model.names} {highlight} />
        </div>
        {#if quiet}
          <div class="flex min-w-0 items-center px-2 text-[12px] text-tertiary opacity-70"><span class="truncate">{commit.subject}</span></div>
        {:else if !isTall(commit)}
          <div class="flex min-w-0 flex-1 items-center gap-2.5 px-2 text-[12.5px] whitespace-nowrap" class:opacity-55={commit.parents.length > 1}>
            <span class="shrink-0 text-tertiary" title={new Date(commit.date * 1000).toLocaleString()}>{when(commit.date)}</span>
            <span class="min-w-0 flex-1 truncate" style:color={tint(range)} title={commit.subject}>{commit.subject}</span>
            <span class="max-w-[35%] shrink-0 truncate text-tertiary" title={commit.author}>{commit.author}</span>
          </div>
        {:else}
          <div class="flex min-w-0 flex-1 flex-col justify-center gap-[5px] px-2" class:opacity-55={commit.parents.length > 1}>
            {#if commit.id === WORKTREE_ID}
              <div class="text-[13.5px] font-semibold">Uncommitted changes</div>
            {:else}
              <!-- Left to right: commit count, branches, date; the author on the right edge. -->
              <div class="flex min-w-0 items-center gap-1.5 overflow-hidden whitespace-nowrap">
                {#each badges(commit.refs) as badge (badge.ref)}
                  <RefBadge ref={badge.ref} isSynced={badge.isSynced} color={laneColor(rowFor(range).color)} />
                {/each}
                <span class="shrink-0 text-[12.5px] text-tertiary" title={new Date(commit.date * 1000).toLocaleString()}>{when(commit.date)}</span>
                <!-- Author right-aligned, same column as on one-line rows. -->
                <span class="ml-auto truncate pl-2 text-[12.5px] text-tertiary" title={commit.author}>{commit.author}</span>
              </div>
            {/if}
            <div class="truncate text-[12.5px]" style:color={tint(range)}>{commit.subject}</div>
          </div>
        {/if}
      </div>
    {/each}
  </div>
</div>

<svelte:window onmousedown={() => (menu = null)} onkeydown={(e) => e.key === "Escape" && (menu = null)} onwheel={() => (menu = null)} onblur={() => (menu = null)} />
{#if menu}
  {@const c = menu.commit}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div use:portal class="context-menu enter-fade" role="menu" tabindex="-1" id="commit-menu" onmousedown={(e) => e.stopPropagation()}
    style:left="{Math.min(menu.x, innerWidth - 300)}px" style:top="{Math.min(menu.y, innerHeight - 140)}px">
    <button role="menuitem" class="menu-item" id="commit-menu-copy-minimal" onclick={() => copy("minimal", minimal(c))}
      title="Repo and commit hash: enough for an agent with git to look at it itself">
      <Sparkles size={13} />{copied === "minimal" ? "Copied ✓" : "Copy for AI"}<span class="menu-hint">repo · hash</span>
    </button>
    <button role="menuitem" class="menu-item" id="commit-menu-copy-full" onclick={() => copy("full", full(c))}
      title="Everything: branches containing it, author, dates, parents, full message, files changed and the whole diff. Ej: to port the change to another repo">
      <FileText size={13} />{copied === "full" ? "Copied ✓" : "Copy for AI (full)"}<span class="menu-hint">+ message · diff</span>
    </button>
    <div class="my-1 h-px bg-border"></div>
    <button role="menuitem" class="menu-item" id="commit-menu-copy-hash" onclick={() => copy("hash", c.id)}>
      <Copy size={13} />{copied === "hash" ? "Copied ✓" : "Copy hash"}
    </button>
  </div>
{/if}

<style>
  .context-menu {
    position: fixed; z-index: 1000; width: max-content; min-width: 220px; padding: 4px; border-radius: 10px;
    background: var(--popover); color: var(--popover-foreground);
    box-shadow: 0 0 0 1px oklch(0.5 0 0 / 0.18), 0 8px 24px rgb(0 0 0 / 0.3);
  }
  .menu-item { display: flex; width: 100%; align-items: center; gap: 8px; height: 28px; padding: 0 10px; border-radius: 6px; font-size: 12px; text-align: left; white-space: nowrap; }
  .menu-item:hover { background: oklch(0.5 0 0 / 0.12); }
  .menu-hint { margin-left: auto; padding-left: 16px; font-size: 11px; color: var(--tertiary); }
  .row { transition: background-color 0.15s ease; }
  .row:hover { background: color-mix(in oklab, var(--quinary) 50%, transparent); }
  .row.selected { background: var(--quinary); } /* faint tint instead of the strong system blue */
</style>
