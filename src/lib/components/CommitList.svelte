<!-- Virtualized commit list: only the rows on screen are in the DOM. Rows touch so the graph lines stay continuous. -->
<script lang="ts">
  import { Archive, Check, ChevronRight, Laptop, CircleCheck, Cloud, CloudOff, Copy, GitBranch, Sparkles, Trash2 } from "@lucide/svelte";
  import { api, WORKTREE_ID, type Card, type Commit } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import { CopyFlash } from "$lib/clipboard.svelte";
  import { plural, subject, when } from "$lib/format";
  import { isHead, isRemote, isTag, localName, refName } from "$lib/refs";
  import GraphCell, { LANE, PAD, laneColor } from "./GraphCell.svelte";
  import RefBadge from "./RefBadge.svelte";
  import ActionRow from "./ActionRow.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import Modal from "./Modal.svelte";
  import MessagePrompt from "./MessagePrompt.svelte";
  import BranchActions from "./BranchActions.svelte";
  import StartFeature from "./StartFeature.svelte";
  import UpdatePanel from "./UpdatePanel.svelte";

  let { model }: { model: RepoModel } = $props();
  const commits = $derived(model.history.commits);

  const ROW = 68; // two lines: branch badges (or the uncommitted row) above the subject
  const LINE = 44; // plain commit: date · subject · author on one line
  const QUIET = 32; // merge without refs: a short single-line row, so real commits lead
  const COLLAPSED = 64; // Commits collapsed: a branch card (52) with "N commits" beside it
  const CARD = 48;

  // Menus: right-click on a plain commit, a branch pill (or a branch row), and the quick copy button.
  let menu = $state<
    | { kind: "commit"; commit: Commit; x: number; y: number }
    | { kind: "branch"; name: string; commit: Commit; x: number; y: number }
    | { kind: "copy"; commit: Commit; branch: string; x: number; y: number }
    | null
  >(null);
  const flash = new CopyFlash(500);
  async function copy(key: string, text: Promise<string> | string) {
    await flash.copy(key, text);
    setTimeout(() => (menu = null), 500); // the ✓ shows, then the menu goes
  }

  // Centered over the graph: merge/rebase from the base, delete, new branch.
  let dialog = $state<{ kind: "update" | "delete"; card: Card } | { kind: "new"; from: string } | null>(null);
  let writingMessage = $state(false); // the "Message for AI" dialog, from right-click on the uncommitted row

  /** One badge per branch: a remote on the same commit as its local branch folds into the local badge. */
  function badges(refs: string[]) {
    const locals = new Set(refs.map(refName));
    const folded = new Set(refs.filter(isRemote).map(localName).filter((n) => locals.has(n)));
    return refs.flatMap((ref) => {
      if (isRemote(ref)) return localName(ref) === "HEAD" || folded.has(localName(ref)) ? [] : [{ ref, isSynced: false }];
      return [{ ref, isSynced: folded.has(refName(ref)) }];
    });
  }
  const branchOf = (c: Commit) => badges(c.refs).find((b) => !isTag(b.ref));

  // MARK: Branch cards (Commits collapsed): their facts, fetched only for the rows on screen.

  /** The base and the environments are what the others compare against: they stay plain pills. */
  const hasCard = (ref: string) => {
    const name = localName(refName(ref));
    return !isTag(ref) && name !== localName(model.history.base ?? "") && !model.history.environments.some((e) => localName(e) === name);
  };
  // Kept across reloads (shown until refetched, so nothing jumps); asked again after each history load.
  let cards = $state.raw(new Map<string, Card>());
  let asked: { for: unknown; refs: Set<string> } = { for: null, refs: new Set() };
  $effect(() => {
    void model.repo;
    cards = new Map(); // another repo: nothing of the previous one
  });
  $effect(() => {
    const loaded = commits, repo = model.repo, base = model.history.base;
    if (!model.branchesOnly || model.focus || !base || !repo) return;
    if (asked.for !== loaded) asked = { for: loaded, refs: new Set() };
    const refs = [...new Set(commits.slice(start, end).flatMap((c) => badges(c.refs).map((b) => refName(b.ref))))].filter((ref) => hasCard(ref) && !asked.refs.has(ref));
    if (!refs.length) return;
    const targets = [base, ...model.history.environments];
    let stale = false;
    const timer = setTimeout(async () => {
      refs.forEach((ref) => asked.refs.add(ref)); // here, not before: a scroll cancels the timer and these must be asked again
      const found = await api.branchStatus(repo, refs, base, targets).catch(() => []);
      if (stale || model.repo !== repo) return;
      const next = new Map(cards);
      for (const f of found) next.set(f.ref, { ...f, local: localName(f.ref) });
      cards = next;
    }, 250); // scrolling settles first
    return () => { stale = true; clearTimeout(timer); };
  });
  const cardOf = (ref: string): Card | null => cards.get(refName(ref)) ?? null;
  const isPending = (ref: string) => hasCard(ref) && !!model.history.base && !cardOf(ref);

  /** What a branch is compared against (the base), how far behind, and whether it's finished: every commit everywhere. */
  function statusOf(card: Card) {
    const done = card.promotion.length > 0 && card.promotion.every(([, missing]) => missing === 0);
    return { against: localName(model.history.base ?? "main"), behind: card.behindBase, done };
  }
  /** Only branches with something to say get a status line. */
  function hasStatus(card: Card) {
    const { behind, done } = statusOf(card);
    return card.firstParentCount > 0 || behind > 0 || done || card.promotion.some(([, missing]) => missing === 0);
  }

  /** Branches only: a branch row is just its cards, count, date and author, on one line (no subject). */
  const bare = (c: Commit) => model.branchesOnly && !model.focus && c.id !== WORKTREE_ID && !c.shelved;
  /** Branches only: the branch whose status line goes under the row (first pill with something to say). */
  const collapsedOf = (c: Commit) => (bare(c) ? badges(c.refs).map((b) => cardOf(b.ref)).find((card) => card != null && hasStatus(card)) ?? null : null);
  /** Commits collapsed: the badge drawn as the row's card. The one with something to say, else the one loading, else the first branch. */
  function cardIndex(c: Commit) {
    if (!bare(c)) return -1;
    const list = badges(c.refs);
    const withStatus = list.findIndex((b) => { const card = cardOf(b.ref); return card != null && hasStatus(card); });
    const pending = collapsedOf(c) ? -1 : list.findIndex((b) => isPending(b.ref));
    return withStatus >= 0 ? withStatus : pending >= 0 ? pending : list.findIndex((b) => !isTag(b.ref));
  }

  // MARK: Layout

  const isQuietMerge = (c: Commit) => c.parents.length > 1 && c.refs.length === 0;
  const isTall = (c: Commit) => c.id === WORKTREE_ID || !!c.shelved || badges(c.refs).length > 0;
  const heightOf = (c: Commit) => (isQuietMerge(c) ? QUIET : isTall(c) && !bare(c) ? ROW : cardIndex(c) >= 0 ? COLLAPSED : bare(c) ? CARD : LINE);
  const offsets = $derived.by(() => {
    const out = new Float64Array(commits.length + 1);
    commits.forEach((c, i) => (out[i + 1] = out[i] + heightOf(c)));
    return out;
  });

  let scrollTop = $state(0);
  let viewHeight = $state(600);
  let scroller: HTMLDivElement;
  /** First row whose bottom is below `y`. */
  function rowAt(y: number) {
    let lo = 0, hi = commits.length;
    while (lo < hi) {
      const m = (lo + hi) >> 1;
      if (offsets[m + 1] <= y) lo = m + 1;
      else hi = m;
    }
    return lo;
  }
  const start = $derived(Math.max(0, rowAt(scrollTop) - 5));
  const end = $derived(Math.min(commits.length, rowAt(scrollTop + viewHeight) + 6));
  $effect(() => model.setViewEnd(end)); // keeps the pages it needs, loads the next one near the end

  const width = $derived(model.graphLanes * LANE + PAD * 2);
  // Collapsed: each row's graph only as wide as its own lanes, so the card sits right next to its line.
  const last = (lanes: (unknown | null)[]) => lanes.findLastIndex((l) => l != null);
  const widthOf = (i: number) => {
    if (!model.branchesOnly || model.focus) return width;
    const row = model.history.rows[i];
    return (Math.max(last(row.before), last(row.after), row.column) + 1) * LANE + PAD * 2;
  };
  /** Subject text tinted with its branch line's color, mixed with the foreground so it stays readable. */
  const tint = (i: number) => `color-mix(in oklab, ${laneColor(model.history.rows[i].color)} 28%, var(--foreground))`;

  // Reveal request (search, coming back to a repo): bring that row to the upper third, once.
  $effect(() => {
    if (!model.scrollTo || !scroller) return;
    const index = commits.findIndex((c) => c.id === model.scrollTo?.id);
    model.takeScrollRequest();
    // The list may have just mounted: wait a frame so it has its real height to scroll in.
    if (index >= 0) requestAnimationFrame(() => (scroller.scrollTop = Math.max(0, offsets[index] - viewHeight / 3)));
  });

  // Lineage spotlight: hovering a branch's card or badge lights its whole line and fades the other lines.
  let spot = $state<number | null>(null);
  // "N commits ›" opens the focus view; closing it comes back to the same scroll position.
  let beforeFocus: number | null = null;
  $effect(() => {
    if (model.focus || model.isLoading || beforeFocus == null || !scroller) return;
    const y = beforeFocus;
    beforeFocus = null;
    requestAnimationFrame(() => (scroller.scrollTop = y));
  });
  async function openFocus(ref: string) {
    beforeFocus = scroller.scrollTop;
    await model.setFocus(ref, model.history.base);
    scroller.scrollTop = 0;
  }

  function scrollToRow(i: number) {
    if (offsets[i] < scroller.scrollTop) scroller.scrollTop = offsets[i];
    else if (offsets[i + 1] > scroller.scrollTop + viewHeight) scroller.scrollTop = offsets[i + 1] - viewHeight;
  }

  /** ↑/↓ move the selection. */
  function keydown(e: KeyboardEvent) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    const next = model.selectedIndex + (e.key === "ArrowDown" ? 1 : -1);
    if (model.selectedIndex < 0 || !commits[next]) return;
    e.preventDefault();
    model.select(commits[next].id);
    scrollToRow(next);
  }

  /** Repo, branch and hash: enough for an agent with git access, it can `git show` the rest. The branch: the one given
   *  (a clicked pill), else one pointing at it, else one containing it. */
  async function metadata(c: Commit, branch?: string) {
    const name = branch ?? (branchOf(c) ? refName(branchOf(c)!.ref) : (await api.commitPlaces(model.repo!, c.id))[0]?.name);
    return [`Repo: ${model.repo}`, name ? `Branch: ${name}` : null, `Commit: ${c.id}`].filter((line) => line != null).join("\n");
  }

  /** A pill or card: its branch menu right under it, or closed if it was already open for it. */
  function openBranchMenu(e: MouseEvent, ref: string, commit: Commit) {
    e.stopPropagation();
    const name = refName(ref);
    if (menu?.kind === "branch" && menu.name === name && menu.commit.id === commit.id) return void (menu = null);
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    menu = { kind: "branch", name, commit, x: r.left, y: r.bottom + 4 };
  }

  function rowMenu(e: MouseEvent, commit: Commit) {
    if (commit.shelved) return;
    e.preventDefault();
    if (commit.id === WORKTREE_ID) return void (writingMessage = true);
    // A row with a branch: the branch menu (same as clicking its badge); a plain commit: the commit menu.
    const branch = branchOf(commit);
    menu = branch ? { kind: "branch", name: refName(branch.ref), commit, x: e.clientX, y: e.clientY } : { kind: "commit", commit, x: e.clientX, y: e.clientY };
  }
</script>

<div bind:this={scroller} bind:clientHeight={viewHeight} onscroll={() => (scrollTop = scroller.scrollTop)} onkeydown={keydown}
  tabindex="0" role="listbox" id="commit-table" class="relative h-full overflow-auto outline-none">
  <div style:height="{offsets[commits.length]}px" class="relative">
    {#each commits.slice(start, end) as commit, k (commit.id)}
      {@const i = start + k}
      {@const selected = commit.id === model.selectedId}
      {@const branch = branchOf(commit)}
      <div class="row absolute inset-x-0 flex" class:selected class:shelved={!!commit.shelved} style:top="{offsets[i]}px" style:height="{heightOf(commit)}px"
        role="option" aria-selected={selected} tabindex="-1" id="commit-row-{commit.id}"
        onclick={() => model.select(selected ? null : commit.id)} onkeydown={(e) => e.key === "Enter" && model.select(commit.id)}
        oncontextmenu={(e) => rowMenu(e, commit)}>
        <div class="shrink-0" style:width="{widthOf(i)}px">
          <GraphCell index={i} row={model.history.rows[i]} height={heightOf(commit)} width={widthOf(i)}
            isWorktree={commit.id === WORKTREE_ID} isShelved={!!commit.shelved} isMerge={commit.parents.length > 1} isHead={commit.refs.some(isHead)}
            names={model.lineNames} grey={model.branchesOnly && !model.focus} {spot} hidden={(collapsedOf(commit)?.firstParentCount ?? 1) - 1} />
        </div>
        {#if isQuietMerge(commit)}
          <div class="flex min-w-0 items-center px-2 text-[12px] text-tertiary opacity-70"><span class="truncate">{commit.subject}</span></div>
        {:else if !isTall(commit)}
          <div class="flex min-w-0 flex-1 items-center gap-2.5 px-2 text-[12.5px] whitespace-nowrap" class:opacity-55={commit.parents.length > 1}>
            <span class="shrink-0 text-tertiary" title={new Date(commit.date * 1000).toLocaleString()}>{when(commit.date)}</span>
            <span class="min-w-0 flex-1 truncate" style:color={tint(i)} title={commit.subject}>{commit.subject}</span>
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
              <div class="truncate text-[12px] text-tertiary">{subject(commit)} · comes back when you check out {commit.shelved}</div>
            {:else}
              <!-- Branches on the left; date and author on the right edge. -->
              <div class="flex min-w-0 items-center gap-1.5 overflow-hidden whitespace-nowrap">
                {#each badges(commit.refs) as badge (badge.ref)}
                  {#if bare(commit) && !isTag(badge.ref)}
                    <!-- Commits collapsed: every branch is the same solid card, its status and actions inside when it has any
                         (placeholder bars while they load). -->
                    {@const own = cardOf(badge.ref)}
                    {@const name = localName(refName(badge.ref))}
                    {@const head = isHead(badge.ref)}
                    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
                    <div class="bcard" id="branch-card-row-{name}" onclick={(e) => openBranchMenu(e, badge.ref, commit)} onmouseenter={() => (spot = model.history.rows[i].color)}
                      onmouseleave={() => (spot = null)} oncontextmenu={(e) => { e.preventDefault(); openBranchMenu(e, badge.ref, commit); }} class:head
                      style:--lane={laneColor(model.history.rows[i].color)} aria-busy={isPending(badge.ref)}>
                      <button class="bname" id="branch-card-{name}" onclick={(e) => openBranchMenu(e, badge.ref, commit)}>
                        {#if head}<CircleCheck size={13} strokeWidth={2.5} />{:else}<GitBranch size={13} strokeWidth={2.5} />{/if}<span class="truncate">{name}</span>
                        {#if badge.isSynced || isRemote(badge.ref)}<span class="cloud" title="On the server at this commit: nothing to push or pull."><Cloud size={12} strokeWidth={2.5} /></span>
                        {:else}<span class="local" title="Only on this computer at this commit: not pushed yet, or the server has a different version."><Laptop size={11} strokeWidth={2.5} />Only local</span>{/if}
                      </button>
                      {#if own && hasStatus(own)}{@render status(own)}
                      {:else if isPending(badge.ref)}<div class="status"><span class="skel"></span><span class="skel w-[54px]"></span></div>{/if}
                    </div>
                    <!-- Right of the card: opens the branch focused (the focus view); turning that off comes back here. -->
                    {#if own && hasStatus(own) && own.firstParentCount}
                      <button class="expand" id="branch-expand-{own.local}" onclick={(e) => { e.stopPropagation(); openFocus(own.ref); }}>
                        {plural(own.firstParentCount, "commit")}<ChevronRight size={11} />
                      </button>
                    {:else if isPending(badge.ref)}<span class="skel ml-1 w-[64px]"></span>{/if}
                  {:else}
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <span class="contents" onmouseenter={() => { if (!isTag(badge.ref)) spot = model.history.rows[i].color; }} onmouseleave={() => (spot = null)}>
                      <RefBadge ref={badge.ref} isSynced={badge.isSynced} color={laneColor(model.history.rows[i].color)}
                        onclick={!isTag(badge.ref) ? (e) => openBranchMenu(e, badge.ref, commit) : undefined}
                        oncontextmenu={!isTag(badge.ref) ? (e) => { e.preventDefault(); openBranchMenu(e, badge.ref, commit); } : undefined} />
                    </span>
                  {/if}
                {/each}
                <span class="meta-right ml-auto shrink-0 pl-2 text-[12.5px] text-tertiary" title={new Date(commit.date * 1000).toLocaleString()}>{when(commit.date)}</span>
                <span class="meta-right author truncate text-[12.5px] text-tertiary" title={commit.author}>· {commit.author}</span>
              </div>
            {/if}
            {#if !commit.shelved && !bare(commit)}<div class="truncate text-[12.5px]" style:color={tint(i)}>{subject(commit)}</div>{/if}
          </div>
        {/if}
        <!-- Quick action on hover, far right: copy for AI for a branch row (commit / amend live in the commit box on the right). -->
        {#if !commit.shelved && branch}
          <div class="quick">
            <button class="icon-only" id="quick-copy-{commit.id}" title="Copy: branch name, hash, or metadata for AI"
              onclick={(e) => { e.stopPropagation(); menu = { kind: "copy", commit, branch: refName(branch.ref), x: e.clientX - 60, y: e.clientY - 23 }; }}
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
      {@const deletable = model.history.local.includes(card.local) && !model.isCurrent(card.ref)}
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
        <button class="st warn" id="branch-status-behind-{card.local}" disabled={!model.history.local.includes(card.local)}
          onclick={stop(() => (dialog = { kind: "update", card }))}><span class="short">↓ {behind}</span><span class="more">&nbsp;behind {against} · Merge or rebase…</span></button>
      {/if}
      <!-- Merged somewhere but not everywhere yet: one quiet grey note, the places on hover. -->
      {@const pending = card.promotion.filter(([, missing]) => missing > 0)}
      {#if pending.length && card.promotion.some(([, missing]) => missing === 0)}
        <span class="note">
          <span class="st" title="Not in {pending.map(([t, missing]) => `${localName(t)} (${plural(missing, 'commit')})`).join(', ')}">{#if pending.some(([t]) => localName(t) === against)}Not in {against}{pending.length > 1 ? ` · +${pending.length - 1}` : ""}{:else}Pending merge{pending.length > 1 ? ` · ${pending.length}` : ""}{/if}</span>
        </span>
      {/if}
      <!-- In no environment nor the base yet: work left halfway, worth a warning. Being merged somewhere needs no note. -->
      {#if card.firstParentCount > 0 && card.promotion.every(([, missing]) => missing > 0)}
        <span class="st alert" title="None of its commits are in {against} or any environment">Not merged anywhere</span>
      {/if}
    {/if}
  </div>
{/snippet}

{#if menu?.kind === "copy"}
  {@const { commit, branch } = menu}
  <ContextMenu x={menu.x} y={menu.y} id="copy-menu" class="w-[224px]" onclose={() => (menu = null)}>
    <button class="menu-item" id="copy-menu-metadata" onclick={() => copy("copy-metadata", metadata(commit, branch))}>
      {#if flash.copied === "copy-metadata"}<Check size={13} />{:else}<Sparkles size={13} class="text-[#a78bfa]" />{/if}Metadata for AI</button>
    <button class="menu-item" id="copy-menu-name" onclick={() => copy("copy-name", localName(branch))}>
      {#if flash.copied === "copy-name"}<Check size={13} />{:else}<GitBranch size={13} />{/if}Branch name<span class="menu-hint">{localName(branch)}</span></button>
    <button class="menu-item" id="copy-menu-hash" onclick={() => copy("copy-hash", commit.id)}>
      {#if flash.copied === "copy-hash"}<Check size={13} />{:else}<Copy size={13} />{/if}Hash<span class="menu-hint font-mono">{commit.id.slice(0, 7)}</span></button>
  </ContextMenu>
{:else if menu?.kind === "branch"}
  {@const { name, commit } = menu}
  <ContextMenu x={menu.x} y={menu.y} id="branch-pill-menu" class="branch-menu" onclose={() => (menu = null)}>
    <BranchActions {name} local={localName(name)} hash={commit.id} isBase={localName(name) === localName(model.history.base ?? "")} isCurrent={model.isCurrent(name)} {model}
      dismiss={() => (menu = null)} onNewBranch={() => (dialog = { kind: "new", from: localName(name) })} onFocus={() => openFocus(name)} />
  </ContextMenu>
{:else if menu?.kind === "commit"}
  {@const c = menu.commit}
  <ContextMenu x={menu.x} y={menu.y} id="commit-context-menu" class="w-[320px]" onclose={() => (menu = null)}>
    <ActionRow title={flash.copied === "metadata" ? "Copied ✓" : "Copy metadata for AI"} icon={Sparkles} color="#a78bfa" id="commit-menu-copy-metadata" onclick={() => copy("metadata", metadata(c))}
      detail="Repo, branch and commit hash: enough for an agent with git to look at it itself"
      example={'paste it in Claude Code and ask "why did this break the build?": it runs git show by itself.'} />
    <ActionRow title={flash.copied === "hash" ? "Copied ✓" : "Copy hash"} icon={Copy} grey id="commit-menu-copy-hash" onclick={() => copy("hash", c.id)} detail={c.id.slice(0, 7)} />
  </ContextMenu>
{/if}
{#if writingMessage}<MessagePrompt {model} onclose={() => (writingMessage = false)} />{/if}

{#if dialog}
  {@const close = () => (dialog = null)}
  <Modal id="branch-dialog" onclose={close} class="dialog-panel">
    {#if dialog.kind === "new"}<StartFeature {model} from={dialog.from} dismiss={close} />
    {:else if dialog.kind === "update"}<UpdatePanel card={dialog.card} {model} dismiss={close} />
    {:else}
      {@const card = dialog.card}
      <!-- Delete a finished branch; git itself refuses if anything isn't merged. -->
      <div class="flex w-[400px] flex-col gap-1">
        <span class="px-1.5 pt-1 text-[13px] font-semibold">Delete {card.local}?</span>
        <span class="px-1.5 pb-1 text-[12px] text-muted-foreground">Git refuses if it has commits that aren't merged, so no work can get lost.</span>
        <ActionRow title="Delete branch" detail="Only on your machine (git branch -d). The server copy stays." icon={Trash2} color="#f5a524"
          id="branch-delete-local" onclick={() => { model.deleteBranch(card.local, false); close(); }} />
        {#if card.hasUpstream}
          <ActionRow title="Delete here and on the server" detail="Also removes it from origin, for the whole team." icon={CloudOff} color="#f5a524"
            id="branch-delete-remote" onclick={() => { model.deleteBranch(card.local, true); close(); }} />
        {/if}
      </div>
    {/if}
  </Modal>
{/if}

<style>
  /* Commits collapsed: the branch as a solid card (name + status line), tinted with its line color; checked out = deeper fill. */
  .bcard {
    display: flex; flex-direction: column; justify-content: center; gap: 3px; min-width: 0; flex-shrink: 1; height: 52px; padding: 0 12px; border-radius: 12px;
    /* Pastel: a light wash of the line color, text leaning to the foreground. */
    color: color-mix(in oklab, var(--lane) 28%, var(--foreground)); background: color-mix(in oklab, var(--lane) 7%, var(--background));
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--lane) 16%, transparent);
    transition: background 0.15s, box-shadow 0.15s, color 0.15s;
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
  .bcard:not(.head):hover {
    color: color-mix(in oklab, var(--lane) 45%, var(--foreground)); background: color-mix(in oklab, var(--lane) 12%, var(--background));
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--lane) 38%, transparent);
  }
  .bcard.head:hover { filter: brightness(1.12); }
  .expand { display: flex; flex-shrink: 0; align-items: center; gap: 1px; margin-left: 6px; white-space: nowrap; font-size: 11.5px; color: var(--tertiary); }
  .expand:hover { color: var(--foreground); }
  /* Status line inside the card: quiet text; amber only for what needs you (behind, uncommitted). Minimal by default
     (↓ 18), the full sentence and its action on hover. */
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
  .local { display: flex; flex-shrink: 0; align-items: center; gap: 4px; margin-left: 6px; font-size: 11.5px; font-weight: 600; color: #f5a524; }
  .st.alert, .bcard.head .st.alert { color: #f5a524; font-weight: 600; }
  /* Behind stays grey on other branches; amber only on the checked-out one or the selected row. */
  .bcard:not(.head) .st.warn { color: var(--muted-foreground); }
  .row.selected .bcard .st.warn { color: color-mix(in oklab, #f5a524 75%, var(--foreground)); }
  :global(.branch-menu) { padding: 10px; border-radius: 14px; } /* BranchActions sets its own width */
  .skel { display: inline-block; width: 34px; height: 9px; border-radius: 999px; background: oklch(0.5 0 0 / 0.16); animation: skel 1.2s ease-in-out infinite; }
  @keyframes skel { 50% { opacity: 0.45; } }
  @media (prefers-reduced-motion: reduce) { .skel { animation: none; } }
  .row { transition: background-color 0.15s ease; }
  .row.shelved { background: color-mix(in oklab, oklch(0.8 0.15 75) 7%, transparent); }
  .row:hover { background: color-mix(in oklab, var(--quinary) 50%, transparent); }
  .row.selected { background: var(--quinary); } /* faint tint instead of the strong system blue */
  :global(.dialog-panel) { padding: 10px; border-radius: 12px; }
  .quick { position: absolute; top: 50%; right: 10px; display: flex; gap: 6px; opacity: 0; pointer-events: none; translate: 4px -50%; transition: opacity 0.15s, translate 0.15s; }
  .meta-right.author { transition: margin-right 0.15s; }
  .row:hover:has(.quick) .meta-right.author { margin-right: 44px; } /* slides left to make room for the copy button */
  .row:hover .quick { opacity: 1; pointer-events: auto; translate: 0 -50%; }
  .quick button { display: flex; align-items: center; justify-content: center; width: 26px; height: 26px; border-radius: 999px;
    color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.1); transition: color 0.15s, background 0.15s; }
  .quick button:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.2); }
</style>
