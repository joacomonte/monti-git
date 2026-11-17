<!-- "Update from main": first simulates every way in memory (advice.rs: who pushed, merge commits, conflicts per merge and
     per commit), then lists them with the recommended one on top and why. Hovering an option for a moment opens a
     flyout beside the dialog (so the dialog never grows) with what it does and each git command it runs. On the
     checked-out branch a conflict stops for the resolver (this closes; the uncommitted row takes over); on any other it
     aborts cleanly and lists the files. A rebase or squash of a pushed branch offers the force push as its own step. -->
<script lang="ts">
  import { onMount, type Component } from "svelte";
  import { fly, slide } from "svelte/transition";
  import { ArrowUpDown, Check, CircleCheck, CloudUpload, Combine, GitMerge, LoaderCircle, Sparkles, TriangleAlert } from "@lucide/svelte";
  import { api, WORKTREE_ID, type Advice, type Card, type UpdateMethod } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import { localName } from "$lib/refs";
  import { plural } from "$lib/format";
  import { Button } from "$lib/components/ui/button";
  import ActionRow from "./ActionRow.svelte";

  type Props = { card: Card; model: RepoModel; dismiss: () => void };
  let { card, model, dismiss }: Props = $props();

  // The simulation takes milliseconds: keep the analysis on screen long enough to be read, not flash.
  const MIN_ANALYSIS_MS = 900;
  const HOVER_MS = 450; // "hover a moment": long enough not to pop up while just passing over the rows
  const FLYOUT_WIDTH = 300;

  const isCurrent = $derived(model.isCurrent(card.ref));
  const parentName = $derived(model.history.base ? localName(model.history.base) : "its base");
  const branch = $derived(card.local);
  let step = $state<"analyzing" | "choose" | "working" | "done" | "pushed">("analyzing");
  let advice = $state<Advice | null>(null); // null after analyzing: it failed, the options show without a pick
  let error = $state<string | null>(null);
  let used = $state<UpdateMethod>("merge");

  const own = $derived(advice?.own ?? card.firstParentCount);
  const incoming = $derived(advice?.incoming ?? card.behindBase);
  const pushed = $derived(advice?.pushed ?? card.hasUpstream);
  const needsForcePush = $derived(pushed && used !== "merge");
  const blocked = $derived(!!advice && advice.recommended === null);

  onMount(async () => {
    const repo = model.repo, parent = model.history.base;
    const started = Date.now();
    const result = repo && parent ? await api.updateAdvice(repo, card.local, parent).catch(() => null) : null;
    await new Promise((resolve) => setTimeout(resolve, Math.max(0, MIN_ANALYSIS_MS - (Date.now() - started))));
    advice = result;
    step = "choose";
  });

  type Prediction = { ok: boolean | null; text: string };
  type Option = { method: UpdateMethod; title: string; detail: string; icon: Component; color: string; available: boolean; prediction: Prediction };

  const files = (list: string[]) => list.length <= 2 ? list.join(", ") : `${list.slice(0, 2).join(", ")} +${list.length - 2}`;

  function prediction(method: UpdateMethod): Prediction {
    if (!advice) return { ok: null, text: "" };
    const conflicts = advice.mergeConflicts;
    if (method === "rebase") {
      if (advice.rebaseConflicts === null) return { ok: null, text: "Not simulated" };
      return advice.rebaseConflicts ? { ok: false, text: "Conflicts on a commit" } : { ok: true, text: "No conflicts" };
    }
    if (conflicts.length === 0) return { ok: true, text: "No conflicts" };
    return { ok: false, text: method === "merge" ? `Conflicts in ${files(conflicts)}` : `Unavailable: conflicts` };
  }

  // The recommended way first, then from the plainest to the most specific.
  const options = $derived.by((): Option[] => {
    const all: Option[] = [
      { method: "merge", title: "Merge", icon: GitMerge, color: "#34c77b", available: true, prediction: prediction("merge"),
        detail: `${parentName}'s changes come in with one merge commit.` },
      { method: "rebase", title: "Rebase", icon: ArrowUpDown, color: "#8b7bff", available: true, prediction: prediction("rebase"),
        detail: `Your ${plural(own, "commit")} move on top of ${parentName}. No merge commit.` },
      { method: "squashRebase", title: "Squash + rebase", icon: Combine, color: "#e0a03a", available: !advice || advice.mergeConflicts.length === 0, prediction: prediction("squashRebase"),
        detail: `Your ${own} commits become one, on top of ${parentName}.` },
    ];
    const shown = all.filter((o) => o.method !== "squashRebase" || own > 1);
    const first = advice?.recommended;
    return first ? [...shown.filter((o) => o.method === first), ...shown.filter((o) => o.method !== first)] : shown;
  });

  type Explanation = { what: string; commands: { cmd: string; meaning: string }[]; conflicts: string; warnings: string[] };

  function explain(method: UpdateMethod): Explanation {
    const p = parentName, commits = plural(own, "commit");
    const worktree = isCurrent ? [] : [{ cmd: "git worktree add", meaning: "works in a temporary folder: your files stay put" }];
    const forcePush = pushed ? [{ cmd: "git push --force-with-lease", meaning: "a separate step you confirm: updates the server, and refuses if someone pushed meanwhile" }] : [];
    const onConflict = (op: string) => isCurrent ? `it stops so you pick which lines stay. Abort (git ${op} --abort) puts it all back.` : `it undoes itself (git ${op} --abort) and lists the files. Nothing changes.`;
    const warnings: string[] = [];
    if (advice && method !== "merge" && advice.others.length) warnings.push(`${advice.others.join(", ")} pushed here too: this rewrites their commits.`);
    if (advice && method === "rebase" && advice.merges) warnings.push("Flattens your merge commits; old conflicts may come back.");
    if (advice && method === "rebase" && advice.rebaseConflicts) warnings.push("Commit by commit it clashes: you may resolve the same lines more than once.");

    if (method === "merge") return {
      what: `Adds one commit that brings ${p} in. Your ${commits} stay exactly as they are.`,
      commands: [...worktree, { cmd: `git merge --no-edit ${p}`, meaning: `joins ${p} with a merge commit, default message` }],
      conflicts: `On a conflict, ${onConflict("merge")}`,
      warnings,
    };
    if (method === "rebase") return {
      what: `Moves your ${commits} on top of ${p}'s latest, as if you branched today. New hashes, same changes.`,
      commands: [...worktree, { cmd: `git rebase ${p}`, meaning: `replays your commits on top of ${p}, oldest first` }, ...forcePush],
      conflicts: `On a conflict, ${onConflict("rebase")}`,
      warnings,
    };
    return {
      what: `Combines your ${commits} into one, with all their messages, on top of ${p}.`,
      commands: [
        { cmd: "git merge-tree --write-tree", meaning: `computes your code combined with ${p}, in memory` },
        { cmd: "git commit-tree", meaning: "creates the single commit from that result" },
        isCurrent ? { cmd: "git reset --keep", meaning: "moves your branch to it, keeping untracked files" } : { cmd: "git update-ref", meaning: "moves your branch to it, without a checkout" },
        ...forcePush,
      ],
      conflicts: "Only runs without conflicts: with any, nothing changes.",
      warnings,
    };
  }

  // The flyout: opens beside the dialog after hovering an option a moment, level with that option, then follows the
  // pointer between options at once. On the right, or on the left when the window has no room there.
  let root = $state<HTMLElement>();
  let explained = $state<UpdateMethod | null>(null);
  let flyTop = $state(0);
  let flySide = $state<"right" | "left">("right");
  let timer: ReturnType<typeof setTimeout> | undefined;

  function hover(method: UpdateMethod, row: HTMLElement) {
    clearTimeout(timer);
    const place = () => {
      if (!root) return;
      const box = root.getBoundingClientRect();
      flyTop = row.getBoundingClientRect().top - box.top;
      flySide = box.right + FLYOUT_WIDTH + 24 <= window.innerWidth ? "right" : "left";
      explained = method;
    };
    if (explained) place();
    else timer = setTimeout(place, HOVER_MS);
  }
  function leave() {
    clearTimeout(timer);
    timer = setTimeout(() => (explained = null), 200);
  }
  const explanation = $derived(explained ? explain(explained) : null);
  const explainedOption = $derived(options.find((o) => o.method === explained));

  async function run(method: UpdateMethod) {
    used = method;
    error = null;
    explained = null;
    step = "working";
    const result = await model.updateFromParent(card, method);
    if (result.ok && model.history.conflict) dismiss(); // stopped on conflicts: the resolver takes over
    else if (result.ok) step = "done";
    else {
      error = result.error;
      step = "choose";
    }
  }

  async function push() {
    error = null;
    const result = await model.push(card.local, true);
    if (result.ok) step = "pushed";
    else error = result.error;
  }

  function goToChanges() {
    model.select(WORKTREE_ID);
    dismiss();
  }

  const working = $derived({ merge: `Merging ${parentName} into ${branch}…`, rebase: `Rebasing ${branch} onto ${parentName}…`, squashRebase: `Squashing ${branch} onto ${parentName}…` }[used]);
  const doneText = $derived({ merge: `${parentName}'s changes are merged into this branch.`, rebase: `Your commits now sit on top of ${parentName}.`, squashRebase: `Your work is now one commit on top of ${parentName}.` }[used]);
  const analysisSteps = $derived([
    `Counting your commits and ${parentName}'s new ones`,
    `Checking whether anyone else pushed to ${branch}`,
    `Simulating a merge, in memory`,
    `Replaying your commits one by one, in memory`,
  ]);
</script>

{#snippet title(text: string, detail: string)}
  <div class="flex items-start gap-2.5">
    <CircleCheck size={20} class="shrink-0 text-primary" />
    <div class="flex flex-col gap-0.5">
      <span class="text-[13px] font-semibold">{text}</span>
      <span class="text-[11px] text-muted-foreground">{detail}</span>
    </div>
  </div>
{/snippet}

{#snippet chip(p: Prediction)}
  <span class="chip" class:good={p.ok === true} class:bad={p.ok === false}>
    {#if p.ok}<CircleCheck size={11} />{:else if p.ok === false}<TriangleAlert size={11} />{/if}{p.text}
  </span>
{/snippet}

<div class="relative flex w-[420px] flex-col gap-3 p-1.5" id="update-panel" bind:this={root}>
  {#if step === "analyzing"}
    <div class="flex flex-col gap-3 px-2 py-1" id="update-analyzing" aria-live="polite">
      <span class="flex items-center gap-2 text-[13px] font-semibold"><Sparkles size={15} class="sparkle text-primary" />Finding the best way to bring in {parentName}…</span>
      <ul class="flex flex-col gap-1.5">
        {#each analysisSteps as text, i (text)}
          <li class="check-step flex items-center gap-2 text-[12px] text-muted-foreground" style:--i={i}>
            <span class="relative grid size-3.5 place-items-center">
              <LoaderCircle size={13} class="spin-icon absolute" /><Check size={13} class="done-icon absolute text-primary" />
            </span>{text}
          </li>
        {/each}
      </ul>
      <div class="flex flex-col gap-2">
        {#each [0, 1] as i (i)}<div class="shimmer h-11 rounded-lg" style:--i={i}></div>{/each}
      </div>
    </div>
  {:else if step === "choose"}
    <div class="flex flex-col gap-0.5 px-2">
      <span class="text-[13px] font-semibold">Bring in {parentName}'s {plural(incoming, "new commit")}</span>
      <span class="text-[11.5px] text-muted-foreground">Added after you created this branch.</span>
    </div>

    {#if advice}
      <div class="verdict" class:blocked id="update-verdict" in:slide={{ duration: 200 }}>
        {#if blocked}<TriangleAlert size={14} class="mt-px shrink-0 text-orange-500" />{:else}<Sparkles size={14} class="mt-px shrink-0 text-primary" />{/if}
        <span class="flex flex-1 flex-col gap-2">
          <span>{advice.reason}</span>
          {#if blocked}<span class="flex justify-end"><Button size="sm" variant="secondary" onclick={goToChanges} id="update-go-to-changes">Go to your changes</Button></span>{/if}
        </span>
      </div>
    {/if}

    <div class="flex flex-col gap-1" role="group" aria-label="Ways to update" onmouseleave={leave}>
      {#each options as option (option.method)}
        {@const recommended = advice?.recommended === option.method}
        <div role="presentation" class="option" class:recommended
          onmouseenter={(e) => hover(option.method, e.currentTarget)} onfocusin={(e) => { explained = null; hover(option.method, e.currentTarget); }}>
          <ActionRow title={recommended ? `${option.title} (recommended)` : option.title} id={`update-option-${option.method}`} detail={option.detail}
            icon={option.icon} color={option.color} disabled={blocked || !option.available} onclick={() => run(option.method)}>
            {#snippet extra()}
              {#if option.prediction.text || (pushed && option.method !== "merge")}
                <span class="mt-1 flex flex-wrap gap-1.5">
                  {#if option.prediction.text}{@render chip(option.prediction)}{/if}
                  {#if pushed && option.method !== "merge"}<span class="chip"><CloudUpload size={11} />Force push after</span>{/if}
                </span>
              {/if}
            {/snippet}
          </ActionRow>
        </div>
      {/each}

      {#if explanation && explainedOption}
        {@const Icon = explainedOption.icon}
        <div class="flyout" class:left={flySide === "left"} style:top="{flyTop}px" style:width="{FLYOUT_WIDTH}px" style:--tint={explainedOption.color}
          id="update-explanation" role="note" transition:fly={{ x: flySide === "left" ? 6 : -6, duration: 160 }} onmouseenter={() => clearTimeout(timer)}>
          <span class="flex items-center gap-1.5 text-[12px] font-semibold text-foreground"><Icon size={13} class="tint" />{explainedOption.title}</span>
          <p>{explanation.what}</p>
          <ul class="commands">
            {#each explanation.commands as command (command.cmd)}
              <li><code>{command.cmd}</code><span>{command.meaning}</span></li>
            {/each}
          </ul>
          <p class="text-tertiary">{explanation.conflicts}</p>
          {#each explanation.warnings as warning (warning)}
            <p class="flex items-start gap-1.5 text-orange-500"><TriangleAlert size={11} class="mt-0.5 shrink-0" />{warning}</p>
          {/each}
        </div>
      {/if}
    </div>

    <span class="px-2 text-[10.5px] text-tertiary">Hover an option for what it does and the git commands it runs.</span>
  {:else if step === "working"}
    <div class="flex items-center gap-2.5 text-[12px]"><LoaderCircle size={14} class="animate-spin" />{working}</div>
  {:else if step === "done"}
    {@render title(`Up to date with ${parentName}`, doneText)}
    {#if needsForcePush}
      <span class="text-[11px] text-muted-foreground">The server still has the old commits. Force push (with lease) replaces them, and refuses if someone else pushed meanwhile.</span>
    {/if}
    <div class="flex justify-end gap-2">
      <Button variant="secondary" size="sm" onclick={dismiss} id="update-close">{needsForcePush ? "Not now" : "Done"}</Button>
      {#if needsForcePush}<Button size="sm" onclick={push} id="update-force-push">Force push</Button>{/if}
    </div>
  {:else}
    {@render title("Pushed", "The server now has the updated branch.")}
    <div class="flex justify-end"><Button size="sm" onclick={dismiss} id="update-done">Done</Button></div>
  {/if}

  {#if error}
    <div class="flex flex-col gap-1.5 rounded-lg bg-orange-500/10 p-2.5">
      <span class="flex items-center gap-1.5 text-[12px] font-semibold text-orange-500">
        <TriangleAlert size={13} />{step === "done" ? "Force push failed. Nothing was overwritten." : "Couldn't update. Nothing changed: it was undone."}
      </span>
      <span class="font-mono text-[11px] whitespace-pre-wrap text-muted-foreground select-text">{error}</span>
      {#if step !== "done"}
        <span class="text-[11px] text-tertiary">Conflicts mean both branches changed the same lines. Check this branch out and update it again: the app stops on the conflicts so you can resolve them.</span>
      {/if}
    </div>
  {/if}
</div>

<style>
  .verdict { display: flex; align-items: flex-start; gap: 8px; margin: 0 4px; padding: 9px 11px; border-radius: 10px; font-size: 12px; line-height: 1.4; background: color-mix(in oklab, var(--primary) 10%, transparent); }
  .verdict.blocked { background: oklch(0.7 0.15 55 / 0.12); }
  .option.recommended { border-radius: 10px; box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--primary) 35%, transparent); }

  .chip { display: inline-flex; align-items: center; gap: 4px; padding: 1px 7px; border-radius: 999px; font-size: 10.5px; font-weight: 500; color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.12); }
  .chip.good { color: rgb(16 185 129); background: rgb(16 185 129 / 0.12); }
  .chip.bad { color: rgb(249 115 22); background: rgb(249 115 22 / 0.12); }

  /* Beside the dialog, level with the hovered option: the panel's own padding (10px) plus a gap. */
  .flyout {
    position: absolute; left: calc(100% + 22px); z-index: 10; display: flex; flex-direction: column; gap: 7px; padding: 11px 13px;
    border-radius: 12px; font-size: 11.5px; line-height: 1.45; color: var(--muted-foreground); background: var(--popover); user-select: text;
    box-shadow: 0 0 0 1px oklch(0.5 0 0 / 0.18), 0 12px 32px rgb(0 0 0 / 0.35);
  }
  .flyout.left { left: auto; right: calc(100% + 22px); }
  .flyout :global(.tint) { color: var(--tint); }
  .commands { display: flex; flex-direction: column; gap: 5px; padding: 8px 9px; border-radius: 8px; background: oklch(0.5 0 0 / 0.08); }
  .commands li { display: flex; flex-direction: column; gap: 1px; }
  .commands code { font-size: 10.5px; color: var(--foreground); }

  /* Analyzing: each check turns from spinner to tick in turn, under a gently pulsing sparkle. */
  .check-step { opacity: 0.45; animation: lit 0.25s ease-out forwards; animation-delay: calc(var(--i) * 190ms + 120ms); }
  .check-step :global(.done-icon) { opacity: 0; animation: lit 0.2s ease-out forwards; animation-delay: calc(var(--i) * 190ms + 180ms); }
  .check-step :global(.spin-icon) { animation: spin 1s linear infinite, gone 0.2s ease-out forwards; animation-delay: 0s, calc(var(--i) * 190ms + 180ms); }
  :global(.sparkle) { animation: pulse 1.2s ease-in-out infinite; }
  .shimmer {
    background: linear-gradient(90deg, oklch(0.5 0 0 / 0.06) 0%, oklch(0.5 0 0 / 0.14) 50%, oklch(0.5 0 0 / 0.06) 100%);
    background-size: 200% 100%; animation: shimmer 1.3s linear infinite; animation-delay: calc(var(--i) * 150ms);
  }
  @keyframes lit { to { opacity: 1; } }
  @keyframes gone { to { opacity: 0; } }
  @keyframes spin { to { transform: rotate(360deg); } }
  @keyframes pulse { 50% { opacity: 0.45; transform: scale(0.88); } }
  @keyframes shimmer { from { background-position: 100% 0; } to { background-position: -100% 0; } }
  @media (prefers-reduced-motion: reduce) {
    .check-step, .check-step :global(.done-icon), .check-step :global(.spin-icon), :global(.sparkle), .shimmer { animation: none; opacity: 1; }
    .check-step :global(.spin-icon) { display: none; }
  }
</style>
