<!-- Environments (base + qa/develop…), pinned, recent activity in a 2-column grid, then the hidden ones collapsed. -->
<script lang="ts">
  import { ChevronRight, EyeOff, Pin, Plus, Server, Zap } from "@lucide/svelte";
  import * as Popover from "$lib/components/ui/popover";
  import type { Component } from "svelte";
  import type { Card } from "$lib/api";
  import { FILTERS, type BranchFilter } from "$lib/branchFilters";
  import type { RepoModel } from "$lib/model.svelte";
  import BranchCard from "./BranchCard.svelte";
  import InfoButton from "./InfoButton.svelte";
  import Loader from "./Loader.svelte";

  let { model }: { model: RepoModel } = $props();
  let showOld = $state(false);
  let filter = $state<BranchFilter>("all");
  let pickingEnvironment = $state(false);
  let pickingPin = $state(false);
  const localName = (ref: string) => (/^(origin|upstream)\//.test(ref) ? ref.slice(ref.indexOf("/") + 1) : ref);
  const ACTIVE_DAYS = 14; // same as ACTIVE_DAYS in src-tauri/src/dashboard.rs
</script>

{#snippet header(title: string, Icon: Component, count: number, info: string | null)}
  <div class="flex items-center gap-1.5 px-1" class:opacity-50={count === 0}>
    <span class="flex items-center gap-1.5 text-[11px] font-semibold text-muted-foreground uppercase">
      <Icon size={11} strokeWidth={2.5} />{title}&nbsp;&nbsp;{count}
    </span>
    {#if info}<InfoButton {title} text={info} id="dashboard-info-{title}" />{/if}
  </div>
{/snippet}

{#snippet grid(cards: Card[], old: boolean)}
  <div class="grid grid-cols-[repeat(auto-fill,520px)] items-start gap-3">
    {#each cards as card (card.ref)}<BranchCard {card} kind="feature" isOld={old} {model} />{/each}
  </div>
{/snippet}

{#snippet empty(text: string)}
  <span class="-mt-2 px-1 text-[11px] text-tertiary opacity-70">{text}</span>
{/snippet}

{#if model.dashboard}
  {@const dashboard = model.dashboard}
  {@const pinnedTop = dashboard.base ? [dashboard.base, ...dashboard.environments] : dashboard.environments}
  {@const branches = [...dashboard.pinned, ...dashboard.features, ...dashboard.old]}
  {@const base = dashboard.base?.ref ?? null}
  {@const active = FILTERS.find((f) => f.id === filter)!}
  <div class="h-full overflow-auto" id="dashboard">
    <div class="enter-stagger flex flex-col gap-[18px] p-4">
      <!-- Quick filters over pinned, recent and hidden branches: where each one's work stands at a glance. -->
      <div class="flex flex-wrap items-center gap-1.5" id="dashboard-filters">
        {#each FILTERS as f (f.id)}
          {@const count = branches.filter((card) => f.matches(card, base)).length}
          <button class="filter" class:on={filter === f.id} id="dashboard-filter-{f.id}" title={f.help} disabled={f.id !== "all" && !count}
            onclick={() => (filter = f.id)}>
            {f.label}<span class="n">{count}</span>
          </button>
        {/each}
      </div>
      {#if filter !== "all"}
        {@const matched = branches.filter((card) => active.matches(card, base))}
        {@render header(active.label, Zap, matched.length, active.help)}
        {#key filter}<div class="enter-fade">{@render grid(matched, false)}</div>{/key}
      {:else}
      {@render header("Environments", Server, pinnedTop.length,
        "Your long-lived branches: main and where code gets tested or deployed. Ej: main, qa, develop. Missing one? Mark it from a branch's ⌄ menu.")}
      {#if !pinnedTop.length}
        {@render empty("No base branch (main / production)")}
      {:else}
        <div class="grid grid-cols-[repeat(auto-fill,520px)] items-start gap-3">
          {#each pinnedTop as card (card.ref)}
            <BranchCard {card} kind={card === dashboard.base ? "base" : "environment"} {model} />
          {/each}
          {#if !dashboard.environments.length}
            <!-- Next to the base when no qa/develop was found: a quiet link that opens the branch list to pick one. -->
            <Popover.Root bind:open={pickingEnvironment}>
              <Popover.Trigger class="add-env self-start" id="dashboard-add-environment"
                title="Mark a branch as an environment (qa, develop, staging…): it shows up here, and features get a tick for it">
                <Plus size={12} strokeWidth={2.5} />Add an environment
              </Popover.Trigger>
              <Popover.Content align="start" class="w-[260px] p-1.5">
                <span class="px-2 pt-1 text-[12px] font-semibold text-muted-foreground">Which branch is an environment?</span>
                <div class="flex max-h-[280px] flex-col gap-0.5 overflow-auto">
                  {#each dashboard.local.filter((b) => b !== localName(base ?? "")) as branch (branch)}
                    <button class="env-option" id="dashboard-environment-{branch}" title="Mark {branch} as an environment"
                      onclick={() => { pickingEnvironment = false; model.toggleEnvironment(branch); }}>
                      <Server size={12} class="shrink-0 text-tertiary" /><span class="truncate">{branch}</span>
                    </button>
                  {/each}
                </div>
              </Popover.Content>
            </Popover.Root>
          {/if}
        </div>
      {/if}

      {@render header("Pinned", Pin, dashboard.pinned.length,
        "Branches you pinned so they never get lost, however old. Ej: a long-running branch you always come back to. Unpin it from its ⌄ menu.")}
      {#if dashboard.pinned.length}{@render grid(dashboard.pinned, false)}{:else}
        <!-- Quiet link that opens the branch list to pin one. -->
        <Popover.Root bind:open={pickingPin}>
          <Popover.Trigger class="add-env self-start" id="dashboard-add-pin" title="Pin a branch: it stays here, however old it gets">
            <Plus size={12} strokeWidth={2.5} />Pin a branch
          </Popover.Trigger>
          <Popover.Content align="start" class="w-[260px] p-1.5">
            <span class="px-2 pt-1 text-[12px] font-semibold text-muted-foreground">Which branch do you want to keep here?</span>
            <div class="flex max-h-[280px] flex-col gap-0.5 overflow-auto">
              {#each dashboard.local.filter((b) => b !== localName(base ?? "")) as branch (branch)}
                <button class="env-option" id="dashboard-pin-{branch}" title="Pin {branch}"
                  onclick={() => { pickingPin = false; model.togglePin(branch); }}>
                  <Pin size={12} class="shrink-0 text-tertiary" /><span class="truncate">{branch}</span>
                </button>
              {/each}
            </div>
          </Popover.Content>
        </Popover.Root>
      {/if}

      {@render header("Recent activity", Zap, dashboard.features.length,
        `Branches you worked on in the last ${ACTIVE_DAYS} days, plus the one you're on. Ej: touched yesterday → shows here. Too cluttered? Hide it from its ⌄ menu.`)}
      {#if dashboard.features.length}{@render grid(dashboard.features, false)}{:else}{@render empty(`Nothing of yours touched in the last ${ACTIVE_DAYS} days`)}{/if}

      {#if dashboard.old.length}
        <div class="flex items-center gap-1.5">
          <button class="flex items-center gap-1.5" id="dashboard-old-toggle" onclick={() => (showOld = !showOld)}>
            <ChevronRight size={10} strokeWidth={3} class="transition-transform duration-200 {showOld ? 'rotate-90' : ''}" />
            {@render header("Hidden", EyeOff, dashboard.old.length, null)}
          </button>
          <InfoButton title="Hidden" id="dashboard-info-hidden"
            text="Branches you haven't touched in {ACTIVE_DAYS} days, or hid yourself. Nothing is deleted. Ej: last month's merged feature. Bring one back from its ⌄ menu." />
        </div>
        {#if showOld}{@render grid(dashboard.old, true)}{/if}
      {/if}
      {/if}
    </div>
  </div>
{:else}
  <Loader title="Loading branches" detail="Comparing every branch with main, the environments and the server" />
{/if}

<style>
  :global(.add-env) {
    display: inline-flex; align-items: center; gap: 6px; height: 30px; padding: 0 12px; border-radius: 999px;
    font-size: 12px; font-weight: 500; color: var(--tertiary); border: 1px dashed oklch(0.5 0 0 / 0.25);
  }
  :global(.add-env:hover), :global(.add-env[data-state="open"]) { color: var(--foreground); border-color: color-mix(in oklab, var(--primary) 50%, transparent); }
  .env-option { display: flex; align-items: center; gap: 8px; height: 30px; padding: 0 10px; border-radius: 8px; font-size: 13px; text-align: left; }
  .env-option:hover { background: oklch(0.5 0 0 / 0.1); }
  .filter {
    display: inline-flex; align-items: center; gap: 6px; height: 28px; padding: 0 6px 0 12px; border-radius: 999px;
    font-size: 12px; font-weight: 600; color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.08);
  }
  .filter:hover:not(:disabled) { color: var(--foreground); background: oklch(0.5 0 0 / 0.14); }
  .filter:disabled { opacity: 0.45; }
  .filter.on { color: white; background: var(--primary); }
  .n {
    min-width: 18px; padding: 0 5px; border-radius: 999px; font-size: 11px; text-align: center; font-variant-numeric: tabular-nums;
    background: oklch(0.5 0 0 / 0.14);
  }
  .filter.on .n { background: rgb(255 255 255 / 0.22); }
</style>
