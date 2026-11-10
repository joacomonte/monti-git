<!-- The dashboard: in every repo of the sidebar, the branches with commits in the last days, newest first across every
     repo (a repo's title heads each run of its branches, so mixed repos stay clear), one card per row with every detail in words and its actions on the right. The next thing to do comes
     first: Commit the uncommitted changes, else Push what's committed; Safe to delete shows but stays quiet. -->
<script lang="ts" module>
  import type { RepoActivity as Activity } from "$lib/api";
  // Last result per window and filter, kept across visits: coming back shows it right away while it refreshes.
  const cache = new Map<string, Activity[]>();
</script>

<script lang="ts">
  import { ArrowDown, ArrowRight, Sparkles, CircleCheck, Clock, Cloud, ChevronRight, CloudUpload, Copy, Crosshair, PenLine, UserRound, Waypoints } from "@lucide/svelte";
  import { api, WORKTREE_ID, type ActiveBranch, type RepoActivity } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import { ago, baseName, plural } from "$lib/format";
  import { prefs } from "$lib/prefs";
  import { isRemote, localName } from "$lib/refs";
  import { mergeState } from "$lib/cards";
  import ContextMenu from "./ContextMenu.svelte";
  import { fly } from "svelte/transition";
  import { laneColor } from "./GraphCell.svelte";
  import Loader from "./Loader.svelte";
  import Segmented from "./Segmented.svelte";

  let { model }: { model: RepoModel } = $props();
  const DAYS = [1, 3, 7];
  const saved = prefs.get("dashboardDays", 7);
  let days = $state(DAYS.includes(saved) ? saved : 7); // a window saved by an older version falls back to a week
  let onlyMine = $state(true); // always starts on: off is a quick look at everyone's, not remembered
  const key = $derived(`${days}:${onlyMine}`);
  // svelte-ignore state_referenced_locally
  let activity = $state.raw<RepoActivity[] | null>(cache.get(`${days}:${onlyMine}`) ?? null); // the cache, before the first frame
  let refreshing = $state(false); // a thin bar on top while it reads the repos again; what's shown stays usable
  let version = $state(0); // bumped after a push from here, to read the repos again
  let sub = $state<"safe" | null>(null); // a sub view: "Dashboard › Safe to delete"
  let menu = $state<{ group: RepoActivity; branch: ActiveBranch; x: number; y: number } | null>(null);

  // Reads the repos on every visit and when the window, the filter or the sidebar's repos change; a newer request wins.
  // The cached result for that window shows meanwhile (the full loader only the first time).
  $effect(() => {
    const repos = model.repos.map((r) => model.picks(r.path)), window = days, mine = onlyMine, at = key;
    void version;
    let stale = false;
    activity = cache.get(at) ?? null;
    refreshing = true;
    api.recentActivity(repos, window, mine).then(
      (found) => { cache.set(at, found); if (!stale) activity = found; },
      () => { if (!stale && !activity) activity = []; },
    ).finally(() => { if (!stale) refreshing = false; });
    return () => (stale = true);
  });

  function setOnlyMine(on: boolean) {
    onlyMine = on;
  }
  function setDays(next: number) {
    days = next;
    prefs.set("dashboardDays", next);
  }

  // Each repo keeps its color: its place in the sidebar list.
  const colorOf = (repo: string) => laneColor(Math.max(0, model.repos.findIndex((r) => r.path === repo)) + 1);
  const hasWork = (g: RepoActivity, b: ActiveBranch) => g.changed > 0 && localName(b.name) === g.current;
  // Every repo's branches in one list, newest first (uncommitted work counts as now); each run of the same repo gets its title.
  const runs = $derived.by(() => {
    const when = (g: RepoActivity, b: ActiveBranch) => (hasWork(g, b) ? Infinity : b.date);
    const all = (activity ?? []).flatMap((group) => group.branches.map((b) => ({ group, b })));
    all.sort((x, y) => when(y.group, y.b) - when(x.group, x.b));
    const out: { group: RepoActivity; branches: ActiveBranch[] }[] = [];
    for (const { group, b } of all) {
      const last = out.at(-1);
      if (last?.group === group) last.branches.push(b);
      else out.push({ group, branches: [b] });
    }
    return out;
  });

  /** Where its work is, in words: everywhere, partly (and where it's still missing), or nowhere yet. */
  function merge(b: ActiveBranch) {
    const m = b.status && mergeState(b.status);
    if (!m) return null;
    const text = { done: `Merged into ${m.done.join(", ")}`, pending: `In ${m.done.join(", ")} · not in ${m.missing.join(", ")} yet`, none: "Not merged anywhere" }[m.kind];
    return { kind: m.kind, text };
  }

  /** The next thing to do on your machine, one at a time: Commit the uncommitted work; only once that's done, Push
   *  the commits not on the server. */
  function pending(g: RepoActivity, b: ActiveBranch): ("commit" | "push")[] {
    if (hasWork(g, b)) return ["commit"];
    return !isRemote(b.name) && (b.status?.unpushed ?? 0) > 0 ? ["push"] : [];
  }
  /** Each action's count with its unit, under its name ("73 files", "5 commits"), and what it means (tooltip, menu hint). */
  function detail(kind: "commit" | "push", g: RepoActivity, b: ActiveBranch) {
    if (kind === "commit") return { count: plural(g.changed, "file"), text: `${plural(g.changed, "changed file")} not committed` };
    const n = b.status?.unpushed ?? 0;
    return { count: plural(n, "commit"), text: `${plural(n, "commit")} not pushed${b.status?.hasUpstream ? "" : ": the branch was never pushed"}` };
  }

  function openGraph(repo: string, b: ActiveBranch) {
    model.revealCommit(repo, b.tip);
  }
  async function focus(g: RepoActivity, b: ActiveBranch) {
    model.open(g.repo, "graph");
    await model.setFocus(b.name, g.base, "dashboard");
  }
  /** The commit box lives on the graph's uncommitted row. */
  function commit(repo: string) {
    model.open(repo, "graph");
    model.select(WORKTREE_ID);
  }
  /** Pushes from here (the toast and Logs show it), then reads the repos again. */
  async function push(repo: string, b: ActiveBranch) {
    model.open(repo, "dashboard");
    await model.push(localName(b.name));
    version++;
  }

  const ACTIONS = {
    commit: { label: "Commit", icon: PenLine, run: (g: RepoActivity) => commit(g.repo) },
    push: { label: "Push", icon: CloudUpload, run: (g: RepoActivity, b: ActiveBranch) => push(g.repo, b) },
  } as const;
</script>

<!-- The refresh bar floats over the top edge, outside the scrolling list: showing it never moves anything (no layout shift). -->
<div class="relative h-full">
{#if refreshing && activity}<div class="refreshing" title="Checking every repo for anything new"></div>{/if}
<div class="dashboard enter-fade" id="dashboard">
  <!-- Title (a breadcrumb in a sub view), the filters under it. -->
  <div class="head">
    <div class="crumbs">
      {#if sub}<button class="crumb" id="dashboard-crumb-home" onclick={() => (sub = null)}>Dashboard</button><ChevronRight size={18} class="text-tertiary" /><span class="title" in:fly={{ x: 12, duration: 220 }}>Safe to delete</span>
      {:else}<span class="title">Dashboard</span>{/if}
    </div>
    <div class="filters">
      {#if !sub}
        <span class="hint">Branches with commits in the last</span>
        <Segmented id="dashboard-days" value={String(days)} onchange={(v) => setDays(Number(v))}
          options={DAYS.map((d) => ({ value: String(d), label: d === 1 ? "24 h" : `${d} days` }))} />
      {/if}
      <button class="tool" class:on={onlyMine} aria-pressed={onlyMine} id="dashboard-only-mine" onclick={() => setOnlyMine(!onlyMine)}
        title="My branches: only those whose last commit is yours. Off: everyone's."><UserRound size={14} />My branches</button>
    </div>
  </div>

  {#if sub === "safe"}
    <!-- Loaded only when opened: its code and its (heavier) read of every branch. -->
    {#await import("./SafeToDelete.svelte") then { default: SafeToDelete }}<SafeToDelete {model} {onlyMine} {colorOf} />{/await}
  {:else if activity == null}
    <Loader title="Looking at every repo" detail="Branches with recent commits, and where each one is merged" />
  {:else if !activity.length}
    <div class="empty" id="dashboard-empty"><Waypoints size={28} strokeWidth={1.5} />No {onlyMine ? "branch of yours" : "branch"} has commits in the last {days === 1 ? "24 hours" : `${days} days`}.</div>
  {:else}
    {#each runs as { group, branches }, i (i)}
      <section class="repo" style:--repo={colorOf(group.repo)}>
        <h3><button class="repo-title" id="dashboard-repo-{baseName(group.repo)}" title="Open {baseName(group.repo)} in the graph"
          onclick={() => model.open(group.repo, "graph")}><span class="dot"></span>{baseName(group.repo)}<ArrowRight size={18} class="go" /></button></h3>
        <div class="cards">
          {#each branches as b (b.name)}
            {@const s = b.status}
            {@const m = merge(b)}
            {@const todo = pending(group, b)}
            {@const remoteOnly = isRemote(b.name)}
            <!-- The card opens the branch's focus view; its buttons do their own thing. -->
            <div class="card" id="dashboard-card-{baseName(group.repo)}-{b.name}" title="Open its focus view" role="button" tabindex="0"
              onclick={(e) => { if (!(e.target as Element).closest(".actions")) focus(group, b); }}
              onkeydown={(e) => { if (e.key === "Enter" && e.target === e.currentTarget) focus(group, b); }}
              oncontextmenu={(e) => { e.preventDefault(); menu = { group, branch: b, x: e.clientX, y: e.clientY }; }}>
              <div class="body">
                <div class="top">
                  <span class="name">{localName(b.name)}</span>
                  <!-- What's not pushed is in the "to push" line below; here only where it lives. -->
                  {#if remoteOnly}<span class="where"><Cloud size={13} />Only on the server</span>
                  {:else if s?.hasUpstream && !s.unpushed}<span class="where"><Cloud size={13} />Pushed</span>{/if}
                </div>
                <div class="last">{b.subject} <span class="meta">· {b.author} · {ago(b.date)}</span></div>
                <!-- Only where it already is (merged, or partly); not being merged anywhere yet needs no line. -->
                {#if m && m.kind !== "none"}
                  <div class="line" class:ok={m.kind === "done"}>
                    <span>{#if m.kind === "done"}<CircleCheck size={14} />{:else}<Clock size={14} />{/if}{m.text}</span>
                  </div>
                {/if}
                {#if s?.behindBase}
                  <div class="line"><span><ArrowDown size={14} />{s.behindBase} behind {localName(group.base ?? "the base")}</span></div>
                {/if}
              </div>
              <!-- Right: what's waiting on your machine (Commit, Push), and Safe to delete when it applies; right-click has every option. -->
              <div class="actions">
                {#each todo as kind (kind)}
                  {@const a = ACTIONS[kind]}
                  {@const d = detail(kind, group, b)}
                  <button class="act {kind}" id="dashboard-{kind}-{b.name}" title={d.text} onclick={() => a.run(group, b)}>
                    <a.icon size={20} />{a.label}<span class="count-line">{d.count}</span>
                  </button>
                {/each}
                {#if m?.kind === "done"}
                  <button class="act quiet-ok" id="dashboard-delete-{b.name}" title="Opens it in the graph: its ✓ there checks with the server and offers the options"
                    onclick={() => openGraph(group.repo, b)}><CircleCheck size={20} />Safe to delete</button>
                {/if}
              </div>
              <ChevronRight size={18} class="chevron" />
            </div>
          {/each}
        </div>
      </section>
    {/each}
  {/if}
  {#if !sub && activity != null}
    <!-- Below every card, in a section of its own: the way into the clean-up list. -->
    <section class="tools-section">
    <h4 class="section-title">Maintenance</h4>
    <button class="cleanup" id="dashboard-open-safe-to-delete" onclick={() => (sub = "safe")}>
      <span class="cleanup-icon"><Sparkles size={20} /></span>
      <span class="flex min-w-0 flex-1 flex-col text-left"><span class="cleanup-title">Clean up merged branches</span>
        <span class="cleanup-detail">Every branch that's already in the base and every environment: safe to delete, newest first</span></span>
      <ChevronRight size={18} class="chevron-go" />
    </button>
    </section>
  {/if}
</div>

</div>
{#if menu}
  {@const { group, branch, x, y } = menu}
  {@const todo = pending(group, branch)}
  {@const close = () => (menu = null)}
  <ContextMenu {x} {y} id="dashboard-menu" class="w-[230px]" onclose={close}>
    {#each todo as kind (kind)}
      {@const a = ACTIONS[kind]}
      <button class="menu-item" id="dashboard-menu-{kind}" onclick={() => { a.run(group, branch); close(); }}><a.icon size={13} />{a.label}<span class="menu-hint">{detail(kind, group, branch).text}</span></button>
    {/each}
    {#if todo.length}<div class="menu-separator"></div>{/if}
    <button class="menu-item" id="dashboard-menu-open" onclick={() => { openGraph(group.repo, branch); close(); }}><Waypoints size={13} />Open in graph</button>
    <button class="menu-item" id="dashboard-menu-focus" onclick={() => { focus(group, branch); close(); }}><Crosshair size={13} />Focus view</button>
    {#if merge(branch)?.kind === "done"}
      <button class="menu-item" id="dashboard-menu-delete" onclick={() => { openGraph(group.repo, branch); close(); }}><CircleCheck size={13} />Safe to delete…</button>
    {/if}
    <div class="menu-separator"></div>
    <button class="menu-item" id="dashboard-menu-copy" title="The repo's path and the branch name, one per line"
      onclick={() => { navigator.clipboard.writeText(`Repo: ${group.repo}\nBranch: ${localName(branch.name)}`); close(); }}>
      <Copy size={13} class="shrink-0" />Copy repo and branch</button>
    <button class="menu-item" id="dashboard-menu-copy-hash" onclick={() => { navigator.clipboard.writeText(branch.tip); close(); }}>
      <Copy size={13} class="shrink-0" />Copy last commit hash<span class="menu-hint font-mono">{branch.tip.slice(0, 7)}</span></button>
  </ContextMenu>
{/if}

<style>
  /* A readable column, centered: never edge to edge on a wide window. */
  .dashboard { display: flex; height: 100%; flex-direction: column; gap: 32px; overflow-y: auto; padding: 22px max(28px, calc((100% - 1080px) / 2)) 36px; }
  /* Refreshing over what's shown: a light sweep along the top, nothing blocks. */
  .refreshing { position: absolute; inset: 0 0 auto 0; z-index: 5; height: 2px; pointer-events: none;
    background: linear-gradient(to right, transparent, var(--primary), transparent); background-size: 40% 100%; background-repeat: no-repeat; animation: sweep 1.3s linear infinite; }
  @keyframes sweep { from { background-position: -40% 0; } to { background-position: 140% 0; } }
  @media (prefers-reduced-motion: reduce) { .refreshing { animation: none; background-size: 100% 100%; opacity: 0.5; } }
  .head { display: flex; flex-direction: column; gap: 10px; }
  .crumbs { display: flex; align-items: center; gap: 6px; }
  .title { font-size: 22px; font-weight: 800; letter-spacing: -0.01em; }
  .crumb { font-size: 22px; font-weight: 800; letter-spacing: -0.01em; color: var(--tertiary); transition: color 0.15s; }
  .crumb:hover { color: var(--foreground); }
  .filters { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
  /* Its own section under the cards: a line above, a small title. */
  .tools-section { display: flex; flex-direction: column; gap: 10px; padding-top: 22px; border-top: 1px solid var(--border); }
  .section-title { font-size: 11px; font-weight: 700; letter-spacing: 0.05em; text-transform: uppercase; color: var(--tertiary); }
  /* The way into Safe to delete: wide and grey, an arrow that moves on hover. */
  .cleanup { display: flex; align-items: center; gap: 14px; width: 100%; padding: 16px 18px; border-radius: 14px; color: var(--foreground);
    background: oklch(0.5 0 0 / 0.07); box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.14); transition: background 0.2s, box-shadow 0.2s; }
  .cleanup:hover { background: oklch(0.5 0 0 / 0.12); box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.24); }
  .cleanup-icon { display: grid; flex-shrink: 0; place-items: center; width: 42px; height: 42px; border-radius: 12px; color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.12); }
  .cleanup-title { font-size: 15px; font-weight: 700; }
  .cleanup-detail { font-size: 12.5px; color: var(--muted-foreground); }
  .cleanup :global(.chevron-go) { flex-shrink: 0; color: var(--tertiary); transition: translate 0.2s, color 0.2s; }
  .cleanup:hover :global(.chevron-go) { translate: 4px 0; color: var(--foreground); }
  .hint { font-size: 12.5px; color: var(--tertiary); }
  .empty { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 60px 0; font-size: 13px; color: var(--muted-foreground); }
  /* The repo leads its group: bigger than the branch names, in its color. */
  .repo h3 { display: flex; align-items: center; gap: 10px; margin-bottom: 12px; font-size: 20px; font-weight: 800; letter-spacing: -0.01em; color: color-mix(in oklab, var(--repo) 55%, var(--foreground)); }
  /* The repo's title opens its graph: on hover it lifts to full color and an arrow slides in. */
  .repo-title { display: flex; align-items: center; gap: 10px; margin-left: -8px; padding: 2px 10px 2px 8px; border-radius: 10px; color: inherit;
    transition: color 0.2s, background 0.2s; }
  .repo-title:hover { color: var(--repo); background: color-mix(in oklab, var(--repo) 10%, transparent); }
  .repo-title :global(.go) { opacity: 0; translate: -6px 0; transition: opacity 0.2s, translate 0.25s var(--ease); }
  .repo-title:hover :global(.go) { opacity: 1; translate: 0 0; }
  .repo-title:hover .dot { scale: 1.15; }
  .dot { transition: scale 0.2s var(--ease); }
  .dot { width: 12px; height: 12px; border-radius: 99px; background: var(--repo); box-shadow: 0 0 0 4px color-mix(in oklab, var(--repo) 22%, transparent); }
  .cards { display: flex; flex-direction: column; gap: 10px; }
  /* One per row, tinted with its repo's color: details on the left, actions on the right. */
  .card { display: flex; align-items: center; gap: 16px; padding: 14px 10px 14px 16px; border-radius: 14px; cursor: pointer; transition: background 0.15s, box-shadow 0.15s;
    background: color-mix(in oklab, var(--repo) 7%, var(--background)); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--repo) 22%, transparent); }
  .card:hover { background: color-mix(in oklab, var(--repo) 11%, var(--background)); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--repo) 40%, transparent); }
  /* Says the card itself goes somewhere: shows on hover. */
  .card :global(.chevron) { flex-shrink: 0; color: var(--tertiary); opacity: 0; transition: opacity 0.15s, translate 0.15s; translate: -4px 0; }
  .card:hover :global(.chevron) { opacity: 1; translate: 0 0; }
  .body { display: flex; min-width: 0; flex: 1; flex-direction: column; gap: 7px; }
  .top { margin-bottom: 2px; }
  .top { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 14px; }
  .name { font-size: 16px; font-weight: 700; line-height: 1.3; word-break: break-word; color: color-mix(in oklab, var(--repo) 30%, var(--foreground)); }
  .where { display: flex; align-items: center; gap: 5px; font-size: 12px; font-weight: 600; color: var(--muted-foreground); }
  /* The last commit: quiet, two lines at most. */
  .last { display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; font-size: 12.5px; line-height: 1.4; color: var(--muted-foreground); }
  .meta { color: var(--tertiary); }
  .line { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 16px; font-size: 12.5px; color: var(--muted-foreground); }
  .line span { display: flex; align-items: center; gap: 6px; }
  .ok { color: #34c77b; }
  /* Side by side on the right, the next thing to do first: tall tiles, the icon on top and the words under it. */
  .actions { display: flex; flex-shrink: 0; gap: 6px; }
  .act { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; width: 74px; height: 92px; padding: 0 6px; border-radius: 12px;
    font-size: 11.5px; font-weight: 600; line-height: 1.2; text-align: center; color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.1); }
  .act:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.18); }
  /* Commit and Push in a soft blue (Commit a touch stronger: it comes first), the count with its unit under the name. */
  .act.commit, .act.push { gap: 6px; color: color-mix(in oklab, var(--primary) 55%, var(--foreground)); }
  .act.commit { background: color-mix(in oklab, var(--primary) 20%, transparent); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--primary) 45%, transparent); }
  .act.push { background: color-mix(in oklab, var(--primary) 10%, transparent); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--primary) 25%, transparent); }
  .act.commit:hover, .act.push:hover { color: var(--foreground); background: color-mix(in oklab, var(--primary) 28%, transparent); }
  .count-line { font-size: 10.5px; font-weight: 600; opacity: 0.75; }
  .act.quiet-ok { color: #34c77b; background: none; box-shadow: inset 0 0 0 1px color-mix(in oklab, #34c77b 35%, transparent); }
  .act.quiet-ok:hover { background: color-mix(in oklab, #34c77b 10%, transparent); }
</style>
