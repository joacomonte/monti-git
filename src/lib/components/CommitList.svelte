<!-- Virtualized commit list: only the rows on screen are in the DOM. Rows touch so the graph lines stay continuous. -->
<script lang="ts">
  import { Archive, ArrowDown, ArrowUp, ArchiveRestore, CircleCheck, Clock, Cloud, Copy, Laptop, GitCommitHorizontal, GitPullRequestDraft, PackageOpen, Sparkles, Trash2 } from "@lucide/svelte";
  import { api, WORKTREE_ID, type Card, type Commit, type GithubStatus } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import { CopyFlash } from "$lib/clipboard.svelte";
  import { plural, subject, when } from "$lib/format";
  import { branchBadges, isHead, isRemote, isTag, localName, refName, unpushed } from "$lib/refs";
  import { mergeState } from "$lib/cards";
  import GraphCell, { LANE, PAD, laneColor } from "./GraphCell.svelte";
  import RefBadge from "./RefBadge.svelte";
  import ActionRow from "./ActionRow.svelte";
  import DeleteBranch from "./DeleteBranch.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import Modal from "./Modal.svelte";
  import BranchActions from "./BranchActions.svelte";
  import StartFeature from "./StartFeature.svelte";
  import UpdatePanel from "./UpdatePanel.svelte";
  import PrBadge from "./PrBadge.svelte";
  import DeployStatus from "./DeployStatus.svelte";
  import EditCommits from "./EditCommits.svelte";

  let { model }: { model: RepoModel } = $props();
  const commits = $derived(model.history.commits);

  const ROW = 68; // two lines: branch badges (or the uncommitted row) above the subject; in Commits collapsed, every branch card
  const LINE = 44; // plain commit: date · subject · author on one line
  const QUIET = 32; // merge without refs: a short single-line row, so real commits lead

  // Menus: right-click on a plain commit, a branch pill (or a branch row).
  let menu = $state<
    | { kind: "commit"; commit: Commit; x: number; y: number }
    | { kind: "branch"; name: string; commit: Commit; x: number; y: number }
    | { kind: "stash"; commit: Commit; x: number; y: number }
    | { kind: "worktree"; commit: Commit; x: number; y: number }
    | null
  >(null);
  const flash = new CopyFlash(500);
  async function copy(key: string, text: Promise<string> | string) {
    await flash.copy(key, text);
    setTimeout(() => (menu = null), 500); // the ✓ shows, then the menu goes
  }

  // Centered over the graph: merge/rebase from the base, delete, new branch.
  let dialog = $state<{ kind: "update" | "delete"; card: Card } | { kind: "new"; from: string } | { kind: "edit"; from: string } | null>(null);

  const branchOf = (c: Commit) => branchBadges(c.refs).find((b) => !isTag(b.ref));

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
    const refs = [...new Set(commits.slice(start, end).flatMap((c) => branchBadges(c.refs).map((b) => refName(b.ref))))].filter((ref) => hasCard(ref) && !asked.refs.has(ref));
    if (!refs.length) return;
    const targets = model.targets;
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

  // GitHub (pull requests and deploys): asked again with each history load, and every 5 s while a deploy runs.
  const NO_GITHUB: GithubStatus = { pullRequests: {}, deploys: [] };
  let github = $state.raw<GithubStatus>(NO_GITHUB);
  $effect(() => {
    const repo = model.repo;
    void model.history;
    if (!repo) return;
    let stale = false, timer: ReturnType<typeof setTimeout> | undefined;
    const load = () => api.githubStatus(repo).then((found) => {
      if (stale || model.repo !== repo) return;
      github = found;
      if (found.deploys.some((d) => d.state === "queued" || d.state === "running")) timer = setTimeout(load, 5000);
    }, () => {});
    load();
    return () => { stale = true; clearTimeout(timer); };
  });
  $effect(() => {
    void model.repo;
    github = NO_GITHUB; // another repo: nothing of the previous one's
  });
  /** The deploy of the commit a card points at: running first, else the newest. Never another commit's. */
  const deployOf = (sha: string) => {
    const mine = github.deploys.filter((d) => d.sha === sha);
    return mine.find((d) => d.state === "queued" || d.state === "running") ?? mine[0] ?? null;
  };
  const isPending = (ref: string) => hasCard(ref) && !!model.history.base && !cardOf(ref);

  /** Only branches with something to say get status icons. */
  const hasStatus = (card: Card) => card.firstParentCount > 0 || card.behindBase > 0 || mergeState(card) != null;

  /** Branches only: a branch row is just its cards, count, date and author, on one line (no subject). */
  const bare = (c: Commit) => model.branchesOnly && !model.focus && c.id !== WORKTREE_ID && !c.stash;
  /** Branches only: the branch whose status line goes under the row (first pill with something to say). */
  const collapsedOf = (c: Commit) => (bare(c) ? branchBadges(c.refs).map((b) => cardOf(b.ref)).find((card) => card != null && hasStatus(card)) ?? null : null);


  // MARK: Layout

  const isQuietMerge = (c: Commit) => c.parents.length > 1 && c.refs.length === 0;
  const isTall = (c: Commit) => c.id === WORKTREE_ID || !!c.stash || branchBadges(c.refs).length > 0;
  const heightOf = (c: Commit) => (isQuietMerge(c) ? QUIET : isTall(c) || bare(c) ? ROW : LINE);
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
    const base = model.history.base;
    // The base focused on its own: its line, with nothing to compare it against.
    await model.setFocus(ref, base && localName(base) === localName(ref) ? null : base);
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

  /** The uncommitted row for an AI agent: where, on which branch, how much, and the commands that show it all. */
  function uncommitted(c: Commit) {
    const branch = model.history.current;
    return [`Repo: ${model.repo}`, `Branch: ${branch ?? "(detached HEAD)"}`, `Uncommitted changes: ${plural(c.stat?.files ?? 0, "file")}, not committed yet`,
      "See them with: git status, git diff HEAD (staged and unstaged together)"].join("\n");
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
    e.preventDefault();
    if (commit.stash) return void (menu = { kind: "stash", commit, x: e.clientX, y: e.clientY });
    if (commit.id === WORKTREE_ID) return void (menu = { kind: "worktree", commit, x: e.clientX, y: e.clientY });
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
      <div class="row absolute inset-x-0 flex" class:selected class:stash={!!commit.stash} style:top="{offsets[i]}px" style:height="{heightOf(commit)}px"
        role="option" aria-selected={selected} tabindex="-1" id="commit-row-{commit.id}"
        onclick={() => model.select(selected ? null : commit.id)} onkeydown={(e) => e.key === "Enter" && model.select(commit.id)}
        oncontextmenu={(e) => rowMenu(e, commit)}>
        <div class="shrink-0" style:width="{widthOf(i)}px">
          <GraphCell index={i} row={model.history.rows[i]} height={heightOf(commit)} width={widthOf(i)}
            isWorktree={commit.id === WORKTREE_ID} isStash={!!commit.stash} isMerge={commit.parents.length > 1} isHead={commit.refs.some(isHead)}
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
            {:else if commit.stash}
              <!-- Right-click (or the row's menu) applies, pops or drops it. -->
              {@const shelved = commit.stash.shelved}
              <div class="flex min-w-0 items-center gap-1.5 overflow-hidden whitespace-nowrap text-[13px]">
                <Archive size={13} class="shrink-0 text-amber-400" />
                <span class="font-semibold text-amber-400">{shelved ? "WIP saved" : "Stash"}</span>
                <span class="truncate text-muted-foreground">{shelved ? `from ${shelved}` : commit.subject}</span>
                <span class="shrink-0 text-[12.5px] text-tertiary" title={new Date(commit.date * 1000).toLocaleString()}>· saved {when(commit.date)}</span>
              </div>
              <div class="truncate text-[12px] text-tertiary">{subject(commit)} · {shelved ? `comes back when you check out ${shelved}` : "right-click to apply or drop it"}</div>
            {:else}
              <!-- Branches on the left; date and author on the right edge. -->
              <div class="flex min-w-0 items-center gap-1.5 overflow-hidden whitespace-nowrap">
                {#each branchBadges(commit.refs) as badge (badge.ref)}
                  {#if bare(commit) && !isTag(badge.ref)}
                    <!-- Commits collapsed: every branch is the same solid card, its status and actions inside when it has any
                         (placeholder bars while they load). -->
                    {@const own = cardOf(badge.ref)}
                    {@const name = localName(refName(badge.ref))}
                    {@const head = isHead(badge.ref)}
                    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
                    <!-- The card's body opens its menu; its status icons (Safe to delete, behind…) do their own thing. -->
                    <div class="bcard" id="branch-card-row-{name}" onclick={(e) => { if (!(e.target as Element).closest(".status button")) openBranchMenu(e, badge.ref, commit); }} onmouseenter={() => (spot = model.history.rows[i].color)}
                      onmouseleave={() => (spot = null)} oncontextmenu={(e) => { e.preventDefault(); openBranchMenu(e, badge.ref, commit); }} class:head
                      style:--lane={laneColor(model.history.rows[i].color)} aria-busy={isPending(badge.ref)}>
                      <button class="bname" id="branch-card-{name}" onclick={(e) => openBranchMenu(e, badge.ref, commit)}>
                        {#if head}<CircleCheck size={13} strokeWidth={2.5} />{/if}<span class="truncate">{name}</span>
                        {#if badge.isSynced || isRemote(badge.ref)}<span class="cloud" title="On the server at this commit: nothing to push or pull."><Cloud size={12} strokeWidth={2.5} /></span>
                        {:else}<span class="laptop" title={own && !own.hasUpstream ? "Only on this computer: never pushed. Right-click → Push" : "Not on the server at this commit. Right-click → Push"}><Laptop size={12} strokeWidth={2.5} /></span>{/if}
                      </button>
                      <div class="status">
                        {#if own && hasStatus(own)}{@render status(own)}{:else if isPending(badge.ref)}<span class="skel"></span>{/if}
                      </div>
                      {#if deployOf(commit.id)}<DeployStatus deploy={deployOf(commit.id)!} id="branch-deploy-{name}" />{/if}
                    </div>
                    {#if github.pullRequests[name]}<PrBadge pr={github.pullRequests[name]} id="branch-pr-{name}" />{/if}
                    <!-- Right of the card: its own commits as a count (amber part: not pushed yet); hover says so, click opens the focus view. -->
                    {#if own?.firstParentCount}
                      {@const local = own.unpushed}
                      <!-- None pushed: one number, all of them waiting to go up; otherwise its commits, and how many of them aren't up yet. -->
                      {@const allLocal = local === own.firstParentCount}
                      <button class="commits" class:pending={local > 0} id="branch-commits-{own.local}" onclick={(e) => { e.stopPropagation(); openFocus(own.ref); }}
                        title={allLocal
                          ? `${plural(local, "commit")} not pushed yet\nOnly on this computer: none of its commits is on the server\nClick: focus view, just this branch`
                          : `${plural(own.firstParentCount, "commit")} of its own\n${local ? `${local} of them not pushed yet` : "All of them pushed"}\nClick: focus view, just this branch`}>
                        {#if allLocal}
                          <span class="unpushed only"><ArrowUp size={12} strokeWidth={2.75} />{local}</span>
                        {:else}
                          <GitCommitHorizontal size={13} strokeWidth={2.5} />{own.firstParentCount}
                          {#if local}<span class="unpushed"><ArrowUp size={11} strokeWidth={2.75} />{local}</span>{/if}
                        {/if}
                      </button>
                    {/if}
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
            {#if !commit.stash && !bare(commit)}<div class="truncate text-[12.5px]" style:color={tint(i)}>{subject(commit)}</div>{/if}
          </div>
        {/if}
      </div>
    {/each}
  </div>
</div>

<!-- Commits collapsed: a branch's second line (behind, not merged yet, merged into, finished) with its actions. -->
{#snippet status(card: Card)}
  {@const against = localName(model.history.base ?? "main")}
  {@const behind = card.behindBase}
  {@const merge = mergeState(card)}
  {@const stop = (action: () => void) => (e: MouseEvent) => { e.stopPropagation(); action(); }}
  {#if merge?.kind === "done"}
    <!-- How it got there: the weakest way across targets in the label, each target's way in the tooltip. -->
    {@const ways = card.promotion.map(([, , how]) => how)}
    {@const label = ways.includes("squashed") ? "Squashed" : ways.includes("cherry-picked") ? "Cherry-picked" : ways.includes("fast-forwarded") ? "Fast-forwarded" : "Merged"}
    {@const explain = { merged: "merged", "fast-forwarded": "fast-forwarded: the base moved up to this branch, no merge commit", "cherry-picked": "same changes copied (cherry-pick or rebase)", squashed: "squashed: its contents are already there", "": "" }}
    {@const how = card.promotion.map(([t, , how]) => `${localName(t)}: ${explain[how]}`).join("\n")}
    <button class="st ok" id="branch-status-delete-{card.local}" title={`${label} · safe to delete\n${how}\nClick: checks with the server, then shows the options`}
      onclick={stop(() => (dialog = { kind: "delete", card }))}><CircleCheck size={12} strokeWidth={2.5} /><Trash2 size={11} strokeWidth={2.5} /></button>
  {:else}
    {#if behind}
      <button class="st warn" id="branch-status-behind-{card.local}" disabled={!model.history.local.includes(card.local)} title={`${plural(behind, "commit")} behind ${against}\nTeammates added them after this branch was cut\nClick: merge or rebase to bring them in`}
        onclick={stop(() => (dialog = { kind: "update", card }))}><ArrowDown size={12} strokeWidth={2.5} />{behind}</button>
    {/if}
    <!-- Merged somewhere but not everywhere yet: how many places are missing, which on hover. -->
    {#if merge?.kind === "pending"}
      {@const counts = card.promotion.filter(([, missing]) => missing > 0).map(([t, missing]) => `${localName(t)} (${plural(missing, "commit")})`)}
      <span class="st" title={`Not in ${merge.missing.join(", ")} yet\nMissing: ${counts.join(", ")}`}><Clock size={12} strokeWidth={2.5} />{merge.missing.length}</span>
    {/if}
    <!-- In no environment nor the base yet: work left halfway, worth a warning. -->
    {#if merge?.kind === "none"}
      <span class="st amber" title={`Not merged anywhere\nNone of its commits are in ${against} or any environment yet`}><GitPullRequestDraft size={12} strokeWidth={2.5} /></span>
    {/if}
  {/if}
{/snippet}

{#if menu?.kind === "branch"}
  {@const { name, commit } = menu}
  <ContextMenu x={menu.x} y={menu.y} id="branch-pill-menu" class="branch-menu" onclose={() => (menu = null)}>
    <BranchActions {name} local={localName(name)} hash={commit.id} isBase={localName(name) === localName(model.history.base ?? "")} isCurrent={model.isCurrent(name)} canPush={unpushed(commit.refs).includes(name)} {model}
      dismiss={() => (menu = null)} onNewBranch={() => (dialog = { kind: "new", from: localName(name) })} onFocus={() => openFocus(name)}
      onEdit={() => (dialog = { kind: "edit", from: localName(name) })} />
  </ContextMenu>
{:else if menu?.kind === "stash"}
  {@const stash = menu.commit}
  {@const done = (action: () => void) => () => { menu = null; action(); }}
  <ContextMenu x={menu.x} y={menu.y} id="stash-context-menu" class="w-[320px]" onclose={() => (menu = null)}>
    <ActionRow title="Apply" detail="Put its changes back in your files, and keep it saved" icon={ArchiveRestore} color="#2fc4b2" id="stash-menu-apply"
      disabled={model.isSyncing} onclick={done(() => model.applyStash(stash, false))} />
    <ActionRow title="Pop" detail="Put its changes back, then drop it" icon={PackageOpen} color="#4f8cff" id="stash-menu-pop"
      disabled={model.isSyncing} onclick={done(() => model.applyStash(stash, true))} />
    <ActionRow title="Drop" detail="Delete it (Undo from Logs puts it back)" icon={Trash2} color="#f43f5e" id="stash-menu-drop"
      disabled={model.isSyncing} onclick={done(() => model.dropStash(stash))} />
  </ContextMenu>
{:else if menu?.kind === "worktree"}
  {@const c = menu.commit}
  <ContextMenu x={menu.x} y={menu.y} id="worktree-context-menu" class="w-[320px]" onclose={() => (menu = null)}>
    <ActionRow title={flash.copied === "uncommitted" ? "Copied ✓" : "Copy context for AI"} icon={Sparkles} color="#a78bfa" id="worktree-menu-copy-ai" onclick={() => copy("uncommitted", uncommitted(c))}
      detail="Repo, branch and how to see these changes: paste it in your agent"
      example={'paste it in Claude Code and ask "review these changes before I commit".'} />
    <ActionRow title="Commit…" detail="Open the commit form, on the right" icon={GitCommitHorizontal} color="#4f8cff" id="worktree-menu-commit"
      onclick={() => { menu = null; model.select(c.id); }} />
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

{#if dialog}
  {@const close = () => (dialog = null)}
  <Modal id="branch-dialog" onclose={close} class="dialog-panel">
    {#if dialog.kind === "new"}<StartFeature {model} from={dialog.from} dismiss={close} />
    {:else if dialog.kind === "edit"}<EditCommits {model} branch={dialog.from} dismiss={close} />
    {:else if dialog.kind === "update"}<UpdatePanel card={dialog.card} {model} dismiss={close} />
    {:else}<DeleteBranch {model} branch={dialog.card.local} dismiss={close} />
    {/if}
  </Modal>
{/if}

<style>
  /* Commits collapsed: the branch as a solid card (name + status line), tinted with its line color; checked out = deeper fill. */
  .bcard {
    position: relative; /* the deploy bar runs along its bottom edge */
    display: flex; align-items: center; gap: 8px; min-width: 0; flex-shrink: 1; height: 40px; padding: 0 12px; border-radius: 12px;
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
  .bcard.head .st:not(.amber, .ok, .warn) { color: rgb(255 255 255 / 0.75); }
  .bname { display: flex; align-items: center; gap: 5px; min-width: 0; font-size: 13px; font-weight: 700; color: inherit; text-align: left; }
  .bname .cloud { display: flex; flex-shrink: 0; margin-left: 3px; opacity: 0.6; }
  .bname .laptop { display: flex; flex-shrink: 0; margin-left: 3px; color: #f5a524; } /* amber: work only on this computer */
  /* Hover: the card glows a little in its own color (no underline). */
  .bcard:not(.head):hover {
    color: color-mix(in oklab, var(--lane) 45%, var(--foreground)); background: color-mix(in oklab, var(--lane) 12%, var(--background));
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--lane) 38%, transparent);
  }
  .bcard.head:hover { filter: brightness(1.12); }
  /* Right of the card, apart from it: the commit count; the amber part (↑ N) is what isn't pushed. Hover unfolds the word. */
  .commits { display: flex; flex-shrink: 0; align-items: center; gap: 4px; height: 24px; margin-left: 10px; padding: 0 8px; border-radius: 99px;
    font-size: 11.5px; font-weight: 600; color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.12); }
  .commits:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.2); }
  .commits .unpushed { display: flex; align-items: center; gap: 1px; margin-left: 2px; padding-left: 6px; border-left: 1px solid oklch(0.5 0 0 / 0.3); color: #f5a524; }
  .commits .unpushed.only { margin-left: 0; padding-left: 0; border-left: none; gap: 2px; }
  /* Status icons beside the name: just the icon (and its count); hovering one unfolds what it means and its action. */
  .status { display: flex; flex-shrink: 0; align-items: center; gap: 4px; white-space: nowrap; font-size: 11.5px; font-weight: 600; }
  .st { display: flex; flex-shrink: 0; align-items: center; gap: 3px; height: 22px; padding: 0 6px; border-radius: 99px; color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.14); }
  button.st:hover:not(:disabled) { color: var(--foreground); }
  .st.amber { color: #f5a524; background: color-mix(in oklab, #f5a524 15%, transparent); }
  .st.ok { color: #34c77b; background: color-mix(in oklab, #34c77b 15%, transparent); }
  .st.ok:hover { color: #34c77b; }
  /* Behind stays grey on other branches; amber on the checked-out one or the selected row. */
  .bcard.head .st.warn, .row.selected .bcard .st.warn { color: #f5a524; }
  .bcard.head .st { background: rgb(0 0 0 / 0.22); }
  :global(.branch-menu) { padding: 10px; border-radius: 14px; } /* BranchActions sets its own width */
  .skel { display: inline-block; width: 34px; height: 9px; border-radius: 999px; background: oklch(0.5 0 0 / 0.16); animation: skel 1.2s ease-in-out infinite; }
  @keyframes skel { 50% { opacity: 0.45; } }
  @media (prefers-reduced-motion: reduce) { .skel { animation: none; } }
  .row { transition: background-color 0.15s ease; }
  .row.stash { background: color-mix(in oklab, oklch(0.8 0.15 75) 7%, transparent); }
  .row:hover { background: color-mix(in oklab, var(--quinary) 50%, transparent); }
  .row.selected { background: var(--quinary); } /* faint tint instead of the strong system blue */
  :global(.dialog-panel) { padding: 10px; border-radius: 12px; }
</style>
