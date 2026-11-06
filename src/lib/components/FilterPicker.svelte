<script lang="ts">
  // Toolbar (Commits collapsed): keep only the branches in some states. Computed on demand, not on every load.
  // With room in the toolbar, each filter is its own toggle; when it's narrow, they fold into the Filter menu.
  import { Check, CircleCheck, Clock, ListFilter, LoaderCircle, UserRound } from "@lucide/svelte";
  import type { Component } from "svelte";
  import * as Popover from "$lib/components/ui/popover";
  import type { BranchFilter, RepoModel } from "$lib/model.svelte";

  // `collapsedOnly`: the branch-state filters are off outside the Collapsed view; My branches works everywhere.
  let { model, collapsedOnly = false }: { model: RepoModel; collapsedOnly?: boolean } = $props();
  const MINE = { label: "My branches", hint: "Only branches whose last commit is yours, plus the base and the environments. Ej: hides your teammates' branches." };
  // Same icons as the branch cards' status.
  const OPTIONS: { key: BranchFilter; label: string; hint: string; icon: Component }[] = [
    { key: "pending", label: "Pending merge", hint: "In some places already, still missing in the base or an environment", icon: Clock },
    { key: "safe", label: "Safe to delete", hint: "Already everywhere: merged, squashed or cherry-picked", icon: CircleCheck },
  ];
  const active = $derived(model.filters.length + (model.onlyMine ? 1 : 0)); // how many are on, for the folded menu

  function toggle(key: BranchFilter) {
    model.setFilters(model.filters.includes(key) ? model.filters.filter((f) => f !== key) : [...model.filters, key]);
  }
</script>

<div class="inline-filters" role="group" aria-label="Show only">
  <button class="tool" class:on={model.onlyMine} aria-pressed={model.onlyMine} id="only-mine-toggle" onclick={() => model.setOnlyMine(!model.onlyMine)}
    title={MINE.hint}><UserRound size={14} />{MINE.label}</button>
  {#each OPTIONS as option (option.key)}
    {@const active = model.filters.includes(option.key)}
    <button class="tool" class:on={active} aria-pressed={active} id="branch-filter-toggle-{option.key}" disabled={collapsedOnly} onclick={() => toggle(option.key)}
      title={collapsedOnly ? "Works in the Collapsed view" : `Show only branches: ${option.label}\n${option.hint}`}>
      <option.icon size={14} />{option.label}
    </button>
  {/each}
  {#if model.isFiltering}<LoaderCircle size={14} class="shrink-0 animate-spin text-tertiary" />{/if}
</div>

<div class="menu-filters">
<Popover.Root>
  <Popover.Trigger class="tool {active ? 'on' : ''}" id="branch-filter-open" title={`Filter branches${active ? `: ${active} on` : ""}`}>
    {#if model.isFiltering}<LoaderCircle size={14} class="animate-spin" />{:else}<ListFilter size={14} />{/if}
    Filter{#if active}<span class="count">{active}</span>{/if}
  </Popover.Trigger>
  <Popover.Content align="start" class="w-[290px] p-1.5" id="branch-filter-panel">
    <div class="flex items-center justify-between px-2 pt-1 pb-1.5">
      <span class="text-[11px] font-semibold text-muted-foreground uppercase">Show only</span>
      {#if active}<button class="clear" id="branch-filter-clear" onclick={() => { model.setFilters([]); if (model.onlyMine) model.setOnlyMine(false); }}>Clear</button>{/if}
    </div>
    {@render pick("only-mine", MINE.label, MINE.hint, model.onlyMine, false, () => model.setOnlyMine(!model.onlyMine))}
    {#each OPTIONS as option (option.key)}
      {@render pick(option.key, option.label, option.hint, model.filters.includes(option.key), collapsedOnly, () => toggle(option.key))}
    {/each}
    {#if collapsedOnly}<span class="block px-2 pt-1.5 text-[11px] text-tertiary">Pending merge and Safe to delete work in the Collapsed view.</span>{/if}
    {#if model.isFiltering}<span class="block px-2 pt-1.5 text-[11px] text-tertiary">Checking squash and cherry-pick merges…</span>{/if}
  </Popover.Content>
</Popover.Root>
</div>

{#snippet pick(key: string, label: string, hint: string, checked: boolean, off: boolean, onclick: () => void)}
  <button class="pick" id="branch-filter-{key}" disabled={off} {onclick}>
    <span class="box" class:on={checked}>{#if checked}<Check size={10} strokeWidth={3} />{/if}</span>
    <span class="flex min-w-0 flex-col"><span>{label}</span><span class="hint">{hint}</span></span>
  </button>
{/snippet}

<style>
  /* GraphToolbar's strip is the container: the toggles when it has room, the menu otherwise. */
  .inline-filters { display: none; align-items: center; gap: 1px; }
  .inline-filters :global(.tool) { height: 26px; padding: 0 8px; }
  @container toolbar (min-width: 760px) {
    .inline-filters { display: flex; }
    .menu-filters { display: none; }
  }
  .count { display: grid; place-items: center; min-width: 16px; height: 16px; padding: 0 4px; border-radius: 999px; font-size: 10.5px; color: white; background: var(--primary); }
  .clear { font-size: 11.5px; color: var(--primary); }
  .clear:hover { text-decoration: underline; }
  .pick { display: flex; width: 100%; align-items: flex-start; gap: 9px; padding: 6px 8px; border-radius: 6px; font-size: 12.5px; text-align: left; }
  .pick:hover:not(:disabled) { background: oklch(0.5 0 0 / 0.1); }
  .pick:disabled { opacity: 0.4; }
  .hint { font-size: 11px; color: var(--muted-foreground); }
  .box { display: grid; flex-shrink: 0; place-items: center; width: 14px; height: 14px; margin-top: 2px; border-radius: 4px; box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.4); color: white; }
  .box.on { background: var(--primary); box-shadow: none; }
</style>
