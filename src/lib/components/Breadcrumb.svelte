<!-- The header's title: where you are, as a path. Dashboard › repo › focused branch › file's history; each step but the
     last goes back to it. -->
<script lang="ts">
  import { ChevronRight, Crosshair, History } from "@lucide/svelte";
  import type { RepoModel } from "$lib/model.svelte";
  import { baseName } from "$lib/format";
  import { localName } from "$lib/refs";

  let { model }: { model: RepoModel } = $props();

  async function toRepo() {
    if (model.historyPath) await model.setHistory(null);
    if (model.focus) await model.setFocus(null);
    model.showGraph = true;
  }

  // `tag`: what kind of view the step is, when its name alone doesn't say (a branch name could be anything).
  type Crumb = { id: string; label: string; go: () => void; tag?: { text: string; icon: typeof Crosshair } };
  const crumbs: Crumb[] = $derived([
    { id: "dashboard", label: "Dashboard", go: () => { model.showGraph = false; model.dashboardView = null; } },
    ...(!model.showGraph && model.dashboardView === "safe" ? [{ id: "safe-to-delete", label: "Safe to delete", go: () => {} }] : []),
    ...(model.showGraph && model.repo ? [{ id: "repo", label: baseName(model.repo), go: toRepo }] : []),
    ...(model.showGraph && model.focus ? [{ id: "focus", label: localName(model.focus[0]), go: () => model.setHistory(null), tag: { text: "Focus", icon: Crosshair } }] : []),
    ...(model.showGraph && model.historyPath ? [{ id: "file", label: baseName(model.historyPath), go: () => {}, tag: { text: "History", icon: History } }] : []),
  ]);
</script>

{#snippet name(crumb: Crumb)}
  {#if crumb.tag}<span class="tag"><crumb.tag.icon size={12} strokeWidth={2.5} />{crumb.tag.text}</span>{/if}<span class="text">{crumb.label}</span>
{/snippet}

<nav class="crumbs" aria-label="Where you are">
  {#each crumbs as crumb, i (crumb.id)}
    {#if i > 0}<ChevronRight size={20} strokeWidth={2.5} class="sep shrink-0" />{/if}
    {#if i < crumbs.length - 1}
      <button class="crumb" id="breadcrumb-{crumb.id}" onclick={crumb.go}>{@render name(crumb)}</button>
    {:else}
      <span class="crumb current" id="breadcrumb-{crumb.id}" aria-current="page">{@render name(crumb)}</span>
    {/if}
  {/each}
</nav>

<style>
  .crumbs { display: flex; min-width: 0; flex: 1; align-items: center; gap: 4px; }
  .crumb { flex-shrink: 10; min-width: 0; overflow: hidden; padding: 2px 6px; border-radius: 7px; font-size: 24px; font-weight: 800; letter-spacing: -0.01em; white-space: nowrap; text-overflow: ellipsis; color: var(--tertiary); transition: color 0.15s, background 0.15s; animation: crumb-in 0.28s var(--ease, ease) both; }
  button.crumb:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.08); }
  .current { flex-shrink: 1; color: var(--foreground); } /* the earlier steps give way first */
  .crumb { display: flex; align-items: center; gap: 8px; }
  .text { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .tag {
    display: flex; flex-shrink: 0; align-items: center; gap: 4px; height: 20px; padding: 0 7px; border-radius: 99px;
    font-size: 11px; font-weight: 600; letter-spacing: 0; color: #a78bfa; background: color-mix(in oklab, #a78bfa 15%, transparent);
  }
  .crumbs :global(.sep) { color: var(--tertiary); opacity: 0.6; }
  @keyframes crumb-in { from { opacity: 0; transform: translateX(-6px); } }
  @media (prefers-reduced-motion: reduce) { .crumb { animation: none; } }
</style>
