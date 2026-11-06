<script lang="ts">
  // Toolbar button ("Configure"; the names are in its tooltip): what every branch is compared against in this repo.
  // The popover reads top to bottom: the base (one, pinned left), the environments (qa, develop…: detected by name until the user picks), and a search to add one or
  // change the base, with the action spelled out on each result.
  import { ChevronDown, House, Layers, Plus, Search, Settings2, X } from "@lucide/svelte";
  import * as Popover from "$lib/components/ui/popover";
  import type { RepoModel } from "$lib/model.svelte";
  import { localName } from "$lib/refs";

  let { model }: { model: RepoModel } = $props();
  let query = $state("");

  const base = $derived(localName(model.history.base ?? ""));
  const chosen = $derived(model.history.environments.map(localName));
  // Re-read whenever a reload brings new environments (the picks live in prefs, which isn't reactive).
  const custom = $derived(model.history.environments && model.customEnvironments);
  const matches = (name: string) => !query.trim() || name.toLowerCase().includes(query.trim().toLowerCase());
  // What's left to pick: every other branch; only a local one can become the base.
  const others = $derived([...new Set([...model.history.local, ...model.history.allBranches])].filter((n) => n !== base && !chosen.includes(n) && matches(n)));
  const isLocal = (name: string) => model.history.local.includes(name);

</script>

<Popover.Root onOpenChange={(open) => { if (!open) query = ""; }}>
  <Popover.Trigger class="tool" id="compare-open"
    title={`Base: ${base || "none"} · Environments: ${chosen.join(", ") || "none"}${custom ? "" : " (detected by name)"}\nWhat every branch is compared against: how far behind the base, and where it's still missing. Click to change.`}>
    <Settings2 size={14} class="shrink-0" />Configure<ChevronDown size={12} class="shrink-0" />
  </Popover.Trigger>
  <Popover.Content align="start" class="w-[320px] p-2" id="compare-panel">
    <section>
      <h4>Base branch</h4>
      <p class="why">Pinned left. "Behind" counts the commits it has that a branch doesn't.</p>
      <div class="item"><House size={14} /><span class="min-w-0 flex-1 truncate">{base || "None"}</span></div>
    </section>
    <section>
      <h4>Environments {#if !custom && chosen.length}<span class="auto">detected by name</span>{/if}
        {#if custom}<button class="reset" id="environments-reset" title="Forget your picks and detect them by name again (qa, develop, staging…)"
          onclick={() => model.setEnvironments(null)}>Auto-detect</button>{/if}</h4>
      <p class="why">Where work gets deployed. Branches show whether they reached each one.</p>
      {#each chosen as name (name)}
        <div class="item"><Layers size={14} /><span class="min-w-0 flex-1 truncate">{name}</span>
          <button class="remove" id="environment-remove-{name}" title="Stop comparing against {name}" onclick={() => model.setEnvironments(chosen.filter((n) => n !== name))}><X size={13} /></button>
        </div>
      {:else}
        <span class="empty">None yet: add one below.</span>
      {/each}
    </section>
    <section>
      <label class="search"><Search size={12} /><input id="compare-search" placeholder="Find a branch to add or make the base" bind:value={query} /></label>
      <div class="max-h-[220px] overflow-y-auto">
        {#each others as name (name)}
          <div class="result">
            <span class="min-w-0 flex-1 truncate">{name}</span>
            <button class="act" id="environment-add-{name}" title="Compare every branch against {name} too" onclick={() => model.setEnvironments([...chosen, name])}><Plus size={12} />Environment</button>
            {#if isLocal(name)}<button class="act" id="base-branch-{name}" title="Make {name} the base, pinned left" onclick={() => model.setBaseBranch(name)}><House size={12} />Base</button>{/if}
          </div>
        {:else}
          <span class="empty">No branches match</span>
        {/each}
      </div>
    </section>
  </Popover.Content>
</Popover.Root>

<style>
  section { display: flex; flex-direction: column; gap: 2px; padding: 4px 2px 8px; }
  section + section { border-top: 1px solid var(--border); padding-top: 8px; }
  h4 { display: flex; align-items: center; gap: 6px; padding: 0 6px; font-size: 10.5px; font-weight: 700; letter-spacing: 0.04em; text-transform: uppercase; color: var(--muted-foreground); }
  .auto { font-weight: 500; letter-spacing: 0; text-transform: none; color: var(--tertiary); }
  .why { padding: 0 6px 4px; font-size: 11px; line-height: 1.35; color: var(--tertiary); }
  .item { display: flex; align-items: center; gap: 8px; height: 30px; padding: 0 4px 0 8px; border-radius: 7px; font-size: 12.5px; font-weight: 600; color: var(--foreground); background: oklch(0.5 0 0 / 0.08); }
  .item :global(svg:first-child) { flex-shrink: 0; color: var(--primary); }
  .item + .item { margin-top: 2px; }
  .remove { display: grid; place-items: center; width: 24px; height: 24px; border-radius: 6px; color: var(--tertiary); }
  .remove:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.14); }
  .empty { padding: 4px 8px; font-size: 11.5px; color: var(--tertiary); }
  .reset { margin-left: auto; font-size: 11px; font-weight: 600; letter-spacing: 0; text-transform: none; color: var(--primary); }
  .reset:hover { text-decoration: underline; }
  .search { display: flex; align-items: center; gap: 6px; height: 30px; margin-bottom: 4px; padding: 0 8px; border-radius: 7px; color: var(--tertiary); background: oklch(0.5 0 0 / 0.08); }
  .search input { flex: 1; min-width: 0; font-size: 12px; color: var(--foreground); background: none; outline: none; }
  /* Results: the actions appear on hover, in words. */
  .result { display: flex; align-items: center; gap: 4px; height: 30px; padding: 0 4px 0 8px; border-radius: 6px; font-size: 12px; color: var(--muted-foreground); }
  .result:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.08); }
  .act { display: flex; opacity: 0; align-items: center; gap: 3px; height: 22px; padding: 0 7px; border-radius: 6px; font-size: 11px; font-weight: 600; color: var(--primary); background: color-mix(in oklab, var(--primary) 12%, transparent); }
  .act:hover { background: color-mix(in oklab, var(--primary) 22%, transparent); }
  .result:hover .act, .act:focus-visible { opacity: 1; } /* still reachable by keyboard */
</style>
