<script lang="ts">
  // Toolbar chip (Commits collapsed): keep only the branches in some states. Computed on demand, not on every load.
  import { Check, ListFilter, LoaderCircle } from "@lucide/svelte";
  import * as Popover from "$lib/components/ui/popover";
  import type { BranchFilter, RepoModel } from "$lib/model.svelte";

  let { model, disabled = false }: { model: RepoModel; disabled?: boolean } = $props();
  const OPTIONS: { key: BranchFilter; label: string; hint: string }[] = [
    { key: "pending", label: "Pending merge", hint: "In some places already, still missing in the base or an environment" },
    { key: "safe", label: "Safe to delete", hint: "Already everywhere: merged, squashed or cherry-picked" },
    { key: "wip", label: "With WIP", hint: "Uncommitted changes, or work the app put aside (stash) when you switched branches" },
  ];
  const on = $derived(model.filters.length > 0);

  function toggle(key: BranchFilter) {
    model.setFilters(model.filters.includes(key) ? model.filters.filter((f) => f !== key) : [...model.filters, key]);
  }
</script>

<Popover.Root>
  <Popover.Trigger class="filter-trigger {on ? 'on' : ''}" id="branch-filter-open" {disabled}
    title={disabled ? "Filters work with Commits collapsed" : `Filter branches${on ? `: ${model.filters.length} on` : ""}`}>
    {#if model.isFiltering}<LoaderCircle size={14} class="animate-spin" />{:else}<ListFilter size={14} />{/if}
    Filter{#if on}<span class="count">{model.filters.length}</span>{/if}
  </Popover.Trigger>
  <Popover.Content align="start" class="w-[290px] p-1.5" id="branch-filter-panel">
    <div class="flex items-center justify-between px-2 pt-1 pb-1.5">
      <span class="text-[11px] font-semibold text-muted-foreground uppercase">Show only branches</span>
      {#if on}<button class="clear" id="branch-filter-clear" onclick={() => model.setFilters([])}>Clear</button>{/if}
    </div>
    {#each OPTIONS as option (option.key)}
      <button class="pick" id="branch-filter-{option.key}" onclick={() => toggle(option.key)}>
        <span class="box" class:on={model.filters.includes(option.key)}>{#if model.filters.includes(option.key)}<Check size={10} strokeWidth={3} />{/if}</span>
        <span class="flex min-w-0 flex-col"><span>{option.label}</span><span class="hint">{option.hint}</span></span>
      </button>
    {/each}
    {#if model.isFiltering}<span class="block px-2 pt-1.5 text-[11px] text-tertiary">Checking squash and cherry-pick merges…</span>{/if}
  </Popover.Content>
</Popover.Root>

<style>
  :global(.filter-trigger) { display: flex; flex-shrink: 0; align-items: center; gap: 6px; height: 30px; padding: 0 10px; border-radius: 8px; font-size: 12.5px; font-weight: 600; color: var(--muted-foreground); }
  :global(.filter-trigger:hover:not(:disabled)), :global(.filter-trigger[data-state="open"]) { color: var(--foreground); background: oklch(0.5 0 0 / 0.1); }
  :global(.filter-trigger.on), :global(.filter-trigger.on:hover:not(:disabled)) { color: var(--primary); }
  :global(.filter-trigger:disabled) { opacity: 0.4; }
  .count { display: grid; place-items: center; min-width: 16px; height: 16px; padding: 0 4px; border-radius: 999px; font-size: 10.5px; color: white; background: var(--primary); }
  .clear { font-size: 11.5px; color: var(--primary); }
  .clear:hover { text-decoration: underline; }
  .pick { display: flex; width: 100%; align-items: flex-start; gap: 9px; padding: 6px 8px; border-radius: 6px; font-size: 12.5px; text-align: left; }
  .pick:hover { background: oklch(0.5 0 0 / 0.1); }
  .hint { font-size: 11px; color: var(--muted-foreground); }
  .box { display: grid; flex-shrink: 0; place-items: center; width: 14px; height: 14px; margin-top: 2px; border-radius: 4px; box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.4); color: white; }
  .box.on { background: var(--primary); box-shadow: none; }
</style>
