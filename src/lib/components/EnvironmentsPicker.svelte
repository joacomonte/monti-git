<script lang="ts">
  // Toolbar button ("Configure"; the names are in its tooltip): what every branch is compared against in this repo.
  // One list: check the branches that are environments; the house on one of them makes it the base (pinned left).
  import { Check, ChevronDown, House, Search, Settings2 } from "@lucide/svelte";
  import * as Popover from "$lib/components/ui/popover";
  import type { RepoModel } from "$lib/model.svelte";
  import { localName } from "$lib/refs";

  let { model }: { model: RepoModel } = $props();
  let query = $state("");

  const base = $derived(model.history.base ? localName(model.history.base) : null);
  const chosen = $derived(model.history.environments.map(localName));
  const picked = (name: string) => name === base || chosen.includes(name);
  // Picked first, then every other branch (by local name), filtered by the search.
  const rows = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const all = [...new Set([...(base ? [base] : []), ...chosen, ...model.history.local, ...model.history.allBranches.map(localName)])];
    return all.filter((n) => !q || n.toLowerCase().includes(q)).sort((a, b) => Number(picked(b)) - Number(picked(a)));
  });

  function toggle(name: string) {
    if (name === base) return; // the base is always one; move the house to change it
    if (!base) return model.configure(name, chosen);
    model.configure(base, chosen.includes(name) ? chosen.filter((n) => n !== name) : [...chosen, name]);
  }
</script>

<Popover.Root onOpenChange={(open) => { if (!open) query = ""; }}>
  <Popover.Trigger class="tool" id="compare-open"
    title={`Base: ${base ?? "none"} · Environments: ${chosen.join(", ") || "none"}\nWhat every branch is compared against: how far behind the base, and where it's still missing. Click to change.`}>
    <Settings2 size={14} class="shrink-0" />Configure<ChevronDown size={12} class="shrink-0" />
  </Popover.Trigger>
  <Popover.Content align="start" class="w-[300px] p-2" id="compare-panel">
    <p class="why">Check the environments. The <House size={11} class="inline -mt-0.5" /> marks the base.</p>
    <label class="search"><Search size={12} /><input id="compare-search" placeholder="Find a branch" bind:value={query} /></label>
    <div class="max-h-[300px] overflow-y-auto">
      {#each rows as name (name)}
        {@const isBase = name === base}
        <div class="row" class:on={picked(name)}>
          <button class="pick" id="environment-{name}" role="checkbox" aria-checked={picked(name)} disabled={isBase} onclick={() => toggle(name)}
            title={isBase ? "The base: pick another one with its house to change it" : picked(name) ? `Stop comparing against ${name}` : `Compare every branch against ${name}`}>
            <span class="box" class:checked={picked(name)}>{#if picked(name)}<Check size={10} strokeWidth={3} />{/if}</span>
            <span class="min-w-0 flex-1 truncate">{name}</span>
          </button>
          <button class="house" class:is-base={isBase} id="base-branch-{name}" disabled={isBase} onclick={() => model.makeBase(name)}
            title={isBase ? `${name} is the base` : `Make ${name} the base`}><House size={14} /></button>
        </div>
      {:else}
        <span class="empty">No branches match</span>
      {/each}
    </div>
  </Popover.Content>
</Popover.Root>

<style>
  .why { padding: 2px 6px 8px; font-size: 11.5px; line-height: 1.4; color: var(--tertiary); }
  .search { display: flex; align-items: center; gap: 6px; height: 30px; margin-bottom: 4px; padding: 0 8px; border-radius: 7px; color: var(--tertiary); background: oklch(0.5 0 0 / 0.08); }
  .search input { flex: 1; min-width: 0; font-size: 12px; color: var(--foreground); background: none; outline: none; }
  .row { display: flex; align-items: center; gap: 2px; border-radius: 7px; }
  .row:hover { background: oklch(0.5 0 0 / 0.08); }
  .pick { display: flex; flex: 1; min-width: 0; align-items: center; gap: 8px; height: 30px; padding: 0 8px; font-size: 12.5px; text-align: left; color: var(--muted-foreground); }
  .row.on .pick { font-weight: 600; color: var(--foreground); }
  .pick:disabled { cursor: default; }
  .box { display: grid; flex-shrink: 0; place-items: center; width: 15px; height: 15px; border-radius: 4px; box-shadow: inset 0 0 0 1.5px oklch(0.5 0 0 / 0.45); color: white; transition: background 0.15s; }
  .box.checked { background: var(--primary); box-shadow: none; }
  .house { display: grid; flex-shrink: 0; place-items: center; width: 28px; height: 28px; border-radius: 6px; color: var(--tertiary); opacity: 0; transition: opacity 0.15s, color 0.15s; }
  .row:hover .house, .house:focus-visible { opacity: 1; }
  .house:hover:not(:disabled) { color: var(--primary); background: oklch(0.5 0 0 / 0.1); }
  .house.is-base { opacity: 1; color: var(--primary); cursor: default; }
  .empty { display: block; padding: 6px 8px; font-size: 11.5px; color: var(--tertiary); }
</style>
