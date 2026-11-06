<!-- Dashboard › Safe to delete: in every repo, the branches whose work is already in the base and every environment,
     newest first, whatever their age. Lazy twice: Dashboard imports it only when opened, and it checks branches a page
     at a time, the next page when the end of the list scrolls into view. A row opens the branch in the graph, where
     its ✓ checks with the server and offers the options. -->
<script lang="ts" module>
  import type { ActiveBranch } from "$lib/api";
  type Row = { repo: string; branch: ActiveBranch };
  // Pages loaded per "My branches", kept across visits: shown at once.
  const cache = new Map<boolean, { rows: Row[]; next: number | null }>();
</script>

<script lang="ts">
  import { ChevronRight, CircleCheck, LoaderCircle, Sparkles } from "@lucide/svelte";
  import { api } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import { mergeState } from "$lib/cards";
  import { ago, baseName } from "$lib/format";
  import { localName } from "$lib/refs";
  import Loader from "./Loader.svelte";
  import { untrack } from "svelte";

  let { model, onlyMine, colorOf }: { model: RepoModel; onlyMine: boolean; colorOf: (repo: string) => string } = $props();
  const PAGE = 20;
  let rows = $state.raw<Row[] | null>(null);
  let next = $state<number | null>(0);
  let loading = $state(false);
  let sentinel = $state<HTMLElement | null>(null);
  let generation = 0; // a newer filter wins over a page still in flight

  async function more() {
    if (loading || next == null) return;
    loading = true;
    const mine = onlyMine, mark = generation;
    try {
      const page = await api.safeToDelete(model.repos.map((r) => model.picks(r.path)), mine, next, PAGE);
      if (mark !== generation) return;
      rows = [...(rows ?? []), ...page.items.map(({ repo, ...branch }) => ({ repo, branch }))];
      next = page.next;
      cache.set(mine, { rows, next });
    } catch {
      if (mark === generation) { rows ??= []; next = null; }
    } finally {
      if (mark === generation) loading = false;
    }
  }

  // A filter change starts over (from what's cached for it, if anything).
  $effect(() => {
    const kept = cache.get(onlyMine);
    untrack(() => {
      generation++;
      loading = false;
      rows = kept?.rows ?? null;
      next = kept ? kept.next : 0;
      if (!kept) more();
    });
  });

  // The end of the list in view: the next page.
  $effect(() => {
    if (!sentinel) return;
    const seen = new IntersectionObserver((entries) => entries.some((e) => e.isIntersecting) && more(), { rootMargin: "300px" });
    seen.observe(sentinel);
    return () => seen.disconnect();
  });
</script>

{#if rows == null}
  <Loader title="Checking every branch" detail="Which ones are already in the base and every environment" />
{:else if !rows.length && next == null}
  <div class="empty" id="safe-to-delete-empty"><Sparkles size={28} strokeWidth={1.5} />Nothing to clean up: no {onlyMine ? "branch of yours" : "branch"} is merged everywhere.</div>
{:else}
  <div class="summary">
    <CircleCheck size={16} class="text-[#34c77b]" />{rows.length === 1 ? "1 branch" : `${rows.length} branches`} safe to delete{next != null ? " so far" : ""}

  </div>
  <div class="list">
    {#each rows as { repo, branch } (repo + branch.name)}
      {@const m = branch.status && mergeState(branch.status)}
      <button class="row" style:--repo={colorOf(repo)} id="safe-to-delete-{baseName(repo)}-{branch.name}" title="Open it in the graph: its ✓ checks with the server and offers the options"
        onclick={() => model.revealCommit(repo, branch.tip)}>
        <span class="repo">{baseName(repo)}</span>
        <span class="flex min-w-0 flex-1 flex-col">
          <span class="name">{localName(branch.name)}</span>
          <span class="meta">{#if m}Merged into {m.done.join(", ")} · {/if}{branch.author} · {ago(branch.date)}</span>
        </span>
        <ChevronRight size={16} class="go" />
      </button>
    {/each}
  </div>
{/if}
{#if rows != null && next != null}
  <div class="more" bind:this={sentinel}>{#if loading}<LoaderCircle size={14} class="animate-spin" />Checking more branches…{/if}</div>
{/if}

<style>
  .more { display: flex; min-height: 40px; align-items: center; justify-content: center; gap: 8px; font-size: 12.5px; color: var(--tertiary); }
  .empty { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 60px 0; font-size: 13px; color: var(--muted-foreground); }
  .summary { display: flex; align-items: center; gap: 8px; font-size: 13px; font-weight: 600; color: var(--muted-foreground); }
  .list { display: flex; flex-direction: column; gap: 6px; }
  .row { display: flex; align-items: center; gap: 12px; padding: 12px 14px; border-radius: 12px; text-align: left;
    background: oklch(0.5 0 0 / 0.05); box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.1); transition: background 0.15s, box-shadow 0.15s; }
  .row:hover { background: color-mix(in oklab, #34c77b 8%, transparent); box-shadow: inset 0 0 0 1px color-mix(in oklab, #34c77b 35%, transparent); }
  .repo { flex-shrink: 0; max-width: 170px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding: 2px 8px; border-radius: 6px;
    font-size: 11.5px; font-weight: 700; color: color-mix(in oklab, var(--repo) 60%, var(--foreground)); background: color-mix(in oklab, var(--repo) 14%, transparent); }
  .name { font-size: 14px; font-weight: 700; word-break: break-word; }
  .meta { font-size: 12px; color: var(--tertiary); }
  .row :global(.go) { flex-shrink: 0; color: var(--tertiary); opacity: 0; translate: -4px 0; transition: opacity 0.15s, translate 0.15s; }
  .row:hover :global(.go) { opacity: 1; translate: 0 0; }
</style>
