<!-- Virtualized commit list: only the rows on screen are in the DOM. Rows touch so the graph lines stay continuous. -->
<script lang="ts">
  import { fade } from "svelte/transition";
  import { Archive, Check, ChevronRight, GitCommitHorizontal, PenLine, CircleCheck, Cloud, CloudOff, Copy, GitBranch, GitMerge, Layers, Sparkles, Trash2, X } from "@lucide/svelte";
  import { allCards, api, featureCards, WORKTREE_ID, type Card, type Commit, type Row } from "$lib/api";
  import { prefs, type Range, type RepoModel } from "$lib/model.svelte";
  import { isShipped } from "$lib/branchFilters";
  import GraphCell, { LANE, PAD, laneColor } from "./GraphCell.svelte";
  import RefBadge from "./RefBadge.svelte";
  import SquashPrompt from "./SquashPrompt.svelte";
  import ActionRow from "./ActionRow.svelte";
  import MessagePrompt from "./MessagePrompt.svelte";
  import BranchActions from "./BranchActions.svelte";
  import PrPrompt from "./PrPrompt.svelte";
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

  /** The feature branch whose own commits include this one (from the dashboard), to squash them all. */
  function branchOf(c: Commit): Card | null {
    const d = model.dashboard;
    if (!d) return null;
    return featureCards(d).find((card) => card.squashBase && card.featureCommits.some((f) => f.id === c.id)) ?? null;
  }
  // Centered over the graph: squash, PR, merge/rebase from its parent, delete, new branch.
  let dialog = $state<{ kind: "squash" | "pr" | "update" | "delete"; card: Card } | { kind: "new"; from: string } | null>(null);

  /** Options for a branch pill in the graph: check out (if it isn't already) and squash (if it has commits to squash). */
  let branchMenu = $state<{ name: string; commit: Commit; x: number; y: number } | null>(null);
  function squashable(ref: string): Card | null {
    const d = model.dashboard;
    const name = ref.replace("HEAD -> ", "").replace(/^(origin|upstream)\//, "");
    const card = d ? featureCards(d).find((c) => c.local === name) : null;
    return card?.squashBase && card.featureCommits.length > 1 ? card : null;
  }

  /** Branches only: the dashboard card of a pill's branch, for its "N commits" (own commits vs its base). */
  function cardOf(ref: string): Card | null {
    const d = model.dashboard;
    const name = ref.replace("HEAD -> ", "").replace(/^(origin|upstream)\//, "");
    return (d ? allCards(d).find((c) => c.local === name) : null) ?? lite.get(ref.replace("HEAD -> ", "")) ?? null;
  }

  // Branches without a dashboard card (someone else's, remote-only): their behind/merged facts, fetched only for the rows
  // on screen and cached until the dashboard reloads. A Card with the rest empty, so statusOf/hasStatus/the card UI just work.
  let lite = $state(new Map<string, Card>());
  let freshFor: unknown = null; // the dashboard `fresh` belongs to
  let fresh = new Set<string>(); // refs fetched for the current dashboard; older entries stay shown until refetched, so nothing jumps
  $effect(() => {
    const d = model.dashboard;
    const repo = model.repo;
    if (!model.branchesOnly || model.focus || !d?.base || !repo) return;
    if (freshFor !== d) { freshFor = d; fresh = new Set(); }
    const known = new Set(allCards(d).map((c) => c.local));
    const refs = [...new Set(model.visible.slice(start, end).flatMap((r) => badges(model.commits[r[0]].refs)))]
      .map((b) => b.ref.replace("HEAD -> ", ""))
      .filter((ref) => !ref.startsWith("tag: ") && !known.has(localName(ref)) && !fresh.has(ref));
    if (!refs.length) return;
    refs.forEach((ref) => fresh.add(ref));
    const targets = [d.base.ref, ...d.environments.map((c) => c.ref)];
    const timer = setTimeout(async () => {
      const found = await api.branchStatus(repo, refs, d.base!.ref, targets);
      if (model.dashboard !== d) return;
      const next = new Map(lite);
      for (const f of found) next.set(f.ref, { ...f, local: localName(f.ref), parent: null, behindParent: 0, featureCommits: [], mergedIn: 0,
        uncommitted: null, alreadyIn: [], mergedBefore: [], squashBase: null } as unknown as Card);
      lite = next;
    }, 250); // scrolling settles first
    return () => clearTimeout(timer);
  });
  $effect(() => { model.repo; lite = new Map(); });
  /** Commits collapsed, while the dashboard (or a foreign branch's facts) is still loading: the row keeps a card's height
   *  with a placeholder status line, so it doesn't jump when the numbers arrive. Index of the badge to draw it on, or -1. */
  function pendingAt(c: Commit) {
    if (!bare(c) || collapsedOf(c)) return -1;
    return badges(c.refs).findIndex((b) => isPending(b.ref));
  }
  function isPending(ref: string) {
    const d = model.dashboard;
    return !ref.startsWith("tag: ") && !cardOf(ref) && (!d || (!!d.base && !lite.has(ref.replace("HEAD -> ", ""))));
  }
  /** Commits collapsed: the badge drawn as the row's card. The one with something to say, else the one loading, else the first branch. */
  function cardIndex(c: Commit) {
    if (!bare(c)) return -1;
    const list = badges(c.refs);
    const withStatus = list.findIndex((b) => { const card = cardOf(b.ref); return card != null && hasStatus(card); });
    const pending = pendingAt(c);
    return withStatus >= 0 ? withStatus : pending >= 0 ? pending : list.findIndex((b) => !b.ref.startsWith("tag: "));
  }
  const isLite = (card: Card) => lite.get(card.ref) === card; // no dashboard card: delete and PR need one

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
    const name = branch ?? branchOf(c)?.local ?? badges(c.refs).find((b) => !b.ref.startsWith("tag: "))?.ref.replace("HEAD -> ", "")
      ?? (await api.commitPlaces(model.repo!, c.id))[0];
    return [`Repo: ${model.repo}`, name ? `Branch: ${name}` : null, `Commit: ${c.id}`].filter((line) => line != null).join("\n");
  }


  let scrollTop = $state(0);
  // "N commits ›" opens "Only this branch"; closing it comes back to the same scroll position.
  let beforeFocus: number | null = null;
  $effect(() => {
    if (model.focus || model.isLoading || beforeFocus == null || !scroller) return;
    const y = beforeFocus;
    beforeFocus = null;
    requestAnimationFrame(() => (scroller.scrollTop = y));
  });
  let viewHeight = $state(600);
  let scroller: HTMLDivElement;

  const width = $derived(model.graphLanes * LANE + PAD * 2);
  /** Branches only: a branch row is just its pills, count, date and author, on one line (no subject). */
  const bare = (c: Commit) => model.branchesOnly && !model.focus && c.id !== WORKTREE_ID && !c.shelved;
  /** Branches only: the branch whose status line goes under the row (first pill with a dashboard card). */
  const collapsedOf = (c: Commit) => (bare(c) ? badges(c.refs).map((b) => cardOf(b.ref)).find((card) => card != null && hasStatus(card)) ?? null : null);

  /** What a branch is compared against, how far behind, and whether it's finished (shipped everywhere). */
  function statusOf(card: Card) {
    const d = model.dashboard;
    const kind = card === d?.base ? "base" : d?.environments.includes(card) ? "environment" : "feature";
    const against = card.parent ? localName(card.parent) : kind === "feature" ? localName(d?.base?.ref ?? "main") : null;
    const behind = card.parent ? card.behindParent : kind === "feature" ? card.behindBase : 0;
    return { kind, against, behind, done: kind === "feature" && isShipped(card) && !card.uncommitted };
  }
  /** Only branches with something to say grow into a card; the rest (e.g. main, up to date) stay a plain pill. */
  function hasStatus(card: Card) {
    const { against, behind, done } = statusOf(card);
    const d = model.dashboard;
    const merged = card !== d?.base && !d?.environments.includes(card) && card.promotion.some(([, missing]) => missing === 0);
    return card.firstParentCount > 0 || (!!against && behind > 0) || done || !!card.uncommitted || merged;
  }
  // Collapsed rows are compact cards: name line, plus the status line when the branch has a dashboard card.
  const GREY = "var(--muted-foreground)"; // collapsed: only the checked-out branch keeps its line color
  const COLLAPSED = 86; // every branch row: card (52) plus room for the "N commits" link under it
  const CARD = 48;
  const localName = (ref: string) => (/^(origin|upstream)\//.test(ref) ? ref.slice(ref.indexOf("/") + 1) : ref);

  // "Not merged into main" notes the user hid, per repo + branch + target; they stay hidden until the branch changes name.
  let hiddenNotes = $state<string[]>(prefs.get("hiddenMergeNotes", []));
  const noteKey = (card: Card, target: string) => `${model.repo}|${card.local}|${localName(target)}`;
  function hideNote(card: Card, target: string) {
    hiddenNotes = [...hiddenNotes, noteKey(card, target)];
    prefs.set("hiddenMergeNotes", hiddenNotes);
  }

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
        <div class="shrink-0" style:width="{width}px">
          <GraphCell index={range[0]} row={rowFor(range)} height={heightOf(range)} {width}
            isWorktree={commit.id === WORKTREE_ID} isShelved={!!commit.shelved} isMerge={commit.parents.length > 1} isHead={commit.refs.some((ref) => ref.startsWith("HEAD -> "))} names={model.names} grey={model.branchesOnly && !model.focus} />
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
                    <div class="bcard" onclick={openMenu} oncontextmenu={(e) => { e.preventDefault(); openMenu(e); }} class:head={isHead}
                      style:--lane={isHead ? laneColor(rowFor(range).color) : GREY} aria-busy={isPending(badge.ref)}>
                      <button class="bname" id="branch-card-{name}" onclick={openMenu}>
                        {#if isHead}<CircleCheck size={13} strokeWidth={2.5} />{:else}<GitBranch size={13} strokeWidth={2.5} />{/if}<span class="truncate">{name}</span>
                        {#if badge.isSynced || /^(origin|upstream)\//.test(badge.ref)}<span class="cloud" title="On the server at this commit: nothing to push or pull."><Cloud size={12} strokeWidth={2.5} /></span>{/if}
                      </button>
                      {#if own && hasStatus(own)}{@render status(own)}
                      {:else if isPending(badge.ref)}<div class="status"><span class="skel"></span><span class="skel w-[54px]"></span></div>{/if}
                    </div>
                    <RefBadge ref={badge.ref} isSynced={badge.isSynced} chipsOnly />
                  {:else}
                    <RefBadge ref={badge.ref} isSynced={badge.isSynced} color={bare(commit) && !badge.ref.startsWith("HEAD -> ") ? GREY : laneColor(rowFor(range).color)}
                      onclick={!badge.ref.startsWith("tag: ") ? openMenu : undefined}
                      oncontextmenu={!badge.ref.startsWith("tag: ") ? (e) => { e.preventDefault(); openMenu(e); } : undefined} />
                  {/if}
                {/each}
                <!-- Date and author together on the right edge. -->
                <span class="meta-right ml-auto shrink-0 pl-2 text-[12.5px] text-tertiary" title={new Date(commit.date * 1000).toLocaleString()}>{when(commit.date)}</span>
                <span class="meta-right truncate text-[12.5px] text-tertiary" title={commit.author}>· {commit.author}</span>
              </div>
            {/if}
            {#if !commit.shelved && !bare(commit)}<div class="truncate text-[12.5px]" style:color={tint(range)}>{commit.subject}</div>{/if}
            {#if pendingAt(commit) >= 0}<div class="under"><span class="skel w-[64px]"></span></div>{/if}
            {#if collapsedOf(commit)?.firstParentCount}
              {@const own = collapsedOf(commit)!}
              <!-- Under the card: opens the branch focused ("Only this branch"); turning that off comes back here. -->
              <div class="under">
                <button class="expand" id="branch-expand-{own.local}"
                  onclick={(e) => { e.stopPropagation(); beforeFocus = scroller.scrollTop; model.setFocus(own.ref, own.parent ?? model.dashboard?.base?.ref ?? null); }}>
                  {own.firstParentCount} commit{own.firstParentCount === 1 ? "" : "s"}<ChevronRight size={11} />
                </button>
                {#if squashable(own.ref)}
                  <!-- Same picker as the pill menu's "Squash commits…". -->
                  <button class="expand st" id="branch-squash-{own.local}" onclick={(e) => { e.stopPropagation(); dialog = { kind: "squash", card: own }; }}>
                    <span class="short"><Layers size={11} strokeWidth={2.5} />Squash</span><span class="more">&nbsp;· combine {own.featureCommits.length} commits into one{own.mergedIn ? ` (${own.mergedIn} came in through merges)` : ""}…</span>
                  </button>
                {/if}
              </div>
            {/if}
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

<!-- Commits collapsed, right of the card: the environments that already have the branch (✓). The missing ones go inside the card. -->
<!-- Commits collapsed: a branch's second line (behind, not merged yet, finished, uncommitted) with its actions. -->
{#snippet status(card: Card)}
  {@const d = model.dashboard}
  {@const { against, behind, done } = statusOf(card)}
  {@const stop = (action: () => void) => (e: MouseEvent) => { e.stopPropagation(); action(); }}
  <div class="status">
    {#if done}
      <span class="st">✓ Finished · safe to delete</span>
      {#if !isLite(card)}
        <button class="st act" id="branch-status-delete-{card.local}" disabled={model.isCurrent(card.ref)} onclick={stop(() => (dialog = { kind: "delete", card }))}>Delete…</button>
      {/if}
    {:else}
      {#if against && behind}
        <button class="st warn" id="branch-status-behind-{card.local}" disabled={!card.parent}
          onclick={stop(() => (dialog = { kind: "update", card }))}><span class="short">↓ {behind}</span><span class="more">&nbsp;behind {against}{#if card.parent} · Merge or rebase…{/if}</span></button>
      {/if}
      <!-- Not merged yet where it came from (or an environment): quiet grey, hideable. Click opens the PR prompt. -->
      {#if card !== d?.base && !d?.environments.includes(card)}
        {#each card.promotion.filter(([t, missing]) => missing > 0 && !hiddenNotes.includes(noteKey(card, t))) as [target, missing] (target)}
          {@const back = localName(target) === against}
          <span class="note">
            <button class="st" id="branch-status-pr-{card.local}-{localName(target)}" disabled={isLite(card)}
              title="{missing} commit{missing === 1 ? '' : 's'} not in {localName(target)} yet. Click to open a PR."
              onclick={stop(() => (dialog = { kind: "pr", card }))}><span class="short"><GitMerge size={11} strokeWidth={2.5} />{localName(target)}</span><span class="more">&nbsp;· {back ? "not merged back yet" : "not in it yet"} · PR…</span></button>
            <button class="hide" id="branch-status-hide-note-{card.local}-{localName(target)}" title="Hide this note" onclick={stop(() => hideNote(card, target))}><X size={10} strokeWidth={2.5} /></button>
          </span>
        {/each}
        <!-- Already fully in an environment (or the base): a quiet tick. -->
        {#each card.promotion.filter(([, missing]) => missing === 0) as [target] (target)}
          <span class="st" title="All its commits are in {localName(target)}">✓ merged into {localName(target)}</span>
        {/each}
      {/if}
    {/if}
    {#if card.uncommitted}
      <button class="st warn" id="branch-status-changes-{card.local}" onclick={stop(() => model.revealWorktree())}>
        {card.uncommitted.files} file{card.uncommitted.files === 1 ? "" : "s"} uncommitted<span class="do">· Review</span></button>
    {/if}
  </div>
{/snippet}

<svelte:window onmousedown={() => { menu = null; branchMenu = null; copyMenu = null; }} onkeydown={(e) => { if (e.key === "Escape") { menu = null; branchMenu = null; copyMenu = null; dialog = null; } }}
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
  {@const d = model.dashboard}
  {@const own = cardOf(name) && !isLite(cardOf(name)!) ? cardOf(name) : null}
  {@const kind = !own ? null : own === d?.base ? "base" : d?.environments.includes(own) ? "environment" : "feature"}
  {@const isCurrent = model.isCurrent(name)}
  <!-- Branches without a card (someone else's) get it without PR and squash. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div use:portal class="context-menu fit enter-pop" out:fade={{ duration: 120 }} role="menu" tabindex="-1" id="branch-pill-menu" onmousedown={(e) => e.stopPropagation()}
    style:left="{Math.min(branchMenu.x, innerWidth - 380)}px" style:top="{Math.max(8, Math.min(branchMenu.y, innerHeight - 520))}px">
    <BranchActions {name} local={localName(name)} hash={own?.last?.id ?? commit.id} isBase={kind === "base" || localName(name) === d?.base?.local} {isCurrent} {model}
      dismiss={() => (branchMenu = null)}
      onSquash={own && kind === "feature" && own.featureCommits.length > 1 && own.squashBase ? () => (dialog = { kind: "squash", card: own }) : null}
      onOpenPr={own && kind === "feature" ? () => (dialog = { kind: "pr", card: own }) : null} onNewBranch={() => (dialog = { kind: "new", from: localName(name) })} />
  </div>
{/if}
{#if writingMessage}<MessagePrompt {model} start={writingMessage === "pick" ? null : writingMessage} onclose={() => (writingMessage = null)} />{/if}
{#if menu}
  {@const c = menu.commit}
  {@const branch = branchOf(c)}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div use:portal class="context-menu enter-pop" out:fade={{ duration: 120 }} role="menu" tabindex="-1" id="commit-menu" onmousedown={(e) => e.stopPropagation()}
    style:left="{Math.min(menu.x, innerWidth - 340)}px" style:top="{Math.min(menu.y, innerHeight - (branch && branch.featureCommits.length > 1 ? 190 : 130))}px">
    <ActionRow title={copied === "metadata" ? "Copied ✓" : "Copy metadata for AI"} icon={Sparkles} color="#a78bfa" id="commit-menu-copy-metadata" onclick={() => copy("metadata", metadata(c))}
      detail="Repo, branch and commit hash: enough for an agent with git to look at it itself"
      example={'paste it in Claude Code and ask "why did this break the build?": it runs git show by itself.'} />
    <ActionRow title={copied === "hash" ? "Copied ✓" : "Copy hash"} icon={Copy} grey id="commit-menu-copy-hash" onclick={() => copy("hash", c.id)}
      detail={c.id.slice(0, 7)} />
    {#if branch && branch.featureCommits.length > 1}
      <div class="mx-2 my-1 h-px bg-border"></div>
      <ActionRow title="Squash this branch's commits…" icon={Layers} color="#ec5fa8" id="commit-menu-squash" onclick={() => { dialog = { kind: "squash", card: branch }; menu = null; }}
        detail="Combine {branch.local}'s {branch.featureCommits.length} commits into one; copies a prompt for your agent"
        example={'"wip", "fix typo", "fix again" become one clean commit: "feat: export csv".'} />
    {/if}
  </div>
{/if}

{#if dialog}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div use:portal class="dialog-backdrop enter-fade" out:fade={{ duration: 150 }} id="branch-dialog" onclick={() => (dialog = null)}>
    <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
    <div class="dialog-panel enter-pop" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      {#if dialog.kind === "new"}<StartFeature {model} from={dialog.from} dismiss={() => (dialog = null)} />
      {:else if dialog.kind === "squash"}<SquashPrompt card={dialog.card} {model} dismiss={() => (dialog = null)} />
      {:else if dialog.kind === "pr"}<PrPrompt card={dialog.card} {model} dismiss={() => (dialog = null)} />
      {:else if dialog.kind === "update"}<UpdatePanel card={dialog.card} {model} isCurrent={model.isCurrent(dialog.card.ref)} dismiss={() => (dialog = null)} />
      {:else}
        {@const card = dialog.card}
        <!-- Delete a finished branch; git itself refuses if anything isn't merged. -->
        <div class="flex w-[400px] flex-col gap-1">
          <span class="px-1.5 pt-1 text-[13px] font-semibold">Delete {card.local}?</span>
          <span class="px-1.5 pb-1 text-[12px] text-muted-foreground">Git refuses if it has commits that aren't merged, so no work can get lost.</span>
          <ActionRow title="Delete branch" detail="Only on your machine (git branch -d). The server copy stays." icon={Trash2} color="#f5a524"
            id="branch-delete-local" onclick={() => { model.deleteBranch(card.local, false); dialog = null; }} />
          {#if card.summary?.hasUpstream}
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
  .bcard.head :is(.st, .note .st, .hide) { color: rgb(255 255 255 / 0.72); }
  .bcard.head .st.warn, .row.selected .bcard.head .st.warn { color: #ffd58a; }
  .bcard.head .st:hover:not(:disabled), .bcard.head .hide:hover { color: white; }
  .bname { display: flex; align-items: center; gap: 5px; min-width: 0; font-size: 13px; font-weight: 700; color: inherit; text-align: left; }
  .bname .cloud { display: flex; flex-shrink: 0; margin-left: 3px; opacity: 0.6; }
  .bname:hover { text-decoration: underline; text-underline-offset: 3px; }
  /* Status line inside the card: quiet text; amber only for what needs you (behind, uncommitted). */
  .under { display: flex; align-items: center; gap: 18px; margin-left: 12px; white-space: nowrap; }
  .expand { display: flex; align-items: center; gap: 1px; font-size: 11.5px; color: var(--tertiary); }
  .expand:hover { color: var(--foreground); }
  /* Minimal by default (↓ 18, ⑂ main); the full sentence and its action on hover. */
  .st .short { display: flex; align-items: center; gap: 3px; }
  .st .more { max-width: 0; overflow: hidden; opacity: 0; transition: max-width 0.3s var(--ease), opacity 0.2s var(--ease); }
  .st:hover .more { max-width: 280px; opacity: 1; }
  .under .st { color: var(--tertiary); }
  .under .st:hover { color: var(--foreground); }
  .note { display: flex; flex-shrink: 0; align-items: center; gap: 2px; }
  .note .st { color: var(--tertiary); }
  .note .hide { display: grid; place-items: center; width: 14px; height: 14px; border-radius: 4px; color: var(--tertiary); opacity: 0; }
  .note:hover .hide { opacity: 1; }
  .note .hide:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.15); }
  .status { display: flex; align-items: center; gap: 18px; min-width: 0; overflow: hidden; white-space: nowrap; font-size: 11.5px; }
  .st { display: flex; flex-shrink: 0; align-items: center; gap: 1px; color: var(--muted-foreground); }
  button.st:hover:not(:disabled) { color: var(--foreground); }
  .st.warn { color: color-mix(in oklab, #f5a524 75%, var(--foreground)); }
  .st.warn:hover:not(:disabled) { color: #ffc35c; }
  /* Behind stays grey on other branches; amber only on the checked-out one or the selected row. */
  .bcard:not(.head) .st.warn { color: var(--muted-foreground); }
  .row.selected .bcard .st.warn { color: color-mix(in oklab, #f5a524 75%, var(--foreground)); }
  /* The action word takes no room until hovered, so the line stays tight. */
  .st .do { max-width: 0; overflow: hidden; opacity: 0; transition: max-width 0.2s var(--ease), opacity 0.15s, margin 0.2s var(--ease); }
  .st:hover .do { max-width: 160px; margin-left: 4px; opacity: 1; }
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
  .meta-right { transition: opacity 0.15s; }
  .row:hover:has(.quick) .meta-right { opacity: 0; } /* the quick buttons take that spot */
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
