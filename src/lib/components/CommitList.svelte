<!-- Virtualized commit list: only the rows on screen are in the DOM. Rows touch so the graph lines stay continuous. -->
<script lang="ts">
  import { fade } from "svelte/transition";
  import { Archive, Check, ChevronRight, GitCommitHorizontal, PenLine, CircleCheck, Cloud, CloudOff, Copy, GitBranch, Sparkles, Trash2 } from "@lucide/svelte";
  import { api, WORKTREE_ID, type Card, type Commit, type Row } from "$lib/api";
  import { type Range, type RepoModel } from "$lib/model.svelte";
  import GraphCell, { LANE, PAD, laneColor } from "./GraphCell.svelte";
  import RefBadge from "./RefBadge.svelte";
  import ActionRow from "./ActionRow.svelte";
  import MessagePrompt from "./MessagePrompt.svelte";
  import BranchActions from "./BranchActions.svelte";
  import StartFeature from "./StartFeature.svelte";
  import UpdatePanel from "./UpdatePanel.svelte";

  let { model }: { model: RepoModel } = $props();

  const ROW = 68; // two lines: branch badges (or the uncommitted row) above the subject
  const LINE = 44; // plain commit: date · subject · author on one line
  const QUIET = 32; // merge without refs: a short single-line row, so real commits lead
  const isQuietMerge = (c: Commit) => c.parents.length > 1 && c.refs.length === 0;
  const isTall = (c: Commit) => c.id === WORKTREE_ID || !!c.shelved || badges(c.refs).length > 0;

  // Right-click menu on a commit: copy it as context for an AI agent.
  let menu = $state<{ commit: Commit; x: number; y: number } | null>(null);
  let copied = $state<string | null>(null); // menu item just copied, for the ✓ before the menu closes

  // Centered over the graph: merge/rebase from the base, delete, new branch.
  let dialog = $state<{ kind: "update" | "delete"; card: Card } | { kind: "new"; from: string } | null>(null);

  /** Options for a branch pill in the graph: check out, pull, new branch, copy. */
  let branchMenu = $state<{ name: string; commit: Commit; x: number; y: number } | null>(null);

  /** Branches only: a pill's card (behind, merged into, "N commits"), once fetched. */
  const cardOf = (ref: string): Card | null => cards.get(ref.replace("HEAD -> ", "")) ?? null;
  /** The base and the environments are what the others compare against: they stay plain pills. */
  const hasCard = (badge: string) => {
    const ref = badge.replace("HEAD -> ", "");
    return !ref.startsWith("tag: ") && localName(ref) !== localName(model.base ?? "") && !model.environments.some((e) => localName(e) === localName(ref));
  };

  // Cards' facts, fetched only for the rows on screen. Kept across reloads (shown until refetched, so nothing jumps).
  let cards = $state(new Map<string, Card>());
  let freshFor: unknown = null; // the history load `fresh` belongs to
  let fresh = new Set<string>(); // refs fetched since that load
  $effect(() => {
    const loaded = model.commits;
    const repo = model.repo, base = model.base;
    if (!model.branchesOnly || model.focus || !base || !repo) return;
    if (freshFor !== loaded) { freshFor = loaded; fresh = new Set(); }
    const refs = [...new Set(model.visible.slice(start, end).flatMap((r) => badges(model.commits[r[0]].refs)))]
      .map((b) => b.ref.replace("HEAD -> ", ""))
      .filter((ref) => hasCard(ref) && !fresh.has(ref));
    if (!refs.length) return;
    const targets = [base, ...model.environments];
    const timer = setTimeout(async () => {
      refs.forEach((ref) => fresh.add(ref)); // here, not before: a scroll cancels the timer and these must be asked again
      const found = await api.branchStatus(repo, refs, base, targets);
      if (model.repo !== repo) return;
      const next = new Map(cards);
      for (const f of found) next.set(f.ref, { ...f, local: localName(f.ref) });
      cards = next;
    }, 250); // scrolling settles first
    return () => clearTimeout(timer);
  });
  $effect(() => { model.repo; cards = new Map(); });
  /** Commits collapsed, while a branch's facts are still loading: the row keeps a card's height
   *  with a placeholder status line, so it doesn't jump when the numbers arrive. Index of the badge to draw it on, or -1. */
  function pendingAt(c: Commit) {
    if (!bare(c) || collapsedOf(c)) return -1;
    return badges(c.refs).findIndex((b) => isPending(b.ref));
  }
  const isPending = (ref: string) => hasCard(ref) && !!model.base && !cardOf(ref);
  /** Commits collapsed: the badge drawn as the row's card. The one with something to say, else the one loading, else the first branch. */
  function cardIndex(c: Commit) {
    if (!bare(c)) return -1;
    const list = badges(c.refs);
    const withStatus = list.findIndex((b) => { const card = cardOf(b.ref); return card != null && hasStatus(card); });
    const pending = pendingAt(c);
    return withStatus >= 0 ? withStatus : pending >= 0 ? pending : list.findIndex((b) => !b.ref.startsWith("tag: "));
  }

  /** Moves the node to <body>, so the fixed menu isn't clipped by the scrolling list. */
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  async function copy(item: string, text: Promise<string> | string) {
    await navigator.clipboard.writeText(await text);
    copied = item;
    setTimeout(() => { menu = null; branchMenu = null; copyMenu = null; copied = null; }, 500);
  }

  // Quick copy button on a branch row: the options open with "Metadata for AI" right under the pointer, so it's one more click.
  let copyMenu = $state<{ commit: Commit; branch: string; x: number; y: number } | null>(null);
  let writingMessage = $state<"pick" | "new" | "amend" | null>(null); // the "Message for AI" dialog: from right-click ("pick") or a quick button

  /** Repo, branch and hash: enough for an agent with git access, it can `git show` the rest. The branch: the one given
   *  (a clicked pill), else the feature branch the commit belongs to, else one pointing at it, else one containing it. */
  async function metadata(c: Commit, branch?: string) {
    const name = branch ?? badges(c.refs).find((b) => !b.ref.startsWith("tag: "))?.ref.replace("HEAD -> ", "")
      ?? (await api.commitPlaces(model.repo!, c.id))[0];
    return [`Repo: ${model.repo}`, name ? `Branch: ${name}` : null, `Commit: ${c.id}`].filter((line) => line != null).join("\n");
  }


  let scrollTop = $state(0);
  // Lineage spotlight: hovering a branch's card or badge lights its whole line and fades the other lines (rows stay as they are).
  let spot = $state<number | null>(null);
  // "N commits ›" opens the focus view; closing it comes back to the same scroll position.
  let beforeFocus: number | null = null;
  $effect(() => {
    if (model.focus || model.isLoading || beforeFocus == null || !scroller) return;
    const y = beforeFocus;
    beforeFocus = null;
    requestAnimationFrame(() => (scroller.scrollTop = y));
  });
  /** Opens the focus view from the top, remembering where the full graph was. */
  async function openFocus(ref: string) {
    beforeFocus = scroller.scrollTop;
    await model.setFocus(ref, model.base);
    scroller.scrollTop = 0;
  }
  // A click outside an open menu only closes it: the row or button under the pointer doesn't react to that click.
  let swallowing = false;
  function closeMenusOutside(e: MouseEvent) {
    swallowing = false;
    if (!(menu || branchMenu || copyMenu) || (e.target as Element).closest?.(".context-menu")) return;
    menu = null;
    branchMenu = null;
    copyMenu = null;
    swallowing = true;
  }
  function swallowClick(e: MouseEvent) {
    if (!swallowing) return;
    swallowing = false;
    e.stopPropagation();
    e.preventDefault();
  }
  let viewHeight = $state(600);
  let scroller: HTMLDivElement;

  const width = $derived(model.graphLanes * LANE + PAD * 2);
  // Collapsed: each row's graph only as wide as its own lanes, so the card sits right next to its line.
  const last = (lanes: (unknown | null)[]) => lanes.findLastIndex((l) => l != null);
  const widthOf = (r: Range) => {
    if (!model.branchesOnly || model.focus) return width;
    const row = rowFor(r);
    return (Math.max(last(row.before), last(row.after), row.column) + 1) * LANE + PAD * 2;
  };
  /** Branches only: a branch row is just its pills, count, date and author, on one line (no subject). */
  const bare = (c: Commit) => model.branchesOnly && !model.focus && c.id !== WORKTREE_ID && !c.shelved;
  /** Branches only: the branch whose status line goes under the row (first pill with something to say). */
  const collapsedOf = (c: Commit) => (bare(c) ? badges(c.refs).map((b) => cardOf(b.ref)).find((card) => card != null && hasStatus(card)) ?? null : null);

  /** What a branch is compared against (the base), how far behind, and whether it's finished: every commit everywhere. */
  function statusOf(card: Card) {
    const done = card.promotion.length > 0 && card.promotion.every(([, missing]) => missing === 0);
    return { against: localName(model.base ?? "main"), behind: card.behindBase, done };
  }
  /** Only branches with something to say get a status line. */
  function hasStatus(card: Card) {
    const { behind, done } = statusOf(card);
    return card.firstParentCount > 0 || behind > 0 || done || card.promotion.some(([, missing]) => missing === 0);
  }
  // Collapsed rows are compact cards: name line, plus the status line when the branch has something to say.
  const COLLAPSED = 64; // every branch row: card (52) with "N commits" beside it
  const CARD = 48;
  const localName = (ref: string) => (/^(origin|upstream)\//.test(ref) ? ref.slice(ref.indexOf("/") + 1) : ref);


  const heightOf = (r: Range) => {
    const c = model.commits[r[0]];
    return r[0] === r[1] && isQuietMerge(c) ? QUIET : isTall(c) && !bare(c) ? ROW : cardIndex(c) >= 0 ? COLLAPSED : bare(c) ? CARD : LINE;
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


  // Reveal request (search, uncommitted changes): bring that row to the upper third, once.
  $effect(() => {
    const request = model.scrollRow;
    if (!request || !scroller) return;
    model.scrollRow = null;
    // The list may have just mounted: wait a frame so it has its real height to scroll in.
    requestAnimationFrame(() => (scroller.scrollTop = Math.max(0, offsets[request.row] - viewHeight / 3)));
  });

  $effect(() => { model.viewEnd = end; });
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
        class:shelved={!!commit.shelved}
        style:top="{offsets[start + k]}px"
        style:height="{heightOf(range)}px"
        onclick={() => click(range)}
        oncontextmenu={(e) => { if (commit.id === WORKTREE_ID) { e.preventDefault(); writingMessage = "pick"; return; } if (commit.shelved) return; e.preventDefault();
          // A row with a branch: the branch menu (same as clicking its badge); a plain commit: the commit menu.
          const branch = badges(commit.refs).find((b) => !b.ref.startsWith("tag: "));
          if (branch) { menu = null; branchMenu = { name: branch.ref.replace("HEAD -> ", ""), commit, x: e.clientX, y: e.clientY }; }
          else { branchMenu = null; menu = { commit, x: e.clientX, y: e.clientY }; } }}
        role="option"
        aria-selected={isSelected(range)}
        tabindex="-1"
        onkeydown={() => {}}
      >
        <div class="shrink-0" style:width="{widthOf(range)}px">
          <GraphCell index={range[0]} row={rowFor(range)} height={heightOf(range)} width={widthOf(range)}
            isWorktree={commit.id === WORKTREE_ID} isShelved={!!commit.shelved} isMerge={commit.parents.length > 1} isHead={commit.refs.some((ref) => ref.startsWith("HEAD -> "))} names={model.names} grey={model.branchesOnly && !model.focus}
            {spot} hidden={(collapsedOf(commit)?.firstParentCount ?? 1) - 1} />
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
          <div class="flex min-w-0 flex-1 flex-col justify-center gap-[5px] px-2" class:opacity-55={commit.parents.length > 1 && !bare(commit)}>
            {#if commit.id === WORKTREE_ID}
              <div class="text-[13.5px] font-semibold">Uncommitted changes</div>
            {:else if commit.shelved}
              <div class="flex min-w-0 items-center gap-1.5 overflow-hidden whitespace-nowrap text-[13px]">
                <Archive size={13} class="shrink-0 text-amber-400" />
                <span class="font-semibold text-amber-400">WIP saved</span>
                <span class="truncate text-muted-foreground">from {commit.shelved}</span>
                <span class="shrink-0 text-[12.5px] text-tertiary" title={new Date(commit.date * 1000).toLocaleString()}>· saved {when(commit.date)}</span>
              </div>
              <div class="truncate text-[12px] text-tertiary">{commit.subject} · comes back when you check out {commit.shelved}</div>
            {:else}
              {@const list = badges(commit.refs)}
              <!-- Branches on the left; date and author on the right edge. -->
              <div class="flex min-w-0 items-center gap-1.5 overflow-hidden whitespace-nowrap">
                {#each list as badge, i (badge.ref)}
                  {@const openMenu = (e: MouseEvent) => { e.stopPropagation(); menu = null; const name = badge.ref.replace("HEAD -> ", ""); if (branchMenu?.name === name && branchMenu.commit.id === commit.id) { branchMenu = null; return; } const r = (e.currentTarget as HTMLElement).getBoundingClientRect(); branchMenu = { name, commit, x: r.left, y: r.bottom + 4 }; }}
                  {#if bare(commit) && !badge.ref.startsWith("tag: ")}
                    <!-- Commits collapsed: every branch is the same solid card, its status and actions inside when it has any
                         (placeholder bars while they load). -->
                    {@const own = cardOf(badge.ref)}
                    {@const name = localName(badge.ref.replace("HEAD -> ", ""))}
                    {@const isHead = badge.ref.startsWith("HEAD -> ")}
                    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
                    <div class="bcard" onclick={openMenu} onmouseenter={() => (spot = rowFor(range).color)} onmouseleave={() => (spot = null)} oncontextmenu={(e) => { e.preventDefault(); openMenu(e); }} class:head={isHead}
                      style:--lane={laneColor(rowFor(range).color)} aria-busy={isPending(badge.ref)}>
                      <button class="bname" id="branch-card-{name}" onclick={openMenu}>
                        {#if isHead}<CircleCheck size={13} strokeWidth={2.5} />{:else}<GitBranch size={13} strokeWidth={2.5} />{/if}<span class="truncate">{name}</span>
                        {#if badge.isSynced || /^(origin|upstream)\//.test(badge.ref)}<span class="cloud" title="On the server at this commit: nothing to push or pull."><Cloud size={12} strokeWidth={2.5} /></span>{/if}
                      </button>
                      {#if own && hasStatus(own)}{@render status(own)}
                      {:else if isPending(badge.ref)}<div class="status"><span class="skel"></span><span class="skel w-[54px]"></span></div>{/if}
                    </div>
                    <!-- Right of the card: opens the branch focused (the focus view); turning that off comes back here. -->
                    {#if own && hasStatus(own) && own.firstParentCount}
                      <button class="expand" id="branch-expand-{own.local}"
                        onclick={(e) => { e.stopPropagation(); openFocus(own.ref); }}>
                        {own.firstParentCount} commit{own.firstParentCount === 1 ? "" : "s"}<ChevronRight size={11} />
                      </button>
                    {:else if isPending(badge.ref)}<span class="skel ml-1 w-[64px]"></span>{/if}
                    <RefBadge ref={badge.ref} isSynced={badge.isSynced} chipsOnly />
                  {:else}
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <span class="contents" onmouseenter={() => { if (!badge.ref.startsWith("tag: ")) spot = rowFor(range).color; }} onmouseleave={() => (spot = null)}>
                      <RefBadge ref={badge.ref} isSynced={badge.isSynced} color={laneColor(rowFor(range).color)}
                        onclick={!badge.ref.startsWith("tag: ") ? openMenu : undefined}
                        oncontextmenu={!badge.ref.startsWith("tag: ") ? (e) => { e.preventDefault(); openMenu(e); } : undefined} />
                    </span>
                  {/if}
                {/each}
                <!-- Date and author together on the right edge. -->
                <span class="meta-right ml-auto shrink-0 pl-2 text-[12.5px] text-tertiary" title={new Date(commit.date * 1000).toLocaleString()}>{when(commit.date)}</span>
                <span class="meta-right author truncate text-[12.5px] text-tertiary" title={commit.author}>· {commit.author}</span>
              </div>
            {/if}
            {#if !commit.shelved && !bare(commit)}<div class="truncate text-[12.5px]" style:color={tint(range)}>{commit.subject}</div>{/if}
          </div>
        {/if}
        <!-- Quick actions on hover, far right: commit / amend for the uncommitted row, copy for AI for a branch row. -->
        {#if commit.id === WORKTREE_ID}
          <div class="quick">
            <button id="quick-commit" title="Commit message by your AI agent, for a new commit" onclick={(e) => { e.stopPropagation(); writingMessage = "new"; }}>
              <GitCommitHorizontal size={13} />Commit</button>
            <button id="quick-amend" title="Amend the last commit, with your AI agent" onclick={(e) => { e.stopPropagation(); writingMessage = "amend"; }}>
              <PenLine size={13} />Amend</button>
          </div>
        {:else if !commit.shelved && badges(commit.refs).some((b) => !b.ref.startsWith("tag: "))}
          {@const branch = badges(commit.refs).find((b) => !b.ref.startsWith("tag: "))!.ref.replace("HEAD -> ", "")}
          <div class="quick">
            <button class="icon-only" id="quick-copy-{commit.id}" title="Copy: branch name, hash, or metadata for AI"
              onclick={(e) => { e.stopPropagation(); menu = null; branchMenu = null; copyMenu = { commit, branch, x: e.clientX, y: e.clientY }; }}
              onmousedown={(e) => e.stopPropagation()}><Copy size={13} /></button>
          </div>
        {/if}
      </div>
    {/each}
  </div>
</div>

<!-- Commits collapsed: a branch's second line (behind, not merged yet, merged into, finished) with its actions. -->
{#snippet status(card: Card)}
  {@const { against, behind, done } = statusOf(card)}
  {@const stop = (action: () => void) => (e: MouseEvent) => { e.stopPropagation(); action(); }}
  <div class="status">
    {#if done}
      <!-- How it got there: the weakest way across targets in the label, each target's way on hover. -->
      {@const ways = card.promotion.map(([, , how]) => how)}
      {@const label = ways.includes("squashed") ? "Squashed" : ways.includes("cherry-picked") ? "Cherry-picked" : ways.includes("fast-forwarded") ? "Fast-forwarded" : "Merged"}
      {@const explain = { merged: "merged", "fast-forwarded": "fast-forwarded: the base moved up to this branch, no merge commit", "cherry-picked": "same changes copied (cherry-pick or rebase)", squashed: "squashed: its contents are already there", "": "" }}
      {@const how = card.promotion.map(([t, , how]) => `${localName(t)}: ${explain[how]}`).join("\n")}
      {@const deletable = model.localBranches.includes(card.local) && !model.isCurrent(card.ref)}
      <!-- Hovering the label swaps it for the delete button, in the same spot. -->
      <span class="swap" class:deletable>
        <span class="st label" title="{how}\nSafe to delete">✓ {label}</span>
        {#if deletable}
          <button class="st del" id="branch-status-delete-{card.local}" title="{how}\nDelete this branch…"
            onclick={stop(() => (dialog = { kind: "delete", card }))}><Trash2 size={11} strokeWidth={2.5} />&nbsp;Safe to delete</button>
        {/if}
      </span>
    {:else}
      {#if behind}
        <button class="st warn" id="branch-status-behind-{card.local}" disabled={!model.localBranches.includes(card.local)}
          onclick={stop(() => (dialog = { kind: "update", card }))}><span class="short">↓ {behind}</span><span class="more">&nbsp;behind {against} · Merge or rebase…</span></button>
      {/if}
      <!-- Merged somewhere but not everywhere yet: one quiet grey note, the places on hover. -->
      {@const pending = card.promotion.filter(([t, missing]) => missing > 0)}
      {#if pending.length && card.promotion.some(([, missing]) => missing === 0)}
        <span class="note">
          <span class="st" title="Not in {pending.map(([t, missing]) => `${localName(t)} (${missing} commit${missing === 1 ? '' : 's'})`).join(', ')}">{#if pending.some(([t]) => localName(t) === against)}Not in {against}{pending.length > 1 ? ` · +${pending.length - 1}` : ""}{:else}Pending merge{pending.length > 1 ? ` · ${pending.length}` : ""}{/if}</span>
        </span>
      {/if}
      <!-- In no environment nor the base yet: work left halfway, worth a warning. Being merged somewhere needs no note. -->
      {#if card.firstParentCount > 0 && card.promotion.every(([, missing]) => missing > 0)}
        <span class="st alert" title="None of its commits are in {localName(model.base ?? 'main')} or any environment">Not merged anywhere</span>
      {/if}
    {/if}
  </div>
{/snippet}

<svelte:window onmousedowncapture={closeMenusOutside} onclickcapture={swallowClick} onkeydown={(e) => { if (e.key === "Escape") { menu = null; branchMenu = null; copyMenu = null; dialog = null; } }}
  onwheel={() => { menu = null; branchMenu = null; copyMenu = null; }} onblur={() => { menu = null; branchMenu = null; copyMenu = null; }} />
{#if copyMenu}
  {@const { commit, branch } = copyMenu}
  <!-- First item centered on the pointer (6px menu padding + half a 34px row), shifted left only if it would leave the window. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div use:portal class="context-menu copy-menu enter-pop" out:fade={{ duration: 120 }} role="menu" tabindex="-1" id="copy-menu" onmousedown={(e) => e.stopPropagation()}
    style:left="{Math.min(copyMenu.x - 60, innerWidth - 236)}px" style:top="{Math.min(copyMenu.y - 23, innerHeight - 130)}px">
    <button class="copy-item" id="copy-menu-metadata" onclick={() => copy("copy-metadata", metadata(commit, branch))}>
      {#if copied === "copy-metadata"}<Check size={13} />{:else}<Sparkles size={13} class="text-[#a78bfa]" />{/if}Metadata for AI</button>
    <button class="copy-item" id="copy-menu-name" onclick={() => copy("copy-name", localName(branch))}>
      {#if copied === "copy-name"}<Check size={13} />{:else}<GitBranch size={13} />{/if}Branch name<span class="hint">{localName(branch)}</span></button>
    <button class="copy-item" id="copy-menu-hash" onclick={() => copy("copy-hash", commit.id)}>
      {#if copied === "copy-hash"}<Check size={13} />{:else}<Copy size={13} />{/if}Hash<span class="hint font-mono">{commit.id.slice(0, 7)}</span></button>
  </div>
{/if}
{#if branchMenu}
  {@const { name, commit } = branchMenu}
  {@const isCurrent = model.isCurrent(name)}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div use:portal class="context-menu fit enter-pop" out:fade={{ duration: 120 }} role="menu" tabindex="-1" id="branch-pill-menu" onmousedown={(e) => e.stopPropagation()}
    style:left="{Math.min(branchMenu.x, innerWidth - 380)}px" style:top="{Math.max(8, Math.min(branchMenu.y, innerHeight - 520))}px">
    <BranchActions {name} local={localName(name)} hash={commit.id} isBase={localName(name) === localName(model.base ?? "")} {isCurrent} {model}
      dismiss={() => (branchMenu = null)} onNewBranch={() => (dialog = { kind: "new", from: localName(name) })}
      onFocus={() => openFocus(name)} />
  </div>
{/if}
{#if writingMessage}<MessagePrompt {model} start={writingMessage === "pick" ? null : writingMessage} onclose={() => (writingMessage = null)} />{/if}
{#if menu}
  {@const c = menu.commit}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div use:portal class="context-menu enter-pop" out:fade={{ duration: 120 }} role="menu" tabindex="-1" id="commit-menu" onmousedown={(e) => e.stopPropagation()}
    style:left="{Math.min(menu.x, innerWidth - 340)}px" style:top="{Math.min(menu.y, innerHeight - 130)}px">
    <ActionRow title={copied === "metadata" ? "Copied ✓" : "Copy metadata for AI"} icon={Sparkles} color="#a78bfa" id="commit-menu-copy-metadata" onclick={() => copy("metadata", metadata(c))}
      detail="Repo, branch and commit hash: enough for an agent with git to look at it itself"
      example={'paste it in Claude Code and ask "why did this break the build?": it runs git show by itself.'} />
    <ActionRow title={copied === "hash" ? "Copied ✓" : "Copy hash"} icon={Copy} grey id="commit-menu-copy-hash" onclick={() => copy("hash", c.id)}
      detail={c.id.slice(0, 7)} />
  </div>
{/if}

{#if dialog}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div use:portal class="dialog-backdrop enter-fade" out:fade={{ duration: 150 }} id="branch-dialog" onclick={() => (dialog = null)}>
    <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
    <div class="dialog-panel enter-pop" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      {#if dialog.kind === "new"}<StartFeature {model} from={dialog.from} dismiss={() => (dialog = null)} />
      {:else if dialog.kind === "update"}<UpdatePanel card={dialog.card} {model} isCurrent={model.isCurrent(dialog.card.ref)} dismiss={() => (dialog = null)} />
      {:else}
        {@const card = dialog.card}
        <!-- Delete a finished branch; git itself refuses if anything isn't merged. -->
        <div class="flex w-[400px] flex-col gap-1">
          <span class="px-1.5 pt-1 text-[13px] font-semibold">Delete {card.local}?</span>
          <span class="px-1.5 pb-1 text-[12px] text-muted-foreground">Git refuses if it has commits that aren't merged, so no work can get lost.</span>
          <ActionRow title="Delete branch" detail="Only on your machine (git branch -d). The server copy stays." icon={Trash2} color="#f5a524"
            id="branch-delete-local" onclick={() => { model.deleteBranch(card.local, false); dialog = null; }} />
          {#if card.hasUpstream}
            <ActionRow title="Delete here and on the server" detail="Also removes it from origin, for the whole team." icon={CloudOff} color="#f5a524"
              id="branch-delete-remote" onclick={() => { model.deleteBranch(card.local, true); dialog = null; }} />
          {/if}
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  /* Commits collapsed: the branch as a solid card (name + status line), tinted with its line color; checked out = deeper fill. */
  .bcard {
    display: flex; flex-direction: column; justify-content: center; gap: 3px; min-width: 0; flex-shrink: 1; height: 52px; padding: 0 12px; border-radius: 12px;
    /* Pastel: a light wash of the line color, text leaning to the foreground. */
    color: color-mix(in oklab, var(--lane) 28%, var(--foreground)); background: color-mix(in oklab, var(--lane) 7%, var(--background));
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--lane) 16%, transparent);
  }
  /* Checked out: solid, a diagonal fade of its line color (darkened so white text keeps its contrast). */
  .bcard.head {
    color: white;
    background: linear-gradient(135deg, color-mix(in oklab, var(--lane) 78%, black), color-mix(in oklab, var(--lane) 45%, black));
    box-shadow: none;
  }
  .bcard.head .bname .cloud { opacity: 0.75; }
  .bcard.head :is(.st, .note .st) { color: rgb(255 255 255 / 0.72); }
  .bcard.head .st.warn, .row.selected .bcard.head .st.warn { color: #ffd58a; }
  .bcard.head .st:hover:not(:disabled) { color: white; }
  .bname { display: flex; align-items: center; gap: 5px; min-width: 0; font-size: 13px; font-weight: 700; color: inherit; text-align: left; }
  .bname .cloud { display: flex; flex-shrink: 0; margin-left: 3px; opacity: 0.6; }
  /* Hover: the card glows a little in its own color (no underline). */
  .bcard { transition: background 0.15s, box-shadow 0.15s, color 0.15s; }
  .bcard:not(.head):hover {
    color: color-mix(in oklab, var(--lane) 45%, var(--foreground)); background: color-mix(in oklab, var(--lane) 12%, var(--background));
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--lane) 38%, transparent);
  }
  .bcard.head:hover { filter: brightness(1.12); }
  /* Status line inside the card: quiet text; amber only for what needs you (behind, uncommitted). */
  .expand { display: flex; flex-shrink: 0; align-items: center; gap: 1px; margin-left: 6px; white-space: nowrap; font-size: 11.5px; color: var(--tertiary); }
  .expand:hover { color: var(--foreground); }
  /* Minimal by default (↓ 18, ⑂ main); the full sentence and its action on hover. */
  .st .short { display: flex; align-items: center; gap: 3px; }
  .st .more { max-width: 0; overflow: hidden; opacity: 0; transition: max-width 0.3s var(--ease), opacity 0.2s var(--ease); }
  .st:hover .more { max-width: 280px; opacity: 1; }
  .note { display: flex; flex-shrink: 0; align-items: center; gap: 2px; }
  .note .st { color: var(--tertiary); }
  .status { display: flex; align-items: center; gap: 18px; min-width: 0; overflow: hidden; white-space: nowrap; font-size: 11.5px; }
  .st { display: flex; flex-shrink: 0; align-items: center; gap: 1px; color: var(--muted-foreground); }
  button.st:hover:not(:disabled) { color: var(--foreground); }
  .st.warn { color: color-mix(in oklab, #f5a524 75%, var(--foreground)); }
  .st.warn:hover:not(:disabled) { color: #ffc35c; }
  .swap { display: grid; flex-shrink: 0; }
  .swap > * { grid-area: 1 / 1; transition: opacity 0.12s; }
  .swap .del { opacity: 0; pointer-events: none; }
  .swap.deletable:hover .label { opacity: 0; }
  .swap.deletable:hover .del { opacity: 1; pointer-events: auto; }
  .st.alert, .bcard.head .st.alert { color: #f5a524; font-weight: 600; }
  /* Behind stays grey on other branches; amber only on the checked-out one or the selected row. */
  .bcard:not(.head) .st.warn { color: var(--muted-foreground); }
  .row.selected .bcard .st.warn { color: color-mix(in oklab, #f5a524 75%, var(--foreground)); }
  .context-menu {
    position: fixed; z-index: 1000; width: 320px; padding: 6px; border-radius: 10px;
    background: var(--popover); color: var(--popover-foreground);
    box-shadow: 0 0 0 1px oklch(0.5 0 0 / 0.18), 0 8px 24px rgb(0 0 0 / 0.3);
  }
  .context-menu.fit { width: auto; max-height: calc(100vh - 16px); overflow: auto; padding: 10px; border-radius: 14px; } /* BranchActions sets its own width */
  .skel { display: inline-block; width: 34px; height: 9px; border-radius: 999px; background: oklch(0.5 0 0 / 0.16); animation: skel 1.2s ease-in-out infinite; }
  @keyframes skel { 50% { opacity: 0.45; } }
  @media (prefers-reduced-motion: reduce) { .skel { animation: none; } }
  .row.shelved { background: color-mix(in oklab, oklch(0.8 0.15 75) 7%, transparent); }
  .dialog-backdrop { position: fixed; inset: 0; z-index: 1000; display: grid; place-items: center; background: rgb(0 0 0 / 0.35); }
  .dialog-panel { padding: 10px; border-radius: 12px; background: var(--popover); color: var(--popover-foreground); box-shadow: 0 0 0 1px oklch(0.5 0 0 / 0.18), 0 16px 40px rgb(0 0 0 / 0.4); }
  .row { transition: background-color 0.15s ease; }
  .quick { position: absolute; top: 50%; right: 10px; display: flex; gap: 6px; opacity: 0; pointer-events: none; translate: 4px -50%; transition: opacity 0.15s, translate 0.15s; }
  .meta-right.author { transition: margin-right 0.15s; }
  .row:hover:has(.quick) .meta-right.author { margin-right: 44px; } /* slides left to make room for the copy button */
  .row:hover .quick { opacity: 1; pointer-events: auto; translate: 0 -50%; }
  .quick button { display: flex; align-items: center; gap: 5px; height: 26px; padding: 0 10px; border-radius: 999px; font-size: 12px; font-weight: 600;
    color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.1); transition: color 0.15s, background 0.15s; }
  .quick button.icon-only { width: 26px; padding: 0; justify-content: center; }
  .copy-menu { width: 224px; }
  .copy-item { display: flex; width: 100%; align-items: center; gap: 8px; height: 34px; padding: 0 10px; border-radius: 7px; font-size: 12.5px; font-weight: 500; text-align: left; }
  .copy-item:hover { background: oklch(0.5 0 0 / 0.12); }
  .copy-item .hint { min-width: 0; margin-left: auto; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 11px; color: var(--tertiary); }
  .quick button:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.2); }
  .row:hover { background: color-mix(in oklab, var(--quinary) 50%, transparent); }
  .row.selected { background: var(--quinary); } /* faint tint instead of the strong system blue */
</style>
