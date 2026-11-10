<!-- Commit form above the file tree, on the uncommitted-changes row, one question at a time (each step slides in):
       1. New commit, amend the last one, or squash the branch into one?
       2. New commit or squash: write the message yourself, or with AI?  Amend: keep its message, or write a new one?
       3. The message (if any) and the one button that does it; with AI: how long, and the prompt is copied for your agent.
     Commits only what's checked (staged) in the tree. -->
<script lang="ts">
  import { AlignLeft, ArrowLeft, Check, ChevronDown, CloudUpload, FileText, GitCommitHorizontal, Layers, PenLine, Quote, Sparkles, TextQuote, TriangleAlert, Type } from "@lucide/svelte";
  import type { Component } from "svelte";
  import { fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { api } from "$lib/api";
  import { plural } from "$lib/format";
  import type { RepoModel } from "$lib/model.svelte";
  import * as Popover from "$lib/components/ui/popover";
  import { aiPrompt, LENGTH_CHOICES, type AiLength } from "$lib/aiPrompt";
  import { localName } from "$lib/refs";

  let { model }: { model: RepoModel } = $props();
  type Step = "what" | "how" | "write" | "length" | "copied";
  let step = $state<Step>("what");
  let direction = $state(1); // slides forward or back
  type Mode = "new" | "amend" | "squash";
  let mode = $state<Mode>("new");
  let keepMessage = $state(false); // amend without touching the message
  let title = $state("");
  let description = $state("");
  let previous = $state({ title: "", description: "" });
  let pushed = $state(false); // amending (or squashing) would change commits on the server: then it force-pushes
  let showMenu = $state(false);
  let length = $state<AiLength>("concise");

  const amend = $derived(mode === "amend");
  const squash = $derived(mode === "squash");
  const base = $derived(model.history.base ? localName(model.history.base) : null);
  // Squash: a branch of its own (not the base), its commits since the base.
  const canSquash = $derived(!!base && !!model.history.current && model.history.current !== base);
  const VERB = { new: "Commit", amend: "Amend", squash: "Squash" } as const;
  const total = $derived(Object.keys(model.changes).length);
  const staged = $derived(model.stagedCount);
  // A new commit needs staged files and a title; an amend, a new title or files to fold in; a squash, its title.
  const canCommit = $derived(!model.isSyncing && (amend ? (keepMessage ? staged > 0 : title.trim() !== "") : squash ? title.trim() !== "" : staged > 0 && title.trim() !== ""));
  const stagedLine = $derived(staged ? `${staged} of ${plural(total, "file")} staged` : "Check files below to stage them");

  $effect(() => {
    const repo = model.repo, kind = mode === "squash" && base ? { squash: base } : mode === "amend" ? "amend" : null;
    void model.history; // reloaded after commits, pushes and pulls: asked again
    if (!repo) return;
    let stale = false;
    pushed = false;
    if (kind) api.rewritesPushed(repo, kind).then((p) => { if (!stale) pushed = p; }, () => {});
    api.lastCommitMessage(repo).then((message) => {
      if (stale) return;
      const [first, ...rest] = message.split("\n");
      previous = { title: first, description: rest.join("\n").trim() };
    }, () => {});
    return () => (stale = true);
  });

  // The path so far: writing it yourself is three steps, with AI four (how long, then it's copied).
  const ORDER = $derived<Step[]>(step === "length" || step === "copied" ? ["what", "how", "length", "copied"] : ["what", "how", "write"]);
  const prompt = $derived(aiPrompt({ repo: model.repo ?? "", branch: model.history.current ?? "(current)", base, mode, length }));
  function go(to: Step) {
    direction = ORDER.indexOf(to) >= ORDER.indexOf(step) ? 1 : -1;
    step = to;
  }
  const back = () => go(step === "copied" ? "length" : step === "write" || step === "length" ? "how" : "what");

  function pickMode(next: Mode) {
    mode = next;
    go("how");
  }
  function pickHow(how: "myself" | "ai" | "keep" | "rewrite") {
    if (how === "ai") return go("length");
    keepMessage = how === "keep";
    if (how === "rewrite" && !title) {
      title = previous.title; // a starting point: the message you're replacing
      description = previous.description;
    }
    go("write");
    if (!keepMessage) requestAnimationFrame(() => document.getElementById("commit-title")?.focus());
  }

  /** Picking the length copies the prompt right away; a little celebration says so, then it's back to the start. */
  let restart: ReturnType<typeof setTimeout> | undefined;
  async function pickLength(next: AiLength) {
    length = next;
    await navigator.clipboard.writeText(prompt);
    go("copied");
    clearTimeout(restart);
    restart = setTimeout(() => { if (step === "copied") go("what"); }, 2600);
  }
  $effect(() => () => clearTimeout(restart));

  async function commit(push: boolean) {
    showMenu = false;
    if (!canCommit) return;
    const [t, d] = keepMessage ? ["", ""] : [title, description];
    if (await model.commit(t, d, { kind: mode, push, rewrite: pushed })) {
      if (t.trim()) previous = { title: t.trim(), description: d.trim() };
      title = description = "";
      go("what");
    }
  }

  /** ⌘↩ commits, ⌘⇧↩ commits and pushes (a new commit; a rewrite force-pushes anyway); plain Return in the title
   *  jumps to the description. */
  function keydown(e: KeyboardEvent) {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      commit(e.shiftKey && mode === "new");
    } else if (e.key === "Enter" && e.target instanceof HTMLInputElement) {
      e.preventDefault();
      document.getElementById("commit-description")?.focus();
    }
  }
</script>

{#snippet option(id: string, Icon: Component, label: string, detail: string, onclick: () => void)}
  <button class="option" {id} {onclick} title={detail}>
    <span class="tile"><Icon size={22} /></span>
    <span class="label">{label}</span>
  </button>
{/snippet}

<div class="box" id="commit-box">
  <div class="head">
    {#if step !== "what"}<button class="back" id="commit-back" title="Back" onclick={back}><ArrowLeft size={14} /></button>{/if}
    <span class="question">
      {#if step === "what"}Uncommitted changes: what do you want to do?
      {:else if step === "how"}{amend ? "Amend: and its message?" : `${squash ? "Squash" : "New commit"}: who writes the message?`}
      {:else if step === "length"}How long should the description be?
      {:else if step === "copied"}Copied!
      {:else}{amend ? (keepMessage ? "Amend, keeping its message" : "Amend with a new message") : squash ? `Squash ${model.history.current} into one` : "New commit"}{/if}
    </span>
  </div>

  <div class="stage">
    {#key step}
      <div class="pane" in:fly={{ x: 32 * direction, duration: 260, easing: cubicOut }} out:fly={{ x: -32 * direction, duration: 200, easing: cubicOut }}>
        {#if step === "what"}
          <div class="choices">
            {@render option("commit-new", GitCommitHorizontal, "New commit", staged ? stagedLine : "With the files you check below", () => pickMode("new"))}
            {@render option("commit-amend", PenLine, "Amend last commit", `Into “${previous.title || "the last commit"}”`, () => pickMode("amend"))}
            {#if canSquash}
              {@render option("commit-squash", Layers, "Squash the branch", `Its commits since ${base} and your changes, as one`, () => pickMode("squash"))}
            {/if}
          </div>
        {:else if step === "how" && !amend}
          <div class="choices">
            {@render option("commit-write-myself", Type, "I'll write it", "Title and an optional description", () => pickHow("myself"))}
            {@render option("commit-write-ai", Sparkles, "Write it with AI", "Your agent writes the message and commits", () => pickHow("ai"))}
          </div>
        {:else if step === "how"}
          <div class="choices">
            {@render option("amend-keep", Quote, "Keep its message", "Only the checked files go in", () => pickHow("keep"))}
            {@render option("amend-rewrite", Type, "New message", "Starts from the current one", () => pickHow("rewrite"))}
            {@render option("amend-ai", Sparkles, "New message with AI", "Your agent rewrites it", () => pickHow("ai"))}
          </div>
        {:else if step === "length"}
          <div class="choices">
            {#each LENGTH_CHOICES as c, i (c.key)}
              {@render option(`commit-ai-${c.key}`, [TextQuote, AlignLeft, FileText][i], amend || squash ? `Rewrite it, ${c.label.toLowerCase()}` : c.label, c.detail, () => pickLength(c.key))}
            {/each}
          </div>
        {:else if step === "copied"}
          <!-- The prompt is on the clipboard: a check pops with a few sparkles flying off. -->
          <div class="copied" id="commit-ai-copied" role="status">
            <span class="burst">
              <span class="check"><Check size={30} strokeWidth={3} /></span>
              {#each [0, 1, 2, 3, 4, 5] as k (k)}<span class="spark" style:--a="{k * 60}deg"><Sparkles size={12} /></span>{/each}
            </span>
            <span class="big">Prompt copied</span>
            <span class="small">Paste it into your AI agent: {amend || squash ? `it shows you the new message and waits for your OK before ${squash ? "squashing" : "amending"}.` : "it writes the message, you commit it."}</span>
          </div>
        {:else}
          {#if !keepMessage}
            <input id="commit-title" bind:value={title} onkeydown={keydown} autocomplete="off" placeholder={previous.title || "Title"}
              class="h-8 rounded-md bg-quinary px-2.5 text-[13px] font-medium outline-none focus:ring-2 focus:ring-primary/40" />
            <textarea id="commit-description" bind:value={description} onkeydown={keydown} placeholder="Description (optional)"
              class="min-h-[56px] flex-1 resize-none rounded-md bg-quinary px-2.5 py-2 text-[13px] leading-snug outline-none focus:ring-2 focus:ring-primary/40"></textarea>
          {:else}
            <div class="kept"><Quote size={14} class="shrink-0" /><span class="min-w-0 flex-1 truncate">{previous.title}</span></div>
          {/if}
          {#if pushed}
            <p class="pushed-warning" id="commit-rewrite-pushed-warning"><TriangleAlert size={14} class="mt-px shrink-0" />
              Already pushed: {amend ? "amending" : "squashing"} replaces {amend ? "it" : "them"} on the server too (force push). Anyone who already pulled will have to reset to the new {amend ? "one" : "commit"}.</p>
          {/if}
          <div class="flex items-center gap-2">
            <span class="min-w-0 flex-1 truncate text-[12px] text-muted-foreground" id="commit-staged-count">
              {squash ? `${staged ? `${plural(staged, "staged file")} and the` : "The"} branch's commits since ${base}` : amend && !staged ? "Only the message changes" : stagedLine}
            </span>
            <div class="split flex h-8 shrink-0 overflow-hidden rounded-md" class:disabled={!canCommit} class:danger={pushed}>
              <button class="px-3.5 text-[13px] font-semibold" id="commit-button" disabled={!canCommit} onclick={() => commit(false)}
                title="{amend ? 'Amend the last commit' : squash ? 'Squash the branch into one commit' : 'Commit the staged files'} (⌘↩)">{pushed ? `${VERB[mode]} & Force push` : VERB[mode]}</button>
              {#if mode === "new"}
                <Popover.Root bind:open={showMenu}>
                  <Popover.Trigger class="border-l border-white/25 px-1.5" id="commit-options" disabled={!canCommit} title="More commit options">
                    <ChevronDown size={11} strokeWidth={3} />
                  </Popover.Trigger>
                  <Popover.Content align="end" class="w-auto p-1" id="commit-options-menu">
                    <button class="menu flex h-8 items-center gap-2 rounded-md px-2.5 text-[12.5px]" id="commit-and-push" onclick={() => commit(true)}
                      title="Commit the staged files, then push them to the server (sets the upstream if it has none)">
                      <CloudUpload size={13} />Commit & Push<kbd class="ml-3 text-[10.5px] text-tertiary">⌘⇧↩</kbd>
                    </button>
                  </Popover.Content>
                </Popover.Root>
              {/if}
            </div>
          </div>
        {/if}
      </div>
    {/key}
  </div>
</div>

<style>
  .box { display: flex; height: 100%; flex-direction: column; gap: 10px; padding: 10px; overflow: hidden; }
  .head { display: flex; align-items: center; gap: 6px; min-height: 24px; }
  .question { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12.5px; font-weight: 700; color: var(--muted-foreground); }
  .back { display: grid; place-items: center; width: 24px; height: 24px; border-radius: 6px; color: var(--muted-foreground); }
  .back:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.12); }
  /* Steps cross in the same cell, so the box doesn't jump while one slides out and the next slides in. */
  .stage { display: grid; min-height: 0; flex: 1; grid-template-columns: minmax(0, 1fr); }
  .pane { display: flex; min-height: 0; flex-direction: column; gap: 8px; grid-area: 1 / 1; }
  /* Big choices side by side, filling the box: the icon on top, the name, a short line under it. */
  .choices { display: grid; flex: 1; grid-auto-columns: minmax(0, 1fr); grid-auto-flow: column; gap: 8px; min-height: 0; container-type: size; }
  .option { display: flex; min-height: 0; overflow: hidden; flex-direction: column; align-items: center; justify-content: center; gap: 8px; padding: 12px 10px; border-radius: 12px; text-align: center;
    background: oklch(0.5 0 0 / 0.07); box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.14); transition: background 0.15s, box-shadow 0.15s, transform 0.1s; }
  .option:active { transform: scale(0.98); }
  .option:hover { background: oklch(0.5 0 0 / 0.13); }
  .tile { display: grid; flex-shrink: 0; place-items: center; width: 44px; height: 44px; border-radius: 12px; color: var(--primary); background: color-mix(in oklab, var(--primary) 14%, transparent); }
  .label { display: flex; min-height: 2.6em; align-items: center; font-size: 14px; font-weight: 700; line-height: 1.3; } /* two lines' room: one-line names keep their icon level with the others */
  /* Short box: icon beside the name, so the choices still fit whole. */
  @container (max-height: 140px) {
    .option { flex-direction: row; justify-content: flex-start; gap: 10px; padding: 8px 12px; text-align: left; }
    .tile { width: 32px; height: 32px; border-radius: 9px; }
    .label { min-height: 0; font-size: 13px; }
  }
  .kept { display: flex; align-items: center; gap: 8px; padding: 9px 10px; border-radius: 8px; font-size: 13px; color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.08); }
  .copied { display: flex; flex: 1; flex-direction: column; align-items: center; justify-content: center; gap: 8px; text-align: center; }
  .burst { position: relative; display: grid; place-items: center; width: 64px; height: 64px; }
  .check { display: grid; place-items: center; width: 56px; height: 56px; border-radius: 99px; color: white; background: #8b5cf6;
    box-shadow: 0 8px 24px -6px color-mix(in oklab, #8b5cf6 80%, transparent); animation: pop 0.5s cubic-bezier(0.34, 1.56, 0.64, 1) both; }
  .spark { position: absolute; color: #c4b5fd; opacity: 0; animation: spark 0.9s 0.15s ease-out both; }
  .big { font-size: 15px; font-weight: 700; animation: rise 0.4s 0.15s ease-out both; }
  .small { max-width: 300px; font-size: 12px; line-height: 1.4; color: var(--muted-foreground); animation: rise 0.4s 0.25s ease-out both; }
  @keyframes pop { 0% { transform: scale(0.2) rotate(-25deg); opacity: 0; } 100% { transform: none; opacity: 1; } }
  @keyframes spark { 0% { opacity: 0; transform: rotate(var(--a)) translateY(0) scale(0.4); } 30% { opacity: 1; } 100% { opacity: 0; transform: rotate(var(--a)) translateY(-40px) scale(1); } }
  @keyframes rise { from { opacity: 0; transform: translateY(6px); } }
  @media (prefers-reduced-motion: reduce) { .check, .spark, .big, .small { animation: none; } .spark { display: none; } }
  .split { color: white; background: var(--primary); }
  .split.disabled { opacity: 0.45; }
  .split.danger { background: #ea580c; } /* amending a pushed commit: force push */
  .pushed-warning { display: flex; gap: 6px; font-size: 12px; line-height: 1.4; color: #f97316; }
  .split > :global(button:not(:disabled):hover) { background: color-mix(in oklab, var(--primary) 85%, black); }
  .menu:hover { background: var(--quinary); }
</style>
