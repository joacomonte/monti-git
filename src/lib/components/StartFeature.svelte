<!-- "New branch from here", one step at a time: 1 pulls the latest of the base (fetch + compare with origin), 2 the kind
     (feature, fix…) or a hand-written name, 3 ticket + description, 4 only with uncommitted changes: what to do with them;
     then it's created from the up-to-date base and checked out. -->
<script lang="ts">
  import { ArrowLeft, ArrowRight, BadgeCheck, Bug, Check, CircleAlert, GitBranchPlus, Inbox, LoaderCircle, Pencil, Sparkles, Wrench } from "@lucide/svelte";
  import type { Component } from "svelte";
  import { fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { api, type WipMode } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import Example from "./Example.svelte";

  let { model, dismiss, from = null }: { model: RepoModel; dismiss: () => void; from?: string | null } = $props();

  const localName = (ref: string) => (/^(origin|upstream)\//.test(ref) ? ref.slice(ref.indexOf("/") + 1) : ref);
  // `from` (the card it was opened from) first, then the base, the environments and the branch you're on.
  const bases = $derived([...new Set(
    [from, model.dashboard?.base?.ref, ...(model.dashboard?.environments ?? []).map((c) => c.ref), model.dashboard?.current]
      .filter((r): r is string => r != null).map(localName),
  )]);
  let picked = $state<string | null>(null);
  const base = $derived(picked ?? bases[0] ?? null);

  const TYPES = [
    { prefix: "feat", label: "Feature", detail: "Something new", icon: Sparkles },
    { prefix: "fix", label: "Fix", detail: "A bug fix", icon: Bug },
    { prefix: "chore", label: "Chore", detail: "Tooling, config, clean-up", icon: Wrench },
  ];
  let type = $state("feat");
  let ticket = $state("");
  let description = $state("");
  let manual = $state<string | null>(null); // hand-written full name: skips type + ticket + description
  // Accents off ("canción" → "cancion"), anything else → "-".
  const slug = (text: string) => text.toLowerCase().normalize("NFD").replace(/\p{M}/gu, "").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
  const name = $derived(manual != null ? manual.trim()
    : description.trim() ? `${type}/${[slug(ticket), slug(description)].filter(Boolean).join("-")}` : "");

  // How the base compares with origin; fetched every time the base changes.
  let status = $state<{ ahead: number; behind: number; remote: boolean } | null>(null);
  $effect(() => {
    const repo = model.repo, b = base;
    status = null;
    if (repo && b) api.baseStatus(repo, b).then((s) => { if (base === b) status = s; }, () => {});
  });
  const diverged = $derived(status != null && status.ahead > 0 && status.behind > 0);
  // Pulled fine: a beat to read the result, then on to the next step by itself.
  $effect(() => {
    if (step !== "sync" || !status || diverged) return;
    const timer = setTimeout(() => go("type"), 900);
    return () => clearTimeout(timer);
  });

  type Step = "sync" | "type" | "name" | "changes" | "working" | "done";
  let step = $state<Step>("sync");
  let direction = $state(1); // slide in from the right going forward, from the left going back
  let dirty = $state(0);
  let error = $state("");
  const steps = $derived<Step[]>(dirty ? ["sync", "type", "name", "changes"] : ["sync", "type", "name"]);
  const at = $derived(Math.min(steps.indexOf(step) < 0 ? steps.length : steps.indexOf(step), steps.length));

  function go(next: Step, dir = 1) {
    direction = dir;
    step = next;
  }

  /** From the name step: ask about uncommitted changes first, if any. */
  async function next() {
    const repo = model.repo;
    if (!repo || !name) return;
    dirty = await api.changedFileCount(repo);
    if (dirty) go("changes");
    else create(null);
  }

  async function create(wip: WipMode | null) {
    if (!base || !name) return;
    error = "";
    go("working");
    if (await model.startFeature(name, base, wip)) {
      go("done");
      setTimeout(dismiss, 1100);
    } else {
      error = model.syncStatus;
      go("name", -1);
    }
  }

  const focus = (node: HTMLInputElement) => { setTimeout(() => node.focus(), 60); };
</script>

{#snippet kind(title: string, detail: string, Icon: Component, id: string, onclick: () => void)}
  <button class="option" {id} {onclick}>
    <span class="tile"><Icon size={15} /></span>
    <span class="flex min-w-0 flex-1 flex-col text-left">
      <span class="text-[13px] font-semibold">{title}</span>
      <span class="text-[11px] text-muted-foreground">{detail}</span>
    </span>
    <ArrowRight size={13} class="go" />
  </button>
{/snippet}

{#snippet option(title: string, detail: string, Icon: Component, mode: WipMode, example: string)}
  <button class="option" id="start-feature-{mode}" onclick={() => create(mode)}>
    <span class="tile"><Icon size={15} /></span>
    <span class="flex min-w-0 flex-col text-left">
      <span class="text-[13px] font-semibold">{title}</span>
      <span class="text-[11px] text-muted-foreground">{detail}</span>
      <Example text={example} />
    </span>
  </button>
{/snippet}

<div class="flex w-[380px] flex-col gap-3 overflow-hidden" id="start-feature">
  <div class="flex items-center gap-2">
    <GitBranchPlus size={15} class="text-primary" />
    <span class="flex-1 text-[13px] font-semibold">New branch</span>
    {#if step !== "done"}
      <!-- Progress: one pill per step, the current one stretched. -->
      <div class="flex items-center gap-1" aria-hidden="true">
        {#each steps as s, i (s)}<span class="dot" class:done={i < at} class:now={i === at}></span>{/each}
      </div>
    {/if}
  </div>

  <div class="stage">
    {#key step}
      <div class="flex flex-col gap-3" in:fly={{ x: 28 * direction, duration: 260, easing: cubicOut }}>
        {#if step === "sync"}
          <div class="center">
            {#if !status}
              <LoaderCircle size={26} class="spin text-primary" />
              <span class="text-[13px] font-semibold">Pulling the latest changes</span>
              <span class="-mt-2 text-[12px] text-muted-foreground">of <b class="text-foreground">{base}</b> from origin…</span>
            {:else if diverged}
              <span class="badge bad"><CircleAlert size={20} strokeWidth={2.5} /></span>
              <span class="text-[12.5px] text-red-500">{base} diverged from origin ({status.ahead} only here, {status.behind} only there).<br />Sort that out first.</span>
            {:else}
              <span class="badge"><Check size={20} strokeWidth={3} /></span>
              <span class="text-[12.5px]">{!status.remote ? `${base} isn't on origin: starting from your copy`
                : status.behind ? `${status.behind} new commits on ${base}: they'll come along` : `${base} is up to date`}</span>
            {/if}
          </div>
          {#if bases.length > 1 && diverged}
            <div class="flex flex-wrap justify-center gap-1.5">
              {#each bases as b (b)}
                <button class="chip" class:on={base === b} id="start-feature-base-{b}" onclick={() => (picked = b)}>{b}</button>
              {/each}
            </div>
          {/if}
        {:else if step === "type"}
          <span class="label">What is it? <span class="font-normal text-tertiary">from {base}</span></span>
          <div class="flex flex-col gap-1">
            {#each TYPES as t (t.prefix)}
              {@render kind(t.label, t.detail, t.icon, `start-feature-type-${t.prefix}`, () => { type = t.prefix; manual = null; go("name"); })}
            {/each}
          </div>
          <button class="skip" id="start-feature-manual" onclick={() => { manual = ""; go("name"); }}><Pencil size={12} />Skip, I'll write the name</button>
        {:else if step === "name"}
          {#if manual != null}
            <span class="label">Branch name</span>
            <input bind:value={manual} id="start-feature-manual-name" placeholder="feat/global/moa-900-export-csv" class="field font-mono" use:focus
              onkeydown={(e) => { if (e.key === "Enter") next(); }} />
          {:else}
            <span class="label">Ticket and a few words</span>
            <div class="flex gap-1.5">
              <input bind:value={ticket} id="start-feature-ticket" placeholder="MOA-900" class="field w-[96px]" />
              <input bind:value={description} id="start-feature-description" placeholder="export csv" class="field min-w-0 flex-1" use:focus
                onkeydown={(e) => { if (e.key === "Enter") next(); }} />
            </div>
            <span class="name" class:empty={!name} id="start-feature-name">{name || `${type}/moa-900-export-csv`}</span>
          {/if}
          {#if error}<span class="text-[11.5px] text-red-500 select-text">{error}</span>{/if}
          <div class="flex justify-between">
            <button class="ghost" id="start-feature-back-type" onclick={() => go("type", -1)}><ArrowLeft size={13} />Back</button>
            <button class="primary" id="start-feature-create" disabled={!name} onclick={next}>Create branch <ArrowRight size={13} /></button>
          </div>
        {:else if step === "changes"}
          <span class="label">{dirty} modified file{dirty === 1 ? "" : "s"}: what do I do with them?</span>
          <div class="flex flex-col gap-1">
            {@render option("Move to the new branch", "Take your uncommitted changes with you", ArrowRight, "carry",
              "you started the ticket on main without a branch: this branch takes what you already wrote.")}
            {@render option("Stash", "Put them aside on the branch you're leaving; back when you return", Inbox, "shelve",
              "you were mid-way through another task: it waits on its branch while you start this one.")}
            {@render option("WIP commit", "Save them as a WIP commit there; undone when you come back", BadgeCheck, "commit",
              "same as stash, but you'll be away from that task for days: a commit is harder to lose.")}
          </div>
          <div class="flex">
            <button class="ghost" id="start-feature-back-name" onclick={() => go("name", -1)}><ArrowLeft size={13} />Name</button>
          </div>
        {:else if step === "working"}
          <div class="center">
            <LoaderCircle size={26} class="spin text-primary" />
            <span class="text-[12.5px] text-muted-foreground">{status?.behind ? `Updating ${base}, then creating` : "Creating"} <b class="font-mono text-foreground">{name}</b>…</span>
          </div>
        {:else}
          <div class="center">
            <span class="check"><Check size={20} strokeWidth={3} /></span>
            <span class="text-[12.5px]">On <b class="font-mono">{name}</b></span>
          </div>
        {/if}
      </div>
    {/key}
  </div>
</div>

<style>
  .label { font-size: 12px; font-weight: 600; color: var(--muted-foreground); }
  .dot { width: 6px; height: 6px; border-radius: 999px; background: oklch(0.5 0 0 / 0.25); transition: width 0.3s cubic-bezier(0.22, 1, 0.36, 1), background 0.3s; }
  .dot.done { background: color-mix(in oklab, var(--primary) 55%, transparent); }
  .dot.now { width: 18px; background: var(--primary); }
  .stage { display: grid; min-height: 150px; }
  .chip { height: 26px; padding: 0 11px; border-radius: 999px; font-size: 12px; font-weight: 600; color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.08); transition: background 0.15s, color 0.15s; }
  .chip:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.14); }
  .chip.on { color: white; background: var(--primary); }
  .field { height: 32px; padding: 0 10px; border-radius: 8px; font-size: 12.5px; outline: none; background: var(--quinary); }
  .field:focus { box-shadow: 0 0 0 2px color-mix(in oklab, var(--primary) 40%, transparent); }
  .name { align-self: flex-start; max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding: 4px 10px; border-radius: 8px;
    font: 600 12px var(--font-mono, ui-monospace, monospace); color: var(--primary); background: color-mix(in oklab, var(--primary) 12%, transparent); transition: color 0.2s, background 0.2s; }
  .name.empty { color: var(--tertiary); background: oklch(0.5 0 0 / 0.07); }
  .primary { display: inline-flex; align-items: center; gap: 6px; height: 32px; padding: 0 14px; border-radius: 9px; font-size: 12.5px; font-weight: 600; color: white; background: var(--primary); }
  .primary:disabled { opacity: 0.4; }
  .primary:hover:not(:disabled) :global(svg) { transform: translateX(2px); }
  .primary :global(svg) { transition: transform 0.15s; }
  .ghost { display: inline-flex; align-items: center; gap: 5px; height: 32px; padding: 0 10px; border-radius: 9px; font-size: 12px; color: var(--muted-foreground); }
  .ghost:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.08); }
  .option { display: flex; align-items: center; gap: 11px; padding: 8px; border-radius: 10px; }
  .option:hover { background: oklch(0.5 0 0 / 0.1); }
  .tile { display: grid; place-items: center; width: 32px; height: 32px; flex-shrink: 0; border-radius: 8px; color: var(--primary); background: color-mix(in oklab, var(--primary) 14%, transparent); }
  .center { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; min-height: 150px; text-align: center; }
  .badge { display: grid; place-items: center; width: 44px; height: 44px; border-radius: 999px; color: white; background: oklch(0.72 0.17 150); animation: pop 0.45s cubic-bezier(0.34, 1.56, 0.64, 1) both; }
  .badge.bad { background: oklch(0.63 0.21 25); }
  .skip { align-self: center; display: inline-flex; align-items: center; gap: 6px; height: 28px; padding: 0 10px; border-radius: 8px; font-size: 12px; color: var(--tertiary); }
  .skip:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.08); }
  .option :global(.go) { color: var(--tertiary); opacity: 0; transition: opacity 0.15s, transform 0.15s; }
  .option:hover :global(.go) { opacity: 1; transform: translateX(2px); }
  .check { display: grid; place-items: center; width: 44px; height: 44px; border-radius: 999px; color: white; background: oklch(0.72 0.17 150); animation: pop 0.45s cubic-bezier(0.34, 1.56, 0.64, 1) both; }
  :global(.spin) { animation: spin 0.9s linear infinite; }
  :global(.pop) { animation: pop 0.35s cubic-bezier(0.34, 1.56, 0.64, 1) both; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @keyframes pop { from { transform: scale(0.4); opacity: 0; } to { transform: scale(1); opacity: 1; } }
  @media (prefers-reduced-motion: reduce) { .check, .badge, :global(.spin), :global(.pop) { animation: none; } }
</style>
