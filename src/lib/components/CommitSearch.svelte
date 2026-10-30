<!-- ⌘K: find a text (e.g. "MOA-854") in commit titles and descriptions, in this repo or every repo in the sidebar.
     Results grouped by repo, each with the branches that already contain it (environments first). Enter / click opens it in the graph. -->
<script lang="ts">
  import { GitCommitHorizontal, Search } from "@lucide/svelte";
  import { api, type SearchHit } from "$lib/api";
  import * as Tabs from "$lib/components/ui/tabs";
  import { prefs, type RepoModel } from "$lib/model.svelte";

  let { model, open = $bindable(false) }: { model: RepoModel; open: boolean } = $props();
  let query = $state("");
  let scope = $state<"repo" | "all">(prefs.get("searchScope", "all"));
  let hits = $state.raw<SearchHit[]>([]);
  let places = $state.raw<Record<string, string[]>>({}); // repo + id -> branches containing it
  let searching = $state(false);
  let active = $state(0);
  let input = $state<HTMLInputElement | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let run = 0; // newest search wins: older results arriving late are dropped

  const ENVIRONMENT = /^(main|master|production|prod|qa|develop|dev|staging|stage|release)$/;
  const repoName = (path: string) => path.split(/[\\/]/).pop() ?? path;

  $effect(() => {
    if (open) setTimeout(() => input?.select());
  });

  // Current repo first, then repos by their newest match.
  const groups = $derived.by(() => {
    const byRepo = new Map<string, SearchHit[]>();
    for (const hit of hits) byRepo.set(hit.repo, [...(byRepo.get(hit.repo) ?? []), hit]);
    return [...byRepo.entries()].sort(([a, x], [b, y]) => (a === model.repo ? -1 : b === model.repo ? 1 : y[0].date - x[0].date));
  });
  const ordered = $derived(groups.flatMap(([, list]) => list)); // keyboard order

  async function search() {
    const text = query.trim();
    const mine = ++run;
    if (text.length < 2) {
      hits = [];
      searching = false;
      return;
    }
    searching = true;
    const { folders, repos: added, hidden } = model.sources;
    const repos = scope === "repo" && model.repo ? [model.repo]
      : (await api.listRepos(folders, added)).map((r) => r.path).filter((p) => !hidden.includes(p));
    const found = await api.searchCommits(repos, text);
    if (mine !== run) return;
    hits = found;
    places = {};
    active = 0;
    searching = false;
    // Where each one is: filled in as they arrive, so the list shows right away.
    for (const hit of found) {
      api.commitPlaces(hit.repo, hit.id).then((names) => {
        if (mine === run) places = { ...places, [hit.repo + hit.id]: names };
      });
    }
  }

  function typed() {
    clearTimeout(timer);
    timer = setTimeout(search, 250);
  }

  function setScope(value: string) {
    scope = value as "repo" | "all";
    prefs.set("searchScope", scope);
    search();
  }

  function choose(hit: SearchHit) {
    open = false;
    model.revealCommit(hit.repo, hit.id);
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === "Escape") open = false;
    else if (e.key === "ArrowDown") active = Math.min(active + 1, ordered.length - 1);
    else if (e.key === "ArrowUp") active = Math.max(active - 1, 0);
    else if (e.key === "Enter" && ordered[active]) choose(ordered[active]);
    else return;
    e.preventDefault();
    document.getElementById(`search-hit-${active}`)?.scrollIntoView({ block: "nearest" });
  }

  function ago(seconds: number) {
    const days = Math.floor((Date.now() / 1000 - seconds) / 86400);
    if (days < 1) return "today";
    if (days < 30) return `${days}d ago`;
    if (days < 365) return `${Math.floor(days / 30)}mo ago`;
    return `${Math.floor(days / 365)}y ago`;
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

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="backdrop enter-fade" onclick={() => (open = false)} id="commit-search-backdrop">
    <div class="panel enter-pop" role="dialog" aria-label="Search commits" tabindex="-1" id="commit-search" onclick={(e) => e.stopPropagation()}>
      <div class="flex items-center gap-2.5 border-b px-4 py-3">
        <Search size={16} class="shrink-0 text-tertiary" />
        <input bind:this={input} bind:value={query} oninput={typed} onkeydown={keydown} id="commit-search-input"
          placeholder="Search commit titles and descriptions… Ej: MOA-854" class="min-w-0 flex-1 bg-transparent text-[15px] outline-none" />
        <Tabs.Root value={scope} onValueChange={setScope} id="commit-search-scope">
          <Tabs.List>
            <Tabs.Trigger value="repo" id="commit-search-scope-repo" title="Only {repoName(model.repo ?? '')}" disabled={!model.repo}>This repo</Tabs.Trigger>
            <Tabs.Trigger value="all" id="commit-search-scope-all">All repos</Tabs.Trigger>
          </Tabs.List>
        </Tabs.Root>
      </div>
      <div class="max-h-[60vh] overflow-auto p-2" id="commit-search-results">
        {#if query.trim().length < 2}
          <p class="hint">Type at least 2 characters. Searches every branch, local and remote.</p>
        {:else if searching && !hits.length}
          <p class="hint">Searching…</p>
        {:else if !hits.length}
          <p class="hint">No commit mentions “{query.trim()}”</p>
        {:else}
          {#each groups as [repo, list] (repo)}
            <div class="flex items-center gap-2 px-2 pt-2 pb-1 text-[11px] font-semibold text-muted-foreground uppercase">
              {repoName(repo)}<span class="font-medium text-tertiary normal-case">{list.length}{list.length >= 30 ? "+" : ""} commits</span>
            </div>
            {#each list as hit (hit.id)}
              {@const index = ordered.indexOf(hit)}
              {@const names = places[hit.repo + hit.id]}
              {@const environments = names?.filter((n) => ENVIRONMENT.test(n.replace(/^(origin|upstream)\//, ""))) ?? []}
              {@const others = names?.filter((n) => !environments.includes(n)) ?? []}
              <button class="hit" class:active={index === active} id="search-hit-{index}" onclick={() => choose(hit)} onmousemove={() => (active = index)}>
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
                    {#each environments as name (name)}<span class="env">✓ {name}</span>{/each}
                    {#each others.slice(0, 2) as name (name)}<span class="branch">{name}</span>{/each}
                    {#if others.length > 2}<span title={others.slice(2).join("\n")}>+{others.length - 2} more</span>{/if}
                  </span>
                </span>
              </button>
            {/each}
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop { position: fixed; inset: 0; z-index: 900; display: flex; justify-content: center; align-items: flex-start; padding-top: 12vh; background: rgb(0 0 0 / 0.4); }
  .panel {
    width: min(720px, 92vw); border-radius: 14px; overflow: hidden; background: var(--popover); color: var(--popover-foreground);
    box-shadow: 0 0 0 1px oklch(0.5 0 0 / 0.2), 0 24px 64px rgb(0 0 0 / 0.45);
  }
  .hint { padding: 18px 10px; text-align: center; font-size: 12px; color: var(--tertiary); }
  .hit { display: flex; width: 100%; gap: 10px; padding: 8px 10px; border-radius: 9px; text-align: left; }
  .hit.active { background: color-mix(in oklab, var(--primary) 14%, transparent); }
  .env { padding: 0 6px; border-radius: 99px; font-weight: 600; color: rgb(52 199 89); background: rgb(52 199 89 / 0.12); }
  .branch { max-width: 180px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding: 0 6px; border-radius: 99px; background: oklch(0.5 0 0 / 0.12); }
  mark { color: inherit; border-radius: 3px; background: color-mix(in oklab, var(--primary) 35%, transparent); }
</style>
