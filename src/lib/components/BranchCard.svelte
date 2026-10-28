<!-- One dashboard card (BranchCardView in Dashboard.swift): header, unsaved work and the branch line on the left,
     status on the right. Read-only summary: every action lives in the ⌄ menu. -->
<script lang="ts">
  import {
    ArrowDown, ArrowRight, ArrowUp, Check, ChevronDown, ChevronRight, CircleDashed, Cloud, CloudOff, CornerDownRight, GitBranch,
    House, Inbox, Minus, Pencil, Server,
  } from "@lucide/svelte";
  import type { Component } from "svelte";
  import type { Card, Totals } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import * as Popover from "$lib/components/ui/popover";
  import BranchActions from "./BranchActions.svelte";
  import InfoButton from "./InfoButton.svelte";
  import SquashPanel from "./SquashPanel.svelte";
  import UpdatePanel from "./UpdatePanel.svelte";

  type Props = { card: Card; kind: "base" | "environment" | "feature"; isOld?: boolean; model: RepoModel };
  let { card, kind, isOld = false, model }: Props = $props();

  /** Plain-language help behind each row's "?". */
  const INFO = {
    cutFrom: "The branch this one was created from. Files and Commits are compared against it. Ej: you created it off main → it says main.",
    promoted: "Did this branch's work already reach main or qa? Ej: \"no, not in main · 3 commits waiting\" = 3 of your commits still need a PR into main. \"yes, in qa\" = it's already on qa.",
    remote: "Your copy vs the server's. Ej: \"2 to push\" = 2 commits only you have; \"not pushed yet\" = it only lives on your computer.",
    wip: "Unsaved changes monti-git put aside when you switched away. Ej: you left with half-done work → it waits here and comes back when you return.",
  };

  const localName = (ref: string) => (/^(origin|upstream)\//.test(ref) ? ref.slice(ref.indexOf("/") + 1) : ref);
  const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

  /** "now", "5m", "3h", "2d", "6w", "4mo". */
  function shortAge(seconds: number) {
    const s = Math.max(0, Date.now() / 1000 - seconds);
    if (s < 60) return "now";
    if (s < 3600) return `${Math.floor(s / 60)}m`;
    if (s < 86400) return `${Math.floor(s / 3600)}h`;
    if (s < 86400 * 14) return `${Math.floor(s / 86400)}d`;
    if (s < 86400 * 60) return `${Math.floor(s / 604800)}w`;
    return `${Math.floor(s / 2_592_000)}mo`;
  }

  const isCurrent = $derived(model.isCurrent(card.ref));
  const slash = $derived(card.local.lastIndexOf("/"));
  const baseName = $derived(localName(model.dashboard?.base?.ref ?? "main"));
  const Icon = $derived(isCurrent ? Check : kind === "base" ? House : kind === "environment" ? Server : GitBranch);
  const dismissed = $derived(model.isUpdateDismissed(card));
  let showUpdate = $state(false);
  let showActions = $state(false);
  let showSquash = $state(false);
  // Panels anchor to parts that are always there: after updating, the pill that opened it goes away.
  let headerEl = $state<HTMLElement | null>(null);
  let lineEl = $state<HTMLElement | null>(null);
  const isPinned = $derived(model.pinnedBranches.includes(card.ref));

  type Status = { icon: Component; text: string; blue: boolean; detail?: string; help?: string };

  /** One line, whatever the number of targets: every "no" wins over the "yes" ones; the tooltip has each count. */
  const promotion = $derived.by((): Status => {
    const pending = card.promotion.filter(([, missing]) => missing > 0);
    const help = card.promotion.map(([t, missing]) => `${localName(t)}: ` + (missing === 0 ? "all in" : `${plural(missing, "commit")} waiting`)).join("\n");
    if (!card.promotion.length) {
      return kind === "base"
        ? { icon: House, text: "it's the base", blue: false, detail: "the others merge into it" }
        : { icon: Minus, text: "nothing to check", blue: false, detail: "no branch to compare with" };
    }
    if (pending.length) {
      return { icon: CircleDashed, text: "not in " + pending.map(([t]) => localName(t)).join(" & "), blue: true,
               detail: `${plural(pending[0][1], "commit")} waiting to merge`, help };
    }
    return { icon: Check, text: "yes, in " + card.promotion.map(([t]) => localName(t)).join(" & "), blue: false, detail: "all your commits are there", help };
  });

  const remote = $derived.by((): Status => {
    const s = card.summary;
    if (!s) return { icon: Cloud, text: "only on the server", blue: false, detail: "no local copy" };
    if (!s.hasUpstream) return { icon: CloudOff, text: "only on your Mac", blue: false, detail: "never pushed" };
    if (!s.ahead && !s.behind) return { icon: Check, text: "synced", blue: false, detail: "same as origin" };
    return {
      icon: s.behind ? ArrowDown : ArrowUp, blue: true,
      text: [s.ahead ? `${s.ahead} to push` : null, s.behind ? `${s.behind} to pull` : null].filter(Boolean).join(" · "),
      detail: s.ahead && s.behind ? "you and the server both have new commits" : s.ahead ? "commits only you have" : "new on the server",
    };
  });

  /** The size of the work, or how old it is when there's nothing on top of its parent. */
  const lineText = $derived.by(() => {
    const n = card.featureCommits.length, files = card.diff?.files ?? 0;
    const parts = [n ? (n >= 100 ? "100+ commits" : plural(n, "commit")) : null, files ? plural(files, "file") : null].filter(Boolean);
    if (!parts.length && card.last) parts.push(`last commit ${shortAge(card.last.date)} ago`);
    return parts.join(" · ");
  });

  const worktreeText = $derived(
    card.worktree
      ? ([[card.worktree[0], "staged"], [card.worktree[1], "not staged"], [card.worktree[2], "new"]] as const)
          .filter(([n]) => n > 0).map(([n, label]) => `${n} ${label}`).join(" · ")
      : "",
  );
</script>

{#snippet counts(t: Totals)}
  <span class="counts"><span class="text-green-600/85 dark:text-green-400/85">+{t.added}</span> <span class="text-red-600/85 dark:text-red-400/85">−{t.removed}</span></span>
{/snippet}

{#snippet status(label: string, info: string, s: Status)}
  <div class="flex flex-col gap-1.5">
    <div class="flex items-center gap-1">
      <span class="text-[9px] font-semibold tracking-[0.6px] text-tertiary uppercase">{label}</span>
      <InfoButton title={label} text={info} id="dashboard-info-{card.ref}-{label}" />
    </div>
    <div class="flex flex-col gap-1 text-[12px]" title={s.help}>
      <div class="flex items-center gap-[5px]">
        <s.icon size={10} strokeWidth={3} class="w-[11px] shrink-0 {s.blue ? 'text-primary' : 'text-muted-foreground'}" />
        <span class="truncate {s.blue ? 'text-primary' : 'text-muted-foreground'}">{s.text}</span>
      </div>
      {#if s.detail}<span class="truncate pl-4 text-[10px] text-tertiary">{s.detail}</span>{/if}
    </div>
  </div>
{/snippet}

<div class="card" class:current={isCurrent} class:opacity-80={isOld} id="dashboard-card-{card.ref}">
  <div class="flex min-w-0 flex-1 flex-col gap-3.5">
    <!-- Header: icon tile, folder prefix dimmed above the bold leaf name, and a subtitle. Always three lines. -->
    <div class="flex items-start gap-2.5" bind:this={headerEl}>
      <div class="tile" class:feature={kind === "feature" && !isCurrent}><Icon size={14} strokeWidth={2.5} /></div>
      <div class="flex min-w-0 flex-col gap-0.5">
        <span class="truncate text-[11px] text-tertiary">{slash >= 0 ? card.local.slice(0, slash + 1) : " "}</span>
        <span class="truncate text-[15px] font-semibold" title={card.local}>{card.local.slice(slash + 1)}</span>
        <div class="flex items-center gap-1.5 text-[11px] text-muted-foreground">
          {#if card.parent}
            <span class="flex shrink-0 items-center gap-1"><CornerDownRight size={9} />from <span class="text-foreground">{localName(card.parent)}</span></span>
            <InfoButton title="Cut from" text={INFO.cutFrom} id="dashboard-info-{card.ref}-cut-from" />
            {#if card.behindParent > 0}
              <!-- The branch it came from got new commits: blue until hidden, then grey (blue again when the parent moves). -->
              <button class="incoming ml-1" data-hidden={dismissed} id="dashboard-card-incoming-{card.ref}" onclick={() => (showUpdate = true)}
                title="{localName(card.parent)} got {plural(card.behindParent, 'new commit')} since this branch was cut. Click to bring them in or hide this notice.">
                {localName(card.parent)} has {plural(card.behindParent, "new commit")}
              </button>
            {/if}
          {:else}
            <span>{kind === "base" ? "base branch" : kind === "environment" ? "environment" : "branch"}</span>
            {#if kind !== "base" && card.behindBase > 0}
              <span class="flex items-center gap-0.5 text-tertiary" title="{baseName} has {plural(card.behindBase, 'commit')} this branch doesn't have.">
                <ArrowDown size={10} />{card.behindBase} behind {baseName}
              </span>
            {/if}
          {/if}
        </div>
      </div>
    </div>

    {#if card.uncommitted}
      <!-- Unsaved work on the checked-out branch: the most important thing on the card. -->
      <button class="banner" id="dashboard-card-uncommitted-{card.ref}" onclick={() => model.revealWorktree()}
        title="Open the graph on your uncommitted changes, to see every file and diff. Ej: check what you're about to commit.">
        <span class="grid size-[18px] shrink-0 place-items-center rounded-full bg-primary text-white"><Pencil size={10} strokeWidth={3} /></span>
        <span class="flex min-w-0 flex-1 flex-col text-left">
          <span class="text-[12px] font-semibold">{plural(card.uncommitted.files, "uncommitted change")} here</span>
          <span class="truncate text-[10px] text-muted-foreground">{worktreeText}</span>
        </span>
        <span class="text-[11px] font-medium">{@render counts(card.uncommitted)}</span>
        <ChevronRight size={10} strokeWidth={3} class="text-tertiary" />
      </button>
    {/if}

    <div class="flex-1"></div>
    <!-- The whole line opens the graph at this branch, its line lit up. -->
    <button class="line" id="dashboard-card-graph-{card.ref}" bind:this={lineEl} onclick={() => model.showInGraph(card)}
      title="{card.last ? `Last commit: ${card.last.subject} · ${card.last.author} · ${shortAge(card.last.date)}\n` : ''}Opens the graph at this branch, with its line highlighted. Ej: see where it split off and what it touches.">
      <span class="truncate">{lineText}</span>
      {#if card.diff?.files}{@render counts(card.diff)}{/if}
      <span class="ml-auto flex shrink-0 items-center gap-1 pl-1.5">View in graph <ArrowRight size={11} /></span>
    </button>
  </div>

  <div class="w-px shrink-0 bg-border"></div>

  <!-- Status column: small and quiet, color only where something needs doing. -->
  <div class="flex w-[210px] shrink-0 flex-col gap-5 pl-1">
    <div class="-mb-2 flex justify-end">
      <Popover.Root bind:open={showActions}>
        <Popover.Trigger class="menu" id="dashboard-card-menu-{card.ref}" title="Actions: check out, pull, copy the name…"><ChevronDown size={10} strokeWidth={3.5} /></Popover.Trigger>
        <Popover.Content class="w-auto p-2.5">
          <BranchActions name={card.ref} local={card.local} isBase={kind === "base"} {isCurrent} {model} dismiss={() => (showActions = false)}
            featureIsOld={kind === "feature" && !isCurrent && !isPinned ? isOld : null}
            onSquash={kind === "feature" && card.featureCommits.length > 1 && card.squashBase ? () => (showSquash = true) : null} />
        </Popover.Content>
      </Popover.Root>
    </div>
    {@render status("Already merged?", INFO.promoted, promotion)}
    {@render status("On the server", INFO.remote, remote)}
    {#if model.dashboard?.shelved.includes(card.local)}
      {@render status("WIP", INFO.wip, { icon: Inbox, text: "saved", blue: true })}
    {/if}
  </div>
</div>

<Popover.Root bind:open={showUpdate}>
  <Popover.Content customAnchor={headerEl} class="w-auto p-2.5">
    <UpdatePanel {card} {model} {isCurrent} dismiss={() => (showUpdate = false)} />
  </Popover.Content>
</Popover.Root>

<Popover.Root bind:open={showSquash}>
  <Popover.Content customAnchor={lineEl} class="w-auto p-2.5">
    <SquashPanel {card} {model} dismiss={() => (showSquash = false)} />
  </Popover.Content>
</Popover.Root>

<style>
  .card {
    display: flex; gap: 16px; padding: 16px; border-radius: 14px;
    background: color-mix(in oklab, var(--quinary) 70%, transparent);
    box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.08);
  }
  .card.current { box-shadow: inset 0 0 0 1.5px var(--primary), 0 0 10px color-mix(in oklab, var(--primary) 35%, transparent); }
  .tile {
    display: grid; place-items: center; width: 30px; height: 30px; flex-shrink: 0; border-radius: 8px;
    color: var(--primary); background: color-mix(in oklab, var(--primary) 14%, transparent);
  }
  .tile.feature { color: var(--muted-foreground); background: color-mix(in oklab, var(--muted-foreground) 14%, transparent); }
  .current .tile { color: white; background: var(--primary); }
  .counts { font-variant-numeric: tabular-nums; white-space: nowrap; }
  :global(.incoming) {
    height: 22px; padding: 0 10px; border-radius: 999px; font-size: 11px; font-weight: 600; white-space: nowrap;
    color: var(--primary); background: color-mix(in oklab, var(--primary) 15%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--primary) 30%, transparent); transition: background 0.12s;
  }
  :global(.incoming:hover) { background: color-mix(in oklab, var(--primary) 28%, transparent); }
  :global(.incoming[data-hidden="true"]) {
    color: var(--tertiary); background: var(--quinary); box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.1);
  }
  .banner {
    display: flex; align-items: center; gap: 10px; padding: 9px 12px; border-radius: 10px;
    background: color-mix(in oklab, var(--primary) 10%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--primary) 30%, transparent);
  }
  .line {
    display: flex; align-items: center; gap: 8px; height: 28px; padding: 0 10px; border-radius: 8px;
    font-size: 11px; font-weight: 500; font-variant-numeric: tabular-nums; color: var(--muted-foreground);
    background: color-mix(in oklab, var(--muted-foreground) 8%, transparent); transition: background 0.12s, color 0.12s;
  }
  .line:hover { color: var(--foreground); background: color-mix(in oklab, var(--muted-foreground) 16%, transparent); }
  :global(.menu) {
    display: grid; place-items: center; width: 24px; height: 24px; border-radius: 999px; color: var(--muted-foreground);
    background: oklch(0.5 0 0 / 0.07);
  }
  :global(.menu:hover), :global(.menu[data-state="open"]) { background: oklch(0.5 0 0 / 0.16); }
</style>
