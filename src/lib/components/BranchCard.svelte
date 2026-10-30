<!-- One dashboard row, in columns the Dashboard header labels: age | name + origin | vs parent | shipped | changes | menu.
     Cells that need something done are buttons: on hover their second line turns into the action. Click anywhere else opens the graph. -->
<script lang="ts">
  import { GitBranch, ArrowRight, ChevronDown, CloudOff, Ellipsis, Copy, EyeOff, Trash2 } from "@lucide/svelte";
  import { api, type Card } from "$lib/api";
  import { isShipped as shipped } from "$lib/branchFilters";
  import type { RepoModel } from "$lib/model.svelte";
  import * as Popover from "$lib/components/ui/popover";
  import { Button } from "$lib/components/ui/button";
  import ActionRow from "./ActionRow.svelte";
  import BranchActions from "./BranchActions.svelte";
  import PrPrompt from "./PrPrompt.svelte";
  import SquashPrompt from "./SquashPrompt.svelte";
  import StartFeature from "./StartFeature.svelte";
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
  // Shipped everywhere and nothing uncommitted: the row says "finished" and suggests the clean-up instead of the columns.
  const isDone = $derived(isShipped && !card.uncommitted);
  // "vs" column: the branch it was cut from, else the base for features. Ej: ↓ 16 behind / main.
  const against = $derived(card.parent ? localName(card.parent) : kind === "feature" ? baseName : null);
  const behindAgainst = $derived(card.parent ? card.behindParent : kind === "feature" ? card.behindBase : 0);
  // Pull can't fast-forward: both sides have commits, or files would conflict. Then the cell offers the AI prompt.
  const cantPull = $derived(!!s && s.behind > 0 && (s.ahead > 0 || card.conflicts.length > 0));
  const stuckTitle = $derived(
    card.conflicts.length ? `Couldn't update: ${plural(card.conflicts.length, "file")} conflict`
    : s?.ahead ? "Couldn't update: you and origin both have new commits"
    : card.uncommitted ? "Couldn't update: your changes are in the way"
    : "Couldn't update automatically",
  );

  let showUpdate = $state(false);
  let showActions = $state(false);
  // Right-click: the options open where the pointer is, instead of at the ⋯ button.
  let menuAt = $state<{ getBoundingClientRect: () => DOMRect } | null>(null);
  let showSquash = $state(false);
  let showPr = $state(false);
  let showNewBranch = $state(false);
  let showPrompt = $state(false);
  let showShipped = $state(false);
  let confirmDelete = $state<"local" | "remote" | null>(null);
  let prompt = $state("");
  let copied = $state(false);
  // Panels anchor to parts that are always there: after updating, the button that opened them goes away.
  let headerEl = $state<HTMLElement | null>(null);
  let deleteEl = $state<HTMLElement | null>(null); // the shipped panel opens right under its Delete… button

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

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div class="card card-columns" class:current={isCurrent} class:opacity-80={isOld} class:shipped={isShipped} id="dashboard-card-{card.ref}" role="button" tabindex="0"
  onclick={() => model.showInGraph(card)} oncontextmenu={(e) => { e.preventDefault(); const at = new DOMRect(e.clientX, e.clientY, 0, 0); menuAt = { getBoundingClientRect: () => at }; showActions = true; }} onkeydown={(e) => { if (e.key === "Enter" && e.target === e.currentTarget) model.showInGraph(card); }}>
  <div class="age" class:current={isCurrent}
    title={card.last ? `Last commit ${shortAge(card.last.date)} ago, by ${card.last.author}` : ""}>
    {#if card.last}
      {@const a = age(card.last.date)}
      {#if a.n === "now"}<b class="text-[16px]">now</b>{:else}<b>{a.n}</b><span>{a.unit}</span><span class="ago">ago</span>{/if}
    {:else}<b>—</b>{/if}
  </div>

  <!-- Full name, the last segment in bold; under it the origin status. -->
  <div class="min-w-0" bind:this={headerEl}>
    {#if isCurrent}<div class="eyebrow"><i></i>Checked out</div>{/if}
    <div class="name">
      {card.local.slice(0, slash + 1)}<b>{card.local.slice(slash + 1)}</b>
    </div>
    <div class="sub">
      <!-- origin, minimal: green when nobody pushed anything new; a chip to click when there is something to pull. -->
      {#if !s}
        <span class="shrink-0">only on the server</span>
      {:else if s.behind}
        <button class="pull" id="dashboard-card-pull-{card.ref}" onclick={own(cantPull ? openPrompt : () => model.pull(card.local))}
          title={cantPull ? `${stuckTitle}. Click to copy a prompt for your AI agent.${card.conflicts.length ? `\nConflicting files:\n${card.conflicts.join("\n")}` : ""}` : "New commits on origin you don't have yet. Click to pull them."}>
          <span class="label">↓ {s.behind} to pull</span><span class="do">{cantPull ? "Fix with AI…" : `↓ Pull ${s.behind}`}</span>
        </button>
      {:else}
        <span class="ok shrink-0" title={s.hasUpstream ? "Nobody pushed anything you don't have" : "Not on origin, so nobody else can have pushed to it"}>✓ Nothing to pull</span>
      {/if}
      {#if s?.ahead}<span class="ml-1.5 shrink-0 text-primary" title="Commits only on your machine">↑ {s.ahead} to push</span>{/if}
    </div>
  </div>

  {#if isDone}
    <!-- Finished: being behind its parent no longer matters, so one wide cell with what to do next. -->
    <div class="done">
      <div class="flex min-w-0 flex-col gap-0.5">
        <span class="ok text-[12.5px] font-semibold">✓ Finished · in {card.promotion.map(([t]) => localName(t)).join(", ")}</span>
        <span class="text-[11.5px] leading-snug text-tertiary">All its commits are in every environment: safe to delete.</span>
      </div>
      <div class="ml-auto flex shrink-0 gap-1.5">
        <button class="done-btn" id="dashboard-card-delete-{card.ref}" disabled={isCurrent} bind:this={deleteEl}
          title={isCurrent ? "You're on it: check out another branch first" : "Delete it here, or here and on origin. Git refuses if anything isn't merged."}
          onclick={own(() => { confirmDelete = null; showShipped = true; })}><Trash2 size={12} />Delete…</button>
        {#if !isOld}
          <button class="done-btn quiet" id="dashboard-card-hide-{card.ref}" title="Move it to Hidden. Nothing is deleted; bring it back from its ⌄ menu."
            onclick={own(() => model.setFeatureOld(card.local, true))}><EyeOff size={12} />Hide</button>
        {/if}
      </div>
    </div>
  {:else}
  <!-- vs parent: how far behind the branch it came from. -->
  {#if !against}
    <div></div>
  {:else if behindAgainst && card.parent}
    <button class="cell act" class:quiet={dismissed} id="dashboard-card-incoming-{card.ref}" onclick={own(() => (showUpdate = true))}
      title="{against} got {plural(behindAgainst, 'new commit')} since this branch was cut. Click to bring them in or hide this notice.">
      <span class="warn">{#key behindAgainst}<span class="num-in">↓ {behindAgainst} behind</span>{/key}</span><span class="hint">{against}</span><span class="do">Merge or rebase<ChevronDown size={11} strokeWidth={2.5} /></span>
    </button>
  {:else if behindAgainst}
    <div class="cell">
      <span class="warn">{#key behindAgainst}<span class="num-in">↓ {behindAgainst} behind</span>{/key}</span><span class="hint">{against}</span>
    </div>
  {:else}
    <div class="cell"><span class="ok">✓ up to date</span><span class="hint">{against}</span></div>
  {/if}

  <!-- shipped: which environments have it. A × opens the PR prompt. -->
  <div class="ship">
    {#if kind !== "feature"}
      <!-- Environments are where things ship to, not something that ships. -->
      <span class="none" title="This is {kind === 'base' ? 'the base branch' : 'an environment'}: features ship into it, so nothing to track here">{kind === "base" ? "base branch" : "environment"}</span>
    {:else if card.promotion.length}
      <div class="flex flex-wrap gap-1">
        {#each card.promotion as [target, missing] (target)}
          {#if missing === 0}
            <span class="env yes">✓ {localName(target)}</span>
          {:else if kind === "feature"}
            <button class="env" id="dashboard-card-pr-{card.ref}-{localName(target)}" onclick={own(() => (showPr = true))}
              title="Not in {localName(target)} yet: {plural(missing, 'commit')} waiting to merge. Click to open a PR.">× {localName(target)}</button>
          {:else}
            <span class="env" title="Not in {localName(target)} yet: {plural(missing, 'commit')} waiting to merge.">× {localName(target)}</span>
          {/if}
        {/each}
      </div>
    {/if}
  </div>

  <!-- changes: uncommitted work. Only the checked-out branch can have any, the rest show a dash. -->
  {#if card.uncommitted}
    <button class="cell act changes" id="dashboard-card-commit-{card.ref}" onclick={own(() => model.revealWorktree())}
      title="{worktreeText}. Click to open them in the graph to review and commit.">
      <span class="warn flex items-center"><span class="pulse"></span>{#key card.uncommitted.files}<span class="num-in">{plural(card.uncommitted.files, "file")}</span>{/key}</span>
      <span class="hint"><span class="add">+{card.uncommitted.added.toLocaleString()}</span> <span class="del">−{card.uncommitted.removed.toLocaleString()}</span></span>
      <span class="do">Commit…</span>
    </button>
  {:else if model.dashboard?.shelved.includes(card.local)}
    <button class="cell act changes" id="dashboard-card-wip-{card.ref}" onclick={own(() => model.checkout(card.local))}
      title="Uncommitted changes Monti Git put aside when you switched away. They come back when you check this branch out.">
      <span class="wip">WIP saved</span>
      <span class="hint">back on checkout</span>
      <span class="do">Check out</span>
    </button>
  {:else}
    <div class="cell" title={isCurrent ? undefined : "Only the checked-out branch has uncommitted changes"}><span class="hint">{isCurrent ? "clean" : "—"}</span></div>
  {/if}
  {/if}

  <!-- Hidden at rest, slides in on hover: options, check out, and the "view in graph" arrow (the whole card opens the graph too). -->
  <div class="end">
    <span class="reveal">
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <span onclick={(e) => e.stopPropagation()}>
        <Popover.Root bind:open={showActions}>
          <Popover.Trigger class="menu" onclick={() => (menuAt = null)} id="dashboard-card-menu-{card.ref}" title="More options (or right-click the card)"><Ellipsis size={14} strokeWidth={2.5} /></Popover.Trigger>
          <Popover.Content class="w-auto p-2.5" customAnchor={menuAt} align={menuAt ? "start" : "center"}>
            <BranchActions name={card.ref} local={card.local} hash={card.last?.id} isBase={kind === "base"} {isCurrent} {model} dismiss={() => (showActions = false)}
              featureIsOld={kind === "feature" && !isCurrent && !isPinned ? isOld : null}
              onSquash={kind === "feature" && card.featureCommits.length > 1 && card.squashBase ? () => (showSquash = true) : null}
              onOpenPr={kind === "feature" ? () => (showPr = true) : null} onNewBranch={() => (showNewBranch = true)} />
          </Popover.Content>
        </Popover.Root>
      </span>
      {#if !isCurrent}
        <button class="switch" id="dashboard-card-checkout-{card.ref}" title="Check out {card.local}" onclick={own(() => model.checkout(card.ref))}>
          <GitBranch size={14} strokeWidth={2.5} />
        </button>
      {/if}
    </span>
    <span class="graph" title="View in graph"><ArrowRight size={14} /></span>
  </div>
</div>

<Popover.Root bind:open={showUpdate}>
  <Popover.Content customAnchor={headerEl} class="w-auto p-2.5">
    <UpdatePanel {card} {model} {isCurrent} dismiss={() => (showUpdate = false)} />
  </Popover.Content>
</Popover.Root>

<Popover.Root bind:open={showSquash}>
  <Popover.Content customAnchor={headerEl} class="w-auto p-2.5">
    <SquashPrompt {card} {model} dismiss={() => (showSquash = false)} />
  </Popover.Content>
</Popover.Root>

<Popover.Root bind:open={showNewBranch}>
  <Popover.Content customAnchor={headerEl} class="w-auto p-3">
    <StartFeature {model} from={card.local} dismiss={() => (showNewBranch = false)} />
  </Popover.Content>
</Popover.Root>

<Popover.Root bind:open={showPr}>
  <Popover.Content customAnchor={headerEl} class="w-auto p-2.5">
    <PrPrompt {card} {model} dismiss={() => (showPr = false)} />
  </Popover.Content>
</Popover.Root>

<Popover.Root bind:open={showShipped}>
  <Popover.Content customAnchor={deleteEl ?? headerEl} align="end" class="w-[400px] p-2.5" id="dashboard-card-shipped-panel-{card.ref}">
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
        <Button variant="secondary" size="sm" id="shipped-delete-cancel-{card.ref}" onclick={() => (confirmDelete = null)}>Cancel</Button>
        <Button size="sm" id="shipped-delete-confirm-{card.ref}" onclick={() => { model.deleteBranch(card.local, confirmDelete === "remote"); showShipped = false; }}>Delete</Button>
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
  /* Same columns as the header row in Dashboard.svelte. */
  :global(.card-columns) { display: grid; grid-template-columns: 62px minmax(0, 1fr) 124px 150px 150px; align-items: center; column-gap: 12px; padding: 0 12px; }
  .card {
    --green: oklch(0.72 0.17 150); --amber: #f5a524;
    position: relative; padding: 15px 12px; border-radius: 14px; cursor: pointer;
    background: color-mix(in oklab, var(--quinary) 70%, transparent);
    box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.1);
    transition: padding 0.25s var(--ease), box-shadow 0.3s cubic-bezier(0.22, 1, 0.36, 1), background 0.2s, opacity 0.2s ease;
  }
  /* Checked out: the whole row in a subtle dark blue. */
  .card.current { background: color-mix(in oklab, var(--primary) 13%, var(--background)); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--primary) 38%, transparent); }
  .card.shipped { opacity: 0.72; }
  .card.shipped:hover { opacity: 1; }
  .card:hover, .card:focus-visible {
    outline: none;
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--primary) 55%, transparent), 0 10px 26px rgb(0 0 0 / 0.2);
  }
  /* Floats over the card's right end on hover (the changes column is wide enough to leave it room): nothing shifts. */
  .changes { justify-self: start; } /* hugs its text, so its hover box stays clear of the actions */
  .end {
    position: absolute; top: 0; bottom: 0; right: 12px; display: flex; align-items: center; gap: 6px; pointer-events: none; opacity: 0; transition: opacity 0.2s;
  }
  .card:hover .end, .card:focus-within .end, .card:has(:global([data-state="open"])) .end { opacity: 1; pointer-events: auto; }
  .reveal { display: flex; align-items: center; gap: 6px; opacity: 0; transform: translateX(6px); transition: opacity 0.2s, transform 0.25s cubic-bezier(0.22, 1, 0.36, 1); }
  /* Also while its menu is open, or reached with the keyboard. */
  .card:hover .reveal, .card:focus-within .reveal, .card:has(:global([data-state="open"])) .reveal { opacity: 1; transform: none; }
  .switch {
    display: grid; place-items: center; width: 28px; height: 28px; border-radius: 999px;
    color: var(--primary); background: color-mix(in oklab, var(--primary) 14%, transparent); transition: color 0.15s, background 0.15s;
  }
  .switch:hover { color: white; background: var(--primary); }
  .graph { display: grid; place-items: center; width: 16px; height: 24px; color: var(--primary); opacity: 0; transform: translateX(-4px); transition: opacity 0.2s, transform 0.25s cubic-bezier(0.22, 1, 0.36, 1); }
  .card:hover .graph, .card:focus-visible .graph { opacity: 1; transform: translateX(2px); }
  /* Cards ease in one after another when the dashboard opens. */
  @media (prefers-reduced-motion: no-preference) {
    .card { animation: enter 0.32s cubic-bezier(0.2, 0.8, 0.2, 1) both; }
    .card:nth-child(2) { animation-delay: 40ms; } .card:nth-child(3) { animation-delay: 80ms; }
    .card:nth-child(4) { animation-delay: 120ms; } .card:nth-child(5) { animation-delay: 160ms; }
    .card:nth-child(n + 6) { animation-delay: 200ms; }
  }
  .age {
    display: flex; flex-direction: column; align-items: center; justify-content: center; height: 62px;
    border-radius: 12px; color: var(--muted-foreground); background: color-mix(in oklab, var(--muted-foreground) 14%, transparent); /* blue only when checked out */
  }
  .age b { font-size: 20px; font-weight: 800; line-height: 1; }
  .age span { font-size: 9.5px; margin-top: 2px; opacity: 0.85; white-space: nowrap; }
  .age .ago { margin-top: 0; opacity: 0.6; }
  .age.current { color: white; background: var(--primary); box-shadow: 0 6px 18px color-mix(in oklab, var(--primary) 35%, transparent); }
  .eyebrow { display: flex; align-items: center; gap: 5px; margin-bottom: 2px; font-size: 10px; font-weight: 700; letter-spacing: 0.07em; text-transform: uppercase; color: var(--primary); }
  .eyebrow i { width: 6px; height: 6px; border-radius: 999px; background: var(--primary); box-shadow: 0 0 6px var(--primary); }
  .name { font-size: 14px; line-height: 1.35; color: var(--muted-foreground); overflow-wrap: anywhere; }
  .name b { color: var(--foreground); font-weight: 700; }
  .sub { display: flex; align-items: center; min-height: 20px; margin-top: 5px; font-size: 12px; color: var(--tertiary); white-space: nowrap; }
  .wip { color: color(display-p3 1 0.7 0.1); } /* same amber as the WIP row in the graph */
  .ok { color: var(--green); } .warn { color: var(--amber); }
  .add { color: oklch(0.72 0.17 150); } .del { color: oklch(0.68 0.19 25); }
  /* Status cell: status on top, a quiet second line; on hover an actionable cell swaps that line for its action. */
  .cell { display: flex; height: 44px; flex-direction: column; justify-content: center; gap: 1px; padding: 0 10px; border-radius: 9px; font-size: 12px; font-weight: 600; white-space: nowrap; text-align: left; }
  .cell .hint { font-size: 11px; font-weight: 500; color: var(--tertiary); overflow: hidden; text-overflow: ellipsis; }
  .cell .do { display: none; font-size: 11px; font-weight: 600; color: var(--primary); }
  .cell.act:hover { background: color-mix(in oklab, var(--primary) 12%, transparent); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--primary) 45%, transparent); }
  .cell.act:hover .hint { display: none; }
  .cell.act:hover .do { display: flex; align-items: center; gap: 3px; }
  .cell.act:active { transform: translateY(1px); }
  .cell.quiet .warn { color: var(--muted-foreground); } /* update notice hidden by the user */
  /* Badge text lines up with the column label: 3px + the badge's own 7px = the cells' 10px. */
  .ship { display: flex; min-height: 44px; flex-direction: column; justify-content: center; align-items: flex-start; gap: 4px; padding: 0 3px; }
  .env { padding: 1px 7px; border-radius: 6px; font-size: 11px; font-weight: 600; white-space: nowrap; color: var(--tertiary); background: oklch(0.5 0 0 / 0.08); }
  /* Finished row: one cell across vs parent, shipped and changes. */
  /* On hover the row makes just enough room for the actions (… / check out / →) floating at its right end: 40px for
     the changes column's text (it already has free space on its right), the full width for a finished row's buttons. */
  .card:hover, .card:focus-within, .card:has(:global([data-state="open"])) { padding-right: 52px; }
  .done { grid-column: span 3; display: flex; align-items: center; gap: 12px; padding: 0 10px; min-width: 0; transition: padding 0.25s var(--ease); }
  .card:hover .done, .card:focus-within .done, .card:has(:global([data-state="open"])) .done { padding-right: 52px; }
  .done-btn {
    display: flex; align-items: center; gap: 6px; height: 30px; padding: 0 12px; border-radius: 9px; font-size: 12px; font-weight: 600;
    color: var(--primary); background: color-mix(in oklab, var(--primary) 12%, transparent); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--primary) 35%, transparent);
  }
  .done-btn:hover:not(:disabled) { color: white; background: var(--primary); }
  .done-btn:disabled { opacity: 0.45; cursor: not-allowed; }
  .done-btn.quiet { color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.08); box-shadow: none; }
  .done-btn.quiet:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.16); }
  .none { padding-left: 7px; font-size: 11px; font-weight: 500; color: var(--tertiary); opacity: 0.7; }
  .none::before { content: "— "; }
  .env.yes { color: var(--green); background: color-mix(in oklab, var(--green) 14%, transparent); }
  button.env:hover { color: white; background: var(--primary); }
  .pull { flex-shrink: 0; padding: 0 8px; border-radius: 999px; font-weight: 600; color: var(--amber); background: color-mix(in oklab, var(--amber) 14%, transparent); }
  .pull .do { display: none; }
  .pull:hover { color: white; background: var(--primary); }
  .pull:hover .label { display: none; }
  .pull:hover .do { display: inline; }
  .pulse { width: 7px; height: 7px; flex-shrink: 0; margin-right: 6px; border-radius: 999px; background: var(--amber); }
  @media (prefers-reduced-motion: no-preference) { .pulse { animation: pulse 1.8s infinite; } }
  @keyframes pulse { 0% { box-shadow: 0 0 0 0 color-mix(in oklab, var(--amber) 55%, transparent); } 70%, 100% { box-shadow: 0 0 0 6px transparent; } }
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
    display: grid; place-items: center; width: 28px; height: 28px; border-radius: 999px; color: var(--muted-foreground);
    background: oklch(0.5 0 0 / 0.07);
  }
  :global(.menu:hover), :global(.menu[data-state="open"]) { background: oklch(0.5 0 0 / 0.16); }
</style>
