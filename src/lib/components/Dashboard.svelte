<!-- Environments (base + qa/develop…), pinned, recent activity in a 2-column grid, then the hidden ones collapsed. -->
<script lang="ts">
  import { ChevronRight, EyeOff, Pin, Server, Zap } from "@lucide/svelte";
  import type { Component } from "svelte";
  import type { Card } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import BranchCard from "./BranchCard.svelte";
  import InfoButton from "./InfoButton.svelte";
  import Loader from "./Loader.svelte";

  let { model }: { model: RepoModel } = $props();
  let showOld = $state(false);
  const ACTIVE_DAYS = 14; // same as ACTIVE_DAYS in src-tauri/src/dashboard.rs
</script>

{#snippet header(title: string, Icon: Component, count: number, info: string | null)}
  <div class="flex items-center gap-1.5 px-1">
    <span class="flex items-center gap-1.5 text-[11px] font-semibold text-muted-foreground uppercase">
      <Icon size={11} strokeWidth={2.5} />{title}&nbsp;&nbsp;{count}
    </span>
    {#if info}<InfoButton {title} text={info} id="dashboard-info-{title}" />{/if}
  </div>
{/snippet}

{#snippet grid(cards: Card[], old: boolean)}
  <div class="grid grid-cols-2 items-stretch gap-3">
    {#each cards as card (card.ref)}<BranchCard {card} kind="feature" isOld={old} {model} />{/each}
  </div>
{/snippet}

{#snippet empty(text: string)}
  <span class="px-1 text-[12px] text-tertiary">{text}</span>
{/snippet}

{#if model.dashboard}
  {@const dashboard = model.dashboard}
  {@const pinnedTop = dashboard.base ? [dashboard.base, ...dashboard.environments] : dashboard.environments}
  <div class="h-full overflow-auto" id="dashboard">
    <div class="flex flex-col gap-[18px] p-4">
      {@render header("Environments", Server, pinnedTop.length,
        "Your long-lived branches: main and where code gets tested or deployed. Ej: main, qa, develop. Missing one? Mark it from a branch's ⌄ menu.")}
      {#if !pinnedTop.length}
        {@render empty("No base branch (main / production)")}
      {:else}
        <div class="grid grid-cols-2 items-stretch gap-3">
          {#each pinnedTop as card (card.ref)}
            <BranchCard {card} kind={card === dashboard.base ? "base" : "environment"} {model} />
          {/each}
          {#if !dashboard.environments.length}
            <!-- Next to the base when no qa/develop was found: says what's missing and how to add it. -->
            <div class="flex items-center gap-3 rounded-[14px] border border-dashed p-4">
              <div class="grid size-[30px] shrink-0 place-items-center rounded-lg bg-quinary text-tertiary"><Server size={14} /></div>
              <div class="flex flex-col gap-[3px]">
                <span class="text-[13px] font-semibold text-muted-foreground">No environments yet</span>
                <span class="text-[11px] text-tertiary">Got a qa or develop branch? Open its ⌄ menu and mark it as an environment.</span>
              </div>
            </div>
          {/if}
        </div>
      {/if}

      {@render header("Pinned", Pin, dashboard.pinned.length,
        "Branches you pinned so they never get lost, however old. Ej: a long-running branch you always come back to. Unpin it from its ⌄ menu.")}
      {#if dashboard.pinned.length}{@render grid(dashboard.pinned, false)}{:else}{@render empty("Pin a branch from its ⌄ menu to keep it here.")}{/if}

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
    </div>
  </div>
{:else}
  <Loader title="Loading branches" detail="Comparing every branch with main, the environments and the server" />
{/if}
