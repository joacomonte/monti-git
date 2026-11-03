<script lang="ts">
  // Toolbar chip: what every branch is compared against in this repo. The base (one, pinned left) and the
  // environments (qa, develop…: detected by name until the user picks).
  import { Check, ChevronDown, Search } from "@lucide/svelte";
  import * as Popover from "$lib/components/ui/popover";
  import { prefs, type RepoModel } from "$lib/model.svelte";

  let { model }: { model: RepoModel } = $props();
  let query = $state("");
  let tab = $state<"base" | "environments">("base");

  const localName = (ref: string) => (/^(origin|upstream)\//.test(ref) ? ref.slice(ref.indexOf("/") + 1) : ref);
  const base = $derived(localName(model.base ?? ""));
  const chosen = $derived(model.environments.map(localName));
  // Re-read whenever a reload brings new environments (prefs itself isn't reactive).
  const custom = $derived(model.environments && model.repo != null && model.repo in prefs.get<Record<string, string[]>>("environments", {}));
  const matches = (name: string) => !query.trim() || name.toLowerCase().includes(query.trim().toLowerCase());
  const bases = $derived(model.localBranches.filter(matches));
  const environments = $derived([...chosen, ...model.allBranches.filter((name) => name !== base && !chosen.includes(name))].filter(matches));

  function toggle(name: string) {
    model.setEnvironments(chosen.includes(name) ? chosen.filter((n) => n !== name) : [...chosen, name]);
  }
</script>

<Popover.Root onOpenChange={(open) => { if (!open) query = ""; }}>
  <Popover.Trigger class="env-trigger" id="compare-open"
    title={`Base: ${base || "none"} · Environments: ${chosen.join(", ") || "none"}${custom ? "" : " (detected by name)"}\nWhat every branch is compared against: how far behind the base, and where it's still missing. Click to change.`}>
    <span class="base">{base || "No base"}</span>
    {#if chosen.length}<span class="names">· {chosen.join(", ")}</span>{/if}
    <ChevronDown size={12} class="shrink-0" />
  </Popover.Trigger>
  <Popover.Content align="start" class="w-[280px] p-1.5" id="compare-panel">
    <div class="tabs">
      <button class:on={tab === "base"} id="compare-tab-base" onclick={() => (tab = "base")}>Base branch</button>
      <button class:on={tab === "environments"} id="compare-tab-environments" onclick={() => (tab = "environments")}>Environments</button>
    </div>
    <label class="search"><Search size={12} /><input id="compare-search" placeholder="Filter branches" bind:value={query} /></label>
    <div class="max-h-[280px] overflow-y-auto">
      {#if tab === "base"}
        {#each bases as branch (branch)}
          <button class="pick" id="base-branch-{branch}" onclick={() => model.setBaseBranch(branch)}>
            <span class="truncate">{branch}</span>{#if branch === base}<Check size={12} strokeWidth={3} class="ml-auto shrink-0 text-primary" />{/if}
          </button>
        {:else}
          <span class="block px-2 py-2 text-[12px] text-muted-foreground">No branches match</span>
        {/each}
      {:else}
        {#each environments as name (name)}
          <button class="pick" id="environment-{name}" onclick={() => toggle(name)}>
            <span class="box" class:on={chosen.includes(name)}>{#if chosen.includes(name)}<Check size={10} strokeWidth={3} />{/if}</span>
            <span class="truncate">{name}</span>
          </button>
        {:else}
          <span class="block px-2 py-2 text-[12px] text-muted-foreground">No branches match</span>
        {/each}
        {#if custom}<button class="reset" id="environments-reset" title="Forget your picks and detect them by name again (qa, develop, staging…)"
          onclick={() => model.setEnvironments(null)}>Auto-detect again</button>{/if}
      {/if}
    </div>
  </Popover.Content>
</Popover.Root>

<style>
  :global(.env-trigger) { display: flex; max-width: 260px; flex-shrink: 0; align-items: center; gap: 5px; height: 28px; padding: 0 9px; border-radius: 8px; font-size: 12px; color: var(--tertiary); }
  :global(.env-trigger:hover), :global(.env-trigger[data-state="open"]) { color: var(--foreground); background: oklch(0.5 0 0 / 0.1); }
  .base { flex-shrink: 0; font-weight: 600; color: var(--muted-foreground); }
  .names { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tabs { display: flex; gap: 2px; margin: 0 2px 6px; padding: 2px; border-radius: 8px; background: oklch(0.5 0 0 / 0.08); }
  .tabs button { flex: 1; height: 24px; border-radius: 6px; font-size: 12px; font-weight: 600; color: var(--muted-foreground); }
  .tabs button.on { color: var(--foreground); background: oklch(0.5 0 0 / 0.16); }
  .reset { display: block; margin: 6px 8px 2px; font-size: 11.5px; color: var(--primary); }
  .reset:hover { text-decoration: underline; }
  .search { display: flex; align-items: center; gap: 6px; height: 28px; margin: 0 2px 4px; padding: 0 8px; border-radius: 7px; color: var(--tertiary); background: oklch(0.5 0 0 / 0.08); }
  .search input { flex: 1; min-width: 0; font-size: 12px; color: var(--foreground); background: none; outline: none; }
  .pick { display: flex; width: 100%; align-items: center; gap: 8px; height: 28px; padding: 0 8px; border-radius: 6px; font-size: 12px; text-align: left; }
  .pick:hover { background: oklch(0.5 0 0 / 0.1); }
  .box { display: grid; flex-shrink: 0; place-items: center; width: 14px; height: 14px; border-radius: 4px; box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.4); color: white; }
  .box.on { background: var(--primary); box-shadow: none; }
</style>
