<!-- ⌘K: find a text (e.g. "MOA-854") in commit messages, or a line of code, in this repo or every repo in the sidebar.
     Results grouped by repo; commits list the branches that already contain them (where work ends up first). Enter / click
     opens it in the graph (a line of code: on that file, at that line). -->
<script lang="ts">
  import { FileCode, GitCommitHorizontal, Search } from "@lucide/svelte";
  import { api, type CodeHit, type CodeScope, type CommitHit, type Place } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import { ago, baseName } from "$lib/format";
  import { prefs, type SearchMode } from "$lib/prefs";
  import Modal from "./Modal.svelte";
  import Segmented from "./Segmented.svelte";

  let { model, open = $bindable(false) }: { model: RepoModel; open: boolean } = $props();
  let query = $state("");
  let scope = $state(prefs.get("searchScope", "all"));
  let mode = $state<SearchMode>(prefs.get("searchMode", "commits"));
  let where = $state<CodeScope>(prefs.get("searchCodeWhere", "head"));
  let commits = $state.raw<CommitHit[]>([]);
  let code = $state.raw<CodeHit[]>([]);
  let truncated = $state(false); // some repo had more matches than listed
  let failed = $state<string | null>(null);
  let places = $state.raw<Record<string, Place[]>>({}); // repo + id -> branches containing it
  let searching = $state(false);
  let active = $state(0);
  let input = $state<HTMLInputElement | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let run = 0; // newest search wins: older results arriving late are dropped

  $effect(() => {
    if (open) setTimeout(() => input?.select());
  });

  /** Hits by repo: the current repo first, then `order`. */
  function byRepo<T extends { repo: string }>(hits: T[], order: (a: [string, T[]], b: [string, T[]]) => number) {
    const groups = new Map<string, T[]>();
    for (const hit of hits) {
      const list = groups.get(hit.repo);
      if (list) list.push(hit);
      else groups.set(hit.repo, [hit]);
    }
    return [...groups.entries()].sort((a, b) => (a[0] === model.repo ? -1 : b[0] === model.repo ? 1 : order(a, b)));
  }
  const commitGroups = $derived(byRepo(commits, (a, b) => b[1][0].date - a[1][0].date)); // repos by their newest match
  const codeGroups = $derived(byRepo(code, (a, b) => a[0].localeCompare(b[0])));
  // Keyboard order: commits, then lines of code.
  type Result = { kind: "commit"; hit: CommitHit } | { kind: "code"; hit: CodeHit };
  const ordered = $derived<Result[]>([
    ...commitGroups.flatMap(([, list]) => list.map((hit) => ({ kind: "commit" as const, hit }))),
    ...codeGroups.flatMap(([, list]) => list.map((hit) => ({ kind: "code" as const, hit }))),
  ]);
  const position = $derived(new Map(ordered.map((r, i) => [r.hit, i])));

  async function search() {
    const text = query.trim();
    const mine = ++run;
    clearTimeout(timer);
    failed = null;
    if (text.length < 2) {
      commits = [];
      code = [];
      truncated = false;
      searching = false;
      return;
    }
    searching = true;
    try {
      const repos = scope === "repo" && model.repo ? [model.repo] : (await model.listedRepos()).map((r) => r.path);
      const none = { hits: [], truncated: false };
      const [found, lines] = await Promise.all([
        mode === "code" ? none : api.searchCommits(repos, text),
        mode === "commits" ? none : api.searchCode(repos, prefs.get("mainBranches", {}), text, where),
      ]);
      if (mine !== run) return;
      commits = found.hits;
      code = lines.hits;
      truncated = found.truncated || lines.truncated;
      places = {};
      active = 0;
      // Where each one is: filled in as they arrive, so the list shows right away.
      for (const hit of found.hits) {
        api.commitPlaces(hit.repo, hit.id).then((names) => {
          if (mine === run) places = { ...places, [hit.repo + hit.id]: names };
        }, () => {});
      }
    } catch (error) {
      if (mine === run) failed = String(error).split("\n")[0];
    } finally {
      if (mine === run) searching = false;
    }
  }

  function typed() {
    clearTimeout(timer);
    timer = setTimeout(search, 250);
  }

  function choose(result: Result) {
    open = false;
    if (result.kind === "code") model.revealLine(result.hit.repo, result.hit.id, result.hit.path, result.hit.line);
    else model.revealCommit(result.hit.repo, result.hit.id);
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") active = Math.min(active + 1, ordered.length - 1);
    else if (e.key === "ArrowUp") active = Math.max(active - 1, 0);
    else if (e.key === "Enter" && ordered[active]) choose(ordered[active]);
    else return;
    e.preventDefault();
    document.getElementById(`search-hit-${active}`)?.scrollIntoView({ block: "nearest" });
  }

  /** Splits `text` around the matches of the query, for highlighting. */
  function marks(text: string) {
    const q = query.trim().toLowerCase();
    const out: { text: string; hit: boolean }[] = [];
    let from = 0;
    for (let at = text.toLowerCase().indexOf(q); q && at >= 0; at = text.toLowerCase().indexOf(q, from)) {
      out.push({ text: text.slice(from, at), hit: false }, { text: text.slice(at, at + q.length), hit: true });
      from = at + q.length;
    }
    out.push({ text: text.slice(from), hit: false });
    return out;
  }
</script>

{#snippet highlighted(text: string)}
  {#each marks(text) as part, i (i)}{#if part.hit}<mark>{part.text}</mark>{:else}{part.text}{/if}{/each}
{/snippet}

{#snippet groupTitle(repo: string, count: string)}
  <div class="flex items-center gap-2 px-2 pt-2 pb-1 text-[11px] font-semibold text-muted-foreground uppercase">
    {baseName(repo)}<span class="font-medium text-tertiary normal-case">{count}</span>
  </div>
{/snippet}

{#if open}
  <Modal id="commit-search" top label="Search" onclose={() => (open = false)} class="search-panel">
    <div class="flex items-center gap-2.5 border-b px-4 py-3">
      <Search size={16} class="shrink-0 text-tertiary" />
      <input bind:this={input} bind:value={query} oninput={typed} onkeydown={keydown} id="commit-search-input"
        placeholder={mode === "code" ? "Search lines of code… Ej: v8 migration" : mode === "both" ? "Search commits and code…" : "Search commit titles and descriptions… Ej: MOA-854"}
        class="min-w-0 flex-1 bg-transparent text-[15px] outline-none" />
      <Segmented id="commit-search-scope" value={scope} onchange={(v) => { scope = v; prefs.set("searchScope", v); search(); }}
        options={[{ value: "repo", label: "This repo", title: `Only ${baseName(model.repo ?? "")}`, disabled: !model.repo }, { value: "all", label: "All repos" }]} />
    </div>
    <div class="flex items-center gap-2 border-b px-4 py-2">
      <Segmented id="commit-search-mode" value={mode} onchange={(v) => { mode = v; prefs.set("searchMode", v); search(); }}
        options={[{ value: "commits", label: "Commits" }, { value: "code", label: "Code" }, { value: "both", label: "Both" }]} />
      {#if mode !== "commits"}
        <span class="text-[11px] text-tertiary">Code in</span>
        <Segmented id="commit-search-where" value={where} onchange={(v) => { where = v; prefs.set("searchCodeWhere", v); search(); }}
          options={[{ value: "head", label: "Current branch", title: "The checked-out branch of each repo, as last committed" },
            { value: "base", label: "Base", title: "Each repo's base (main, production…)" },
            { value: "all", label: "All branches", title: "Every local and remote branch; slower on big repos" }]} />
      {/if}
    </div>
    <div class="max-h-[60vh] overflow-auto p-2" id="commit-search-results">
      {#if query.trim().length < 2}
        <p class="hint">Type at least 2 characters.{mode === "code" ? "" : " Commits: every branch, local and remote."}</p>
      {:else if failed}
        <p class="hint">Search failed: {failed}</p>
      {:else if searching && !ordered.length}
        <p class="hint">Searching…</p>
      {:else if !ordered.length}
        <p class="hint">Nothing mentions “{query.trim()}”</p>
      {:else}
        {#each commitGroups as [repo, list] (repo)}
          {@render groupTitle(repo, `${list.length} commits`)}
          {#each list as hit (hit.id)}
            {@const index = position.get(hit) ?? -1}
            {@const names = places[hit.repo + hit.id]}
            {@const targets = names?.filter((p) => p.target) ?? []}
            {@const others = names?.filter((p) => !p.target) ?? []}
            <button class="hit" class:active={index === active} id="search-hit-{index}" onclick={() => choose({ kind: "commit", hit })} onmousemove={() => (active = index)}>
              <GitCommitHorizontal size={14} class="mt-[3px] shrink-0 text-tertiary" />
              <span class="flex min-w-0 flex-1 flex-col gap-1">
                <span class="flex items-baseline gap-2">
                  <span class="min-w-0 flex-1 truncate text-[13px] font-medium">{@render highlighted(hit.subject)}</span>
                  <span class="shrink-0 text-[11px] text-tertiary">{ago(hit.date)}</span>
                </span>
                {#if hit.snippet}<span class="truncate text-[12px] text-muted-foreground">{@render highlighted(hit.snippet)}</span>{/if}
                <span class="flex flex-wrap items-center gap-1.5 text-[11px] text-tertiary">
                  <span class="font-mono">{hit.id.slice(0, 7)}</span>·<span>{hit.author}</span>
                  {#if !names}<span class="opacity-60">· finding branches…</span>{/if}
                  {#each targets as place (place.name)}<span class="env">✓ {place.name}</span>{/each}
                  {#each others.slice(0, 2) as place (place.name)}<span class="branch">{place.name}</span>{/each}
                  {#if others.length > 2}<span title={others.slice(2).map((p) => p.name).join("\n")}>+{others.length - 2} more</span>{/if}
                </span>
              </span>
            </button>
          {/each}
        {/each}
        {#each codeGroups as [repo, list] (repo)}
          {@render groupTitle(repo, `${list.length} lines`)}
          {#each list as hit (hit.path + hit.line + hit.id)}
            {@const index = position.get(hit) ?? -1}
            <button class="hit" class:active={index === active} id="search-hit-{index}" onclick={() => choose({ kind: "code", hit })} onmousemove={() => (active = index)}>
              <FileCode size={14} class="mt-[3px] shrink-0 text-tertiary" />
              <span class="flex min-w-0 flex-1 flex-col gap-1">
                <span class="truncate font-mono text-[12px]">{@render highlighted(hit.text)}</span>
                <span class="flex min-w-0 items-center gap-1.5 text-[11px] text-tertiary">
                  <span class="truncate" style="direction: rtl; text-align: left">{hit.path}:{hit.line}</span>
                  {#each hit.branches.slice(0, 2) as name (name)}<span class={hit.targets.includes(name) ? "env" : "branch"}>{hit.targets.includes(name) ? `✓ ${name}` : name}</span>{/each}
                  {#if hit.branches.length > 2}<span class="shrink-0" title={hit.branches.slice(2).join("\n")}>+{hit.branches.length - 2} more</span>{/if}
                </span>
              </span>
            </button>
          {/each}
        {/each}
        {#if truncated}
          <p class="hint" id="commit-search-truncated">Only the first matches of each repo are listed: type more to narrow it down.</p>
        {/if}
      {/if}
    </div>
  </Modal>
{/if}

<style>
  :global(.search-panel) { width: min(720px, 92vw); border-radius: 14px; overflow: hidden; }
  .hint { padding: 18px 10px; text-align: center; font-size: 12px; color: var(--tertiary); }
  .hit { display: flex; width: 100%; gap: 10px; padding: 8px 10px; border-radius: 9px; text-align: left; }
  .hit.active { background: color-mix(in oklab, var(--primary) 14%, transparent); }
  .env { padding: 0 6px; border-radius: 99px; font-weight: 600; color: rgb(52 199 89); background: rgb(52 199 89 / 0.12); }
  .branch { max-width: 180px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding: 0 6px; border-radius: 99px; background: oklch(0.5 0 0 / 0.12); }
  mark { color: inherit; border-radius: 3px; background: color-mix(in oklab, var(--primary) 35%, transparent); }
</style>
