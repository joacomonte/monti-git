<!-- One dashboard card: how long since the last commit on the left; name, merge ticks, origin status and the last commit
     on the right, then a button for each thing that needs doing. Click anywhere else opens the graph at this branch. -->
<script lang="ts">
  import { ArrowRight, ChevronDown, CloudOff, Copy, EyeOff, Trash2 } from "@lucide/svelte";
  import { api, type Card, type Totals } from "$lib/api";
  import { isShipped as shipped } from "$lib/branchFilters";
  import type { RepoModel } from "$lib/model.svelte";
  import * as Popover from "$lib/components/ui/popover";
  import { Button } from "$lib/components/ui/button";
  import ActionRow from "./ActionRow.svelte";
  import BranchActions from "./BranchActions.svelte";
  import SquashPanel from "./SquashPanel.svelte";
  import UpdatePanel from "./UpdatePanel.svelte";

  type Props = { card: Card; kind: "base" | "environment" | "feature"; isOld?: boolean; model: RepoModel };
  let { card, kind, isOld = false, model }: Props = $props();

  const localName = (ref: string) => (/^(origin|upstream)\//.test(ref) ? ref.slice(ref.indexOf("/") + 1) : ref);
  const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

  /** Age block: big number, then the unit, then "ago". Ej: 3 / years / ago. */
  function age(seconds: number): { n: string; unit: string } {
    const s = Math.max(0, Date.now() / 1000 - seconds);
    const steps: [number, number, string][] = [[3600, 60, "minute"], [86400, 3600, "hour"], [86400 * 14, 86400, "day"],
      [86400 * 60, 604800, "week"], [86400 * 365, 2_592_000, "month"], [Infinity, 31_536_000, "year"]];
    if (s < 60) return { n: "now", unit: "" };
    const [, size, unit] = steps.find(([limit]) => s < limit)!;
    const n = Math.floor(s / size);
    return { n: String(n), unit: `${unit}${n === 1 ? "" : "s"}` };
  }

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
  const dismissed = $derived(model.isUpdateDismissed(card));
  const isPinned = $derived(model.pinnedBranches.includes(card.ref));
  const s = $derived(card.summary);
  // "12 staged · 29 not staged · 3 new": what the next commit would take as is.
  const worktreeText = $derived(
    card.worktree
      ? ([[card.worktree[0], "staged"], [card.worktree[1], "not staged"], [card.worktree[2], "new"]] as const)
          .filter(([n]) => n > 0).map(([n, label]) => `${n} ${label}`).join(" · ")
      : "",
  );
  const isShipped = $derived(kind === "feature" && shipped(card));
  // Background sync fast-forwards whatever it can, so anything still behind origin needs a hand.
  const isStuck = $derived(!!s && s.behind > 0 && !model.isSyncing);
  const stuckTitle = $derived(
    card.conflicts.length ? `Couldn't update: ${plural(card.conflicts.length, "file")} conflict`
    : s?.ahead ? "Couldn't update: you and origin both have new commits"
    : card.uncommitted ? "Couldn't update: your changes are in the way"
    : "Couldn't update automatically",
  );

  let showUpdate = $state(false);
  let showActions = $state(false);
  let showSquash = $state(false);
  let showPrompt = $state(false);
  let showShipped = $state(false);
  let confirmDelete = $state<"local" | "remote" | null>(null);
  let prompt = $state("");
  let copied = $state(false);
  // Panels anchor to parts that are always there: after updating, the button that opened them goes away.
  let headerEl = $state<HTMLElement | null>(null);

  async function openPrompt() {
    const repo = model.repo;
    if (!repo) return;
    prompt = "";
    showPrompt = true;
    prompt = await api.updatePrompt(repo, card.local).catch((error) => `Couldn't build the prompt: ${error}`);
  }

  async function copyPrompt() {
    await navigator.clipboard.writeText(prompt);
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  /** Inner buttons do their own thing instead of opening the graph. */
  const own = (action: () => void) => (e: MouseEvent) => { e.stopPropagation(); action(); };
</script>

<!-- One line; hovering it unfolds the detail below. -->
{#snippet action(id: string, icon: string, title: string, detail: string, tone: "blue" | "grey" | "alert", onclick: () => void, tip = "", lines: Totals | null = null)}
  <button class="action {tone}" {id} onclick={own(onclick)} title={tip}>
    <span class="icon">{icon}</span>
    <span class="flex min-w-0 flex-col text-left">
      <span class="flex items-center gap-2 leading-[22px]">
        <span class="truncate text-[12.5px] font-semibold">{title}</span>
        {#if lines}<span class="counts shrink-0 text-[11.5px] font-semibold"><span class="add">+{lines.added.toLocaleString()}</span> <span class="del">−{lines.removed.toLocaleString()}</span></span>{/if}
      </span>
      <span class="detail"><span class="truncate text-[11.5px] text-muted-foreground">{detail}</span></span>
    </span>
    <ArrowRight size={13} class="arrow shrink-0" />
  </button>
{/snippet}

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div class="card" class:opacity-80={isOld} class:shipped={isShipped} id="dashboard-card-{card.ref}" role="button" tabindex="0"
  onclick={() => model.showInGraph(card)} onkeydown={(e) => { if (e.key === "Enter" && e.target === e.currentTarget) model.showInGraph(card); }}>
  <div class="age" class:current={isCurrent}
    title={card.last ? `Last commit ${shortAge(card.last.date)} ago, by ${card.last.author}` : ""}>
    {#if card.last}
      {@const a = age(card.last.date)}
      {#if a.n === "now"}<b class="text-[22px]">now</b>{:else}<b>{a.n}</b><span>{a.unit}</span><span class="ago">ago</span>{/if}
    {:else}<b>—</b>{/if}
  </div>

  <div class="flex min-w-0 flex-1 flex-col gap-2.5">
    <div bind:this={headerEl}>
      <div class="flex items-center gap-1.5 pr-8 text-[12px] text-tertiary">
        <span class="truncate">{slash >= 0 ? card.local.slice(0, slash + 1) : kind === "base" ? "base branch" : kind === "environment" ? "environment" : "branch"}</span>
        {#if isCurrent}<span class="chip blue shrink-0" title="You're on this branch (checked out)">● checked out</span>{/if}
        <span class="ml-auto flex shrink-0 gap-1">
          {#each card.promotion as [target, missing] (target)}
            <span class="chip {missing === 0 ? 'green' : 'grey'}"
              title={missing === 0 ? `Merged into ${localName(target)}: all your commits are there.` : `Not in ${localName(target)} yet: ${plural(missing, "commit")} waiting to merge.`}>
              {missing === 0 ? "✓" : "✗"} {localName(target)}
            </span>
          {/each}
        </span>
      </div>
      <div class="truncate text-[17px] font-bold" title={card.local}>{card.local.slice(slash + 1)}</div>
      <div class="mt-[3px] flex flex-wrap items-center gap-1.5 text-[12px] text-muted-foreground">
        {#if card.parent}
          <span>↳ from <span class="font-semibold text-foreground">{localName(card.parent)}</span></span>
        {:else if kind !== "base" && card.behindBase > 0}
          <span title="{baseName} has {plural(card.behindBase, 'commit')} this branch doesn't have.">↓ {card.behindBase} behind {baseName}</span>
        {/if}
        {#if !s}
          <span>{card.parent || (kind !== "base" && card.behindBase > 0) ? "· " : ""}only on the server</span>
        {:else if !s.hasUpstream}
          <span>{card.parent || (kind !== "base" && card.behindBase > 0) ? "· " : ""}only on your Mac</span>
        {:else if !s.ahead && !s.behind}
          <span>{card.parent || (kind !== "base" && card.behindBase > 0) ? "· " : ""}✓ synced with origin</span>
        {/if}
        {#if s?.ahead}<span class="chip blue" title="Commits only on your machine">↑ {s.ahead} to push</span>{/if}
        {#if s?.behind}<span class="chip amber" title="New commits on the server you don't have">↓ {s.behind} behind origin</span>{/if}
        {#if model.dashboard?.shelved.includes(card.local)}
          <span class="chip blue" title="Unsaved changes Monti Git put aside when you switched away. They come back when you return.">WIP saved</span>
        {/if}
      </div>
    </div>

    {#if card.last}
      <div class="last" title="{card.last.subject} · {card.last.author}"><span class="font-semibold">Last commit:</span> {card.last.subject}</div>
    {/if}

    {#if isShipped}
      {@render action(`dashboard-card-shipped-${card.ref}`, "✓", `Shipped to ${baseName}`, "All its commits are there. Clean up the branch?", "grey",
        () => { confirmDelete = null; showShipped = true; }, "The work is merged everywhere: hide the branch or delete it.")}
    {:else if card.parent && card.behindParent > 0}
      {@render action(`dashboard-card-incoming-${card.ref}`, "↓", `${plural(card.behindParent, "commit")} behind ${localName(card.parent)}`,
        "Merge or rebase to stay up to date", dismissed ? "grey" : "blue", () => (showUpdate = true),
        `${localName(card.parent)} got ${plural(card.behindParent, "new commit")} since this branch was cut. Click to bring them in or hide this notice.`)}
    {/if}
    {#if card.uncommitted}
      {@render action(`dashboard-card-uncommitted-${card.ref}`, "✎",
        `${plural(card.uncommitted.files, "file")} changed, not committed`,
        worktreeText, "blue", () => model.revealWorktree(),
        "", card.uncommitted)}
    {/if}
    {#if isStuck}
      {@render action(`dashboard-card-stuck-${card.ref}`, "!", stuckTitle, "Copy a prompt for your AI agent to fix it", "alert", openPrompt,
        card.conflicts.length ? `Conflicting files:\n${card.conflicts.join("\n")}` : "")}
    {/if}
  </div>

  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <span class="absolute top-3.5 right-3.5" onclick={(e) => e.stopPropagation()}>
    <Popover.Root bind:open={showActions}>
      <Popover.Trigger class="menu" id="dashboard-card-menu-{card.ref}" title="Actions: check out, pull, copy the name…"><ChevronDown size={10} strokeWidth={3.5} /></Popover.Trigger>
      <Popover.Content class="w-auto p-2.5">
        <BranchActions name={card.ref} local={card.local} hash={card.last?.id} isBase={kind === "base"} {isCurrent} {model} dismiss={() => (showActions = false)}
          featureIsOld={kind === "feature" && !isCurrent && !isPinned ? isOld : null}
          onSquash={kind === "feature" && card.featureCommits.length > 1 && card.squashBase ? () => (showSquash = true) : null} />
      </Popover.Content>
    </Popover.Root>
  </span>
  <span class="graph">View in graph <ArrowRight size={12} /></span>
</div>

<Popover.Root bind:open={showUpdate}>
  <Popover.Content customAnchor={headerEl} class="w-auto p-2.5">
    <UpdatePanel {card} {model} {isCurrent} dismiss={() => (showUpdate = false)} />
  </Popover.Content>
</Popover.Root>

<Popover.Root bind:open={showSquash}>
  <Popover.Content customAnchor={headerEl} class="w-auto p-2.5">
    <SquashPanel {card} {model} dismiss={() => (showSquash = false)} />
  </Popover.Content>
</Popover.Root>

<Popover.Root bind:open={showShipped}>
  <Popover.Content customAnchor={headerEl} class="w-[400px] p-2.5" id="dashboard-card-shipped-panel-{card.ref}">
    {#if !confirmDelete}
      <span class="px-1.5 text-[13px] font-semibold text-muted-foreground">{card.local} is shipped</span>
      <div class="flex flex-col gap-1">
        <ActionRow title="Move to Hidden" detail="Out of Recent activity, nothing deleted. Bring it back from its ⌄ menu." icon={EyeOff} grey
          id="shipped-hide-{card.ref}" onclick={() => { model.setFeatureOld(card.local, true); showShipped = false; }} />
        <ActionRow title="Delete branch" detail={isCurrent ? "You're on it: check out another branch first." : "Only on your Mac (git branch -d). The server copy stays."}
          icon={Trash2} color="#f5a524" disabled={isCurrent} id="shipped-delete-{card.ref}" onclick={() => (confirmDelete = "local")} />
        {#if s?.hasUpstream}
          <ActionRow title="Delete here and on the server" detail={isCurrent ? "You're on it: check out another branch first." : "Also removes it from origin, for the whole team."}
            icon={CloudOff} color="#f5a524" disabled={isCurrent} id="shipped-delete-remote-{card.ref}" onclick={() => (confirmDelete = "remote")} />
        {/if}
      </div>
    {:else}
      <div class="flex flex-col gap-1 px-1.5 pt-1">
        <span class="text-[13px] font-semibold">Delete {card.local}{confirmDelete === "remote" ? " here and on the server" : ""}?</span>
        <span class="text-[12px] text-muted-foreground">
          {confirmDelete === "remote" ? "Your teammates won't see it on origin anymore." : "The copy on the server stays."}
          Git refuses if it has commits that aren't merged, so no work can get lost.
        </span>
      </div>
      <div class="flex justify-end gap-2 px-1.5 pb-1">
        <Button variant="secondary" size="sm" id="shipped-delete-cancel-{card.ref}" title="Go back without deleting" onclick={() => (confirmDelete = null)}>Cancel</Button>
        <Button size="sm" id="shipped-delete-confirm-{card.ref}" title="Delete the branch" onclick={() => { model.deleteBranch(card.local, confirmDelete === "remote"); showShipped = false; }}>Delete</Button>
      </div>
    {/if}
  </Popover.Content>
</Popover.Root>

<Popover.Root bind:open={showPrompt}>
  <Popover.Content customAnchor={headerEl} class="flex w-[480px] flex-col gap-3 p-4" id="dashboard-card-prompt-{card.ref}">
    <div class="flex flex-col gap-0.5">
      <span class="text-[14px] font-semibold">Fix this with your AI agent</span>
      <span class="text-[12px] text-muted-foreground">Paste it into Claude Code, Cursor, Codex… opened in this repo. Nothing was changed on your side.</span>
    </div>
    <pre class="prompt">{prompt || "Building the prompt…"}</pre>
    <button class="copy" id="dashboard-card-copy-prompt-{card.ref}" disabled={!prompt} onclick={copyPrompt}>
      <Copy size={13} />{copied ? "Copied!" : "Copy prompt"}
    </button>
  </Popover.Content>
</Popover.Root>

<style>
  .card {
    --green: oklch(0.72 0.17 150); --amber: #f5a524;
    position: relative; display: flex; gap: 18px; padding: 18px; border-radius: 16px; cursor: pointer;
    background: color-mix(in oklab, var(--quinary) 70%, transparent);
    box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.1);
    transition: box-shadow 0.4s cubic-bezier(0.22, 1, 0.36, 1), padding-bottom 0.45s cubic-bezier(0.22, 1, 0.36, 1), opacity 0.2s ease;
  }
  .card.shipped { opacity: 0.72; }
  .card.shipped:hover { opacity: 1; }
  /* Hover: a bit taller at the bottom, so "View in graph" gets its own room. */
  .card:hover, .card:focus-visible {
    padding-bottom: 42px; outline: none;
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--primary) 55%, transparent), 0 12px 32px rgb(0 0 0 / 0.22);
  }
  .graph {
    position: absolute; right: 18px; bottom: 14px; display: flex; align-items: center; gap: 4px;
    font-size: 12px; font-weight: 600; color: var(--primary); opacity: 0; transform: translateY(6px);
    transition: opacity 0.2s ease, transform 0.3s cubic-bezier(0.22, 1, 0.36, 1); pointer-events: none;
  }
  .card:hover .graph, .card:focus-visible .graph {
    opacity: 1; transform: none; transition: opacity 0.35s ease 0.12s, transform 0.45s cubic-bezier(0.22, 1, 0.36, 1) 0.12s;
  }
  /* Cards ease in one after another when the dashboard opens. */
  @media (prefers-reduced-motion: no-preference) {
    .card { animation: enter 0.32s cubic-bezier(0.2, 0.8, 0.2, 1) both; }
    .card:nth-child(2) { animation-delay: 40ms; } .card:nth-child(3) { animation-delay: 80ms; }
    .card:nth-child(4) { animation-delay: 120ms; } .card:nth-child(5) { animation-delay: 160ms; }
    .card:nth-child(n + 6) { animation-delay: 200ms; }
  }
  .age {
    display: flex; flex-direction: column; align-items: center; justify-content: center; width: 72px; flex-shrink: 0;
    border-radius: 14px; color: var(--muted-foreground); background: color-mix(in oklab, var(--muted-foreground) 14%, transparent); /* blue only when checked out */
  }
  .age b { font-size: 28px; font-weight: 800; line-height: 1; }
  .age span { font-size: 10.5px; margin-top: 4px; opacity: 0.85; white-space: nowrap; }
  .age .ago { margin-top: 0; opacity: 0.6; }
  .age.current { color: white; background: var(--primary); box-shadow: 0 8px 22px color-mix(in oklab, var(--primary) 35%, transparent); }
  .chip { display: inline-flex; align-items: center; padding: 1px 8px; border-radius: 999px; font-size: 11px; font-weight: 600; white-space: nowrap; }
  .chip.blue { color: var(--primary); background: color-mix(in oklab, var(--primary) 15%, transparent); }
  .chip.green { color: var(--green); background: color-mix(in oklab, var(--green) 14%, transparent); }
  .chip.grey { color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.1); }
  .chip.amber { color: var(--amber); background: color-mix(in oklab, var(--amber) 14%, transparent); }
  .last {
    font-size: 12px; line-height: 1.45; color: var(--muted-foreground); opacity: 0.9;
    display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
  }
  .action {
    --tone: var(--primary);
    align-self: flex-start; max-width: 100%; display: inline-flex; align-items: center; gap: 9px; padding: 6px 12px 6px 6px; border-radius: 10px;
    background: color-mix(in oklab, var(--tone) 9%, transparent); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--tone) 28%, transparent);
    transition: background 0.15s, box-shadow 0.15s;
  }
  .action.grey { --tone: var(--muted-foreground); }
  .action.alert { --tone: var(--amber); }
  .action:hover { background: color-mix(in oklab, var(--tone) 16%, transparent); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--tone) 50%, transparent); }
  .counts { font-variant-numeric: tabular-nums; }
  .add { color: oklch(0.72 0.17 150); } .del { color: oklch(0.68 0.19 25); }
  .icon {
    display: grid; place-items: center; width: 22px; height: 22px; border-radius: 7px; flex-shrink: 0; align-self: flex-start;
    font-size: 12px; font-weight: 800; color: var(--tone); background: color-mix(in oklab, var(--tone) 18%, transparent);
  }
  /* Folded to zero height; unfolds smoothly on hover (grid rows animate where height: auto can't). */
  .detail {
    display: grid; grid-template-rows: 0fr; opacity: 0;
    transition: grid-template-rows 0.35s cubic-bezier(0.22, 1, 0.36, 1), opacity 0.25s ease, margin-bottom 0.35s cubic-bezier(0.22, 1, 0.36, 1);
  }
  .detail > span { overflow: hidden; display: block; }
  .action:hover .detail, .action:focus-visible .detail { grid-template-rows: 1fr; opacity: 1; margin-bottom: 2px; }
  .action :global(.arrow) { color: var(--tone); transition: transform 0.15s; }
  .action:hover :global(.arrow) { transform: translateX(3px); }
  .prompt {
    max-height: 300px; overflow: auto; white-space: pre-wrap; user-select: text; cursor: text;
    font: 11.5px/1.55 var(--font-mono, ui-monospace, monospace); padding: 12px 14px; border-radius: 10px;
    background: oklch(0.5 0 0 / 0.08);
  }
  .copy {
    align-self: flex-start; display: inline-flex; align-items: center; gap: 7px; padding: 7px 14px; border-radius: 10px;
    font-size: 12.5px; font-weight: 600; color: white; background: var(--primary);
  }
  .copy:disabled { opacity: 0.5; }
  :global(.menu) {
    display: grid; place-items: center; width: 26px; height: 26px; border-radius: 999px; color: var(--muted-foreground);
    background: oklch(0.5 0 0 / 0.07);
  }
  :global(.menu:hover), :global(.menu[data-state="open"]) { background: oklch(0.5 0 0 / 0.16); }
</style>
