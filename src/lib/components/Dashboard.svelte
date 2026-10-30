<!-- Environments (base + qa/develop…), pinned, recent activity in one centered column, then the hidden ones collapsed. -->
<script lang="ts">
  import { fade } from "svelte/transition";
  import { flip } from "svelte/animate";
  import { cubicOut } from "svelte/easing";
  import { ChevronRight, EyeOff, MapPin, Pin, Plus, Server, Zap } from "@lucide/svelte";
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
  <div class="flex flex-col gap-3">
    <!-- Cards glide to their new place when the list changes (checkout, sync), and fade out when they leave. -->
    {#each cards as card (card.ref)}
      <div animate:flip={{ duration: 320, easing: cubicOut }} out:fade={{ duration: 150 }}><BranchCard {card} kind="feature" isOld={old} {model} /></div>
    {/each}
  </div>
{/snippet}

<!-- The checked-out branch gets its own section on top, so the lists below leave it out. -->
{#snippet section(cards: Card[], old: boolean)}{@render grid(cards.filter((c) => !model.isCurrent(c.ref)), old)}{/snippet}

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
    <div class="enter-stagger mx-auto flex max-w-[920px] flex-col gap-[18px] px-4 py-4">
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
      <!-- Column labels for every card below (same grid as .card-columns in BranchCard.svelte). -->
      <div class="card-columns col-labels" id="dashboard-column-labels">
        <span>Branch<InfoButton title="Branch" text="Full name, the last part in bold. Under it: origin (✓ Nothing to pull, or ↓ to pull when someone pushed; ↑ yours still to push)." id="dashboard-column-info-Branch" /></span>
        <span>vs parent<InfoButton title="vs parent" text="Commits the branch it was cut from got since, that this one doesn't have. Ej: ↓ 2 behind main → merge or rebase to catch up." id="dashboard-column-info-vs-parent" /></span>
        <span>shipped<InfoButton title="shipped" text="Environments that already have every commit of this branch. Ej: ✓ qa × production → it's tested, not released yet." id="dashboard-column-info-shipped" /></span>
        <span>changes<InfoButton title="changes" text="Uncommitted work on the branch you're on: files and lines added/removed. Click it to review and commit. Other branches show — (git keeps uncommitted changes only where you're checked out)." id="dashboard-column-info-changes" /></span>
      </div>
      {#if filter !== "all"}
        {@const matched = branches.filter((card) => active.matches(card, base))}
        {@render header(active.label, Zap, matched.length, active.help)}
        {#key filter}<div class="enter-fade">{@render grid(matched, false)}</div>{/key}
      {:else}
      {@const current = [...pinnedTop, ...branches].find((c) => model.isCurrent(c.ref))}
      <div class="flex items-center gap-1">
        {@render header("Environments", Server, pinnedTop.length,
          "Your long-lived branches: main and where code gets tested or deployed. Ej: main, qa, develop. Add one with the + next to this title.")}
        <!-- + next to the title: opens the branch list to mark one as an environment. -->
        <Popover.Root bind:open={pickingEnvironment}>
          <Popover.Trigger class="add-icon" id="dashboard-add-environment"
            title="Add an environment (qa, develop, staging…): it shows up here, and features get a tick for it">
            <Plus size={12} strokeWidth={2.5} />
          </Popover.Trigger>
          <Popover.Content align="start" class="w-[260px] p-1.5">
            <span class="px-2 pt-1 text-[12px] font-semibold text-muted-foreground">Which branch is an environment?</span>
            <div class="flex max-h-[280px] flex-col gap-0.5 overflow-auto">
              {#each dashboard.local.filter((b) => b !== localName(base ?? "") && !model.isEnvironment(b)) as branch (branch)}
                <button class="env-option" id="dashboard-environment-{branch}" title="Mark {branch} as an environment"
                  onclick={() => { pickingEnvironment = false; model.toggleEnvironment(branch); }}>
                  <Server size={12} class="shrink-0 text-tertiary" /><span class="truncate">{branch}</span>
                </button>
              {/each}
            </div>
          </Popover.Content>
        </Popover.Root>
      </div>
      {#if !pinnedTop.length}
        {@render empty("No base branch (main / production)")}
      {:else}
        <div class="flex flex-col gap-3">
          {#each pinnedTop as card (card.ref)}
            <BranchCard {card} kind={card === dashboard.base ? "base" : "environment"} {model} />
          {/each}
        </div>
      {/if}

      <!-- Environments always stay on top; when you're on one of them it's highlighted there instead of repeated here. -->
      {#if current && !pinnedTop.includes(current)}
        {@render header("You're on", MapPin, 1, null)}
        <BranchCard card={current} kind="feature" isOld={dashboard.old.includes(current)} {model} />
      {/if}

      <!-- Only when something is pinned; pin from a card's ⌄ menu. -->
      {#if dashboard.pinned.length}
        {@render header("Pinned", Pin, dashboard.pinned.length,
          "Branches you pinned so they never get lost, however old. Ej: a long-running branch you always come back to. Unpin it from its ⌄ menu.")}
        {@render section(dashboard.pinned, false)}
      {/if}

      {@render header("Recent activity", Zap, dashboard.features.length,
        `Branches you worked on in the last ${ACTIVE_DAYS} days, plus the one you're on. Ej: touched yesterday → shows here. Too cluttered? Hide it from its ⌄ menu.`)}
      {#if dashboard.features.length}{@render section(dashboard.features, false)}{:else}{@render empty(`Nothing of yours touched in the last ${ACTIVE_DAYS} days`)}{/if}

      {#if dashboard.old.length}
        <div class="flex items-center gap-1.5">
          <button class="flex items-center gap-1.5" id="dashboard-old-toggle" onclick={() => (showOld = !showOld)}>
            <ChevronRight size={10} strokeWidth={3} class="transition-transform duration-200 {showOld ? 'rotate-90' : ''}" />
            {@render header("Hidden", EyeOff, dashboard.old.length, null)}
          </button>
          <InfoButton title="Hidden" id="dashboard-info-hidden"
            text="Branches you haven't touched in {ACTIVE_DAYS} days, or hid yourself. Nothing is deleted. Ej: last month's merged feature. Bring one back from its ⌄ menu." />
        </div>
        {#if showOld}{@render section(dashboard.old, true)}{/if}
      {/if}
      {/if}
    </div>
  </div>
{:else}
  <Loader title="Loading branches" detail="Comparing every branch with main, the environments and the server" />
{/if}

<style>
  :global(.add-icon) { display: grid; place-items: center; width: 20px; height: 20px; border-radius: 6px; color: var(--tertiary); }
  :global(.add-icon:hover), :global(.add-icon[data-state="open"]) { color: var(--foreground); background: oklch(0.5 0 0 / 0.12); }
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
  .col-labels { position: sticky; top: 0; z-index: 2; margin: -8px 0 -10px; padding-top: 6px; padding-bottom: 6px; font-size: 10px; font-weight: 600; letter-spacing: 0.05em; text-transform: uppercase; color: var(--tertiary); background: var(--background); }
  .col-labels span:first-child { grid-column: 2; }
  .col-labels span:not(:first-child) { padding-left: 10px; }
  .col-labels span { display: flex; align-items: center; gap: 5px; }
</style>
