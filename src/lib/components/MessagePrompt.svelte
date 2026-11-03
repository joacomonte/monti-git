<!-- "Message for AI", one step at a time: where the uncommitted changes go (new commit, amended into the last one, or the
     branch squashed into one), how long the description, then review and copy the prompt for the agent. Opened from the
     uncommitted row's right-click (CommitList) and from next to the Commit button (CommitBox). -->
<script lang="ts">
  import { AlignLeft, ArrowLeft, Check, Copy, FileText, GitCommitHorizontal, Layers, Lock, PenLine, Sparkles, TextQuote } from "@lucide/svelte";
  import type { Component } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import type { RepoModel } from "$lib/model.svelte";
  import Example from "./Example.svelte";

  let { model, onclose, start = null }: { model: RepoModel; onclose: () => void; start?: "new" | "amend" | null } = $props(); // start: skip "where" with that mode

  const steps = ["where", "length", "copy"] as const;
  // svelte-ignore state_referenced_locally
  let step = $state<(typeof steps)[number]>(start ? "length" : "where");
  let direction = $state(1);
  const at = $derived(steps.indexOf(step));
  const go = (to: (typeof steps)[number]) => { direction = steps.indexOf(to) > at ? 1 : -1; step = to; };

  type Mode = "new" | "amend" | "squash";
  type Length = "concise" | "medium" | "long" | "keep"; // keep: amend without touching the message
  // svelte-ignore state_referenced_locally
  let mode = $state<Mode>(start ?? "new");
  let length = $state<Length>("concise");
  let copied = $state(false);

  const branch = $derived(model.current ?? "(current)");
  const base = $derived(model.base?.replace(/^(origin|upstream)\//, "") ?? null);
  // A branch of its own (not the base itself): squashing it means its commits since it split from the base.
  const canSquash = $derived(!!base && !!model.current && model.current !== base);

  const LENGTHS: Record<Exclude<Length, "keep">, string> = {
    concise: "one or two short lines: what changed and why.",
    medium: "a short paragraph or 3–5 bullets: what changed and why, and any notable decision.",
    long: "complete: the problem or context, what changed by area, decisions and trade-offs, how to test it, and anything risky.",
  };

  const prompt = $derived.by(() => {
    if (mode === "amend" && length === "keep") return [
      `Repo: ${model.repo}`,
      `Branch: ${branch}`,
      "Amend my uncommitted changes into the last commit (HEAD) without changing its message (`git commit --amend --no-edit`).",
      "- If something is staged (`git diff --staged`), only that goes in; otherwise everything in `git status` and `git diff`, new files included.",
      "Show me what will be added and wait for my OK before amending. If it's already on origin, it'll need --force-with-lease: don't push, I'll do it myself.",
    ].join("\n");
    const task = {
      new: ["Write a commit message for my uncommitted changes."],
      amend: [
        "Amend my uncommitted changes into the last commit (HEAD), so it stays one commit.",
        "- The message must cover both: read the last commit's message (`git log -1 --format=%B`) and the new changes, and rewrite title and/or body as a summary of all of it.",
      ],
      squash: [
        `Squash ${branch}'s own commits since it split from ${base} (\`git merge-base ${base} HEAD\`) plus my uncommitted changes into one single commit (e.g. \`git reset --soft <that merge-base>\` then commit).`,
        `- The message must cover everything: read those commits' messages (\`git log --format=%B <merge-base>..HEAD\`) and the new changes, and write one title and body that summarize all of it.`,
      ],
    }[mode];
    return [
      `Repo: ${model.repo}`,
      `Branch: ${branch}`,
      ...task,
      "- Read the changes: if something is staged (`git diff --staged`), only that goes in; otherwise everything in `git status` and `git diff`, new files included.",
      "- Title: follow this repo's conventions (look at `git log --oneline -20`: e.g. `feat(scope): …`, a ticket id like MOA-123 from the branch name). Imperative, under ~72 characters.",
      `- Description: ${LENGTHS[length === "keep" ? "concise" : length]}`,
      "- Same language as the recent commit messages.",
      mode === "new"
        ? "Give me just the message in a code block. Don't commit."
        : "Show me the new message and wait for my OK before rewriting anything. If it's already on origin, it'll need --force-with-lease: don't push, I'll do it myself.",
    ].join("\n");
  });

  async function copy() {
    await navigator.clipboard.writeText(prompt);
    copied = true;
    setTimeout(onclose, 700);
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
  }
</script>

<svelte:window onkeydown={keydown} />

<!-- Big choice card: tinted icon tile, title, explanation, and a hover example. Picking one moves to the next step. -->
{#snippet choice(id: string, Icon: Component, color: string, title: string, detail: string, example: string, pick: () => void, on: boolean)}
  <button class="choice" class:on {id} style:--tint={color} onclick={pick}>
    <span class="tile"><Icon size={20} /></span>
    <span class="flex min-w-0 flex-1 flex-col gap-1 text-left">
      <span class="text-[14px] font-semibold">{title}</span>
      <span class="text-[12.5px] leading-relaxed text-muted-foreground">{detail}</span>
      <Example text={example} />
    </span>
  </button>
{/snippet}

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop enter-fade" out:fade|global={{ duration: 150 }} id="message-prompt" onclick={onclose} onmousedown={(e) => e.stopPropagation()}>
  <div class="panel enter-pop" role="dialog" tabindex="-1" aria-labelledby="message-prompt-title" onclick={(e) => e.stopPropagation()}>
    <div class="flex items-center gap-2.5 px-1 pb-1">
      <Sparkles size={17} class="shrink-0 text-[#a78bfa]" />
      <span class="text-[14px] font-semibold" id="message-prompt-title">Commit message by your AI agent</span>
      <span class="ml-auto flex shrink-0 items-center gap-1">
        {#each steps as s, i (s)}<span class="dot" class:done={i < at} class:now={i === at}></span>{/each}
      </span>
    </div>

    <div class="stage">
      {#key step}
        <div class="flex flex-col gap-2.5" in:fly={{ x: 28 * direction, duration: 260, easing: cubicOut }}>
          {#if step === "where"}
            <span class="label">Where do your changes go?</span>
            {@render choice("message-prompt-mode-new", GitCommitHorizontal, "#4f8cff", "New commit",
              "A new commit on top, with a title and description written from your changes.",
              "you finished a piece of work and want it as its own commit.", () => { mode = "new"; go("length"); }, mode === "new")}
            {@render choice("message-prompt-mode-amend", PenLine, "#f5a524", "Amend the last commit",
              "Your changes join the last commit, and its message is rewritten to cover both.",
              "you forgot a file or a fix right after committing: it stays one commit.", () => { mode = "amend"; go("length"); }, mode === "amend")}
            {#if canSquash}
              {@render choice("message-prompt-mode-squash", Layers, "#ec5fa8", "Squash into one",
                `${branch}'s commits since ${base} and your changes become a single commit, with one message summarizing all.`,
                '"wip", "fix typo", "fix again" + your changes become one clean commit.', () => { mode = "squash"; go("length"); }, mode === "squash")}
            {/if}
            <div class="flex justify-end"><button class="ghost" id="message-prompt-cancel" onclick={onclose}>Cancel</button></div>
          {:else if step === "length"}
            <span class="label">{mode === "amend" ? "Update the last commit's message?" : "How long should the description be?"}</span>
            {#if mode === "amend"}
              {@render choice("message-prompt-keep", Lock, "#8e8e93", "Keep the message", "Only add your changes; the title and description stay exactly as they are.",
                "you forgot a file: it goes into the commit and nothing else changes.", () => { length = "keep"; go("copy"); }, length === "keep")}
            {/if}
            {@render choice("message-prompt-concise", TextQuote, "#a78bfa", mode === "amend" ? "Rewrite it, concise" : "Concise", "Title plus one or two lines.",
              '"fix(auth): keep session on refresh" + "The token was dropped when the tab reloaded."', () => { length = "concise"; go("copy"); }, length === "concise")}
            {@render choice("message-prompt-medium", AlignLeft, "#4f8cff", mode === "amend" ? "Rewrite it, medium" : "Medium", "Title plus a short paragraph or a few bullets: what and why.",
              "a feature with two or three parts worth naming.", () => { length = "medium"; go("copy"); }, length === "medium")}
            {@render choice("message-prompt-long", FileText, "#34c77b", mode === "amend" ? "Rewrite it, long" : "Long", "Context, changes by area, decisions, how to test, and risks.",
              "a big change your reviewers will read commit by commit.", () => { length = "long"; go("copy"); }, length === "long")}
            <div class="flex justify-start"><button class="ghost" id="message-prompt-back-where" onclick={() => go("where")}><ArrowLeft size={13} />Back</button></div>
          {:else}
            <span class="label">Paste this into your AI agent</span>
            <pre class="preview" id="message-prompt-preview">{prompt}</pre>
            <span class="text-[12px] text-tertiary">
              {mode === "new" ? "It only writes the message; you commit it." : mode === "amend" && length === "keep" ? "It shows you what goes in and waits for your OK before amending." : "It shows you the message and waits for your OK before rewriting history."}
            </span>
            <div class="flex justify-between gap-2">
              <button class="ghost" id="message-prompt-back-length" onclick={() => go("length")}><ArrowLeft size={13} />Back</button>
              <button class="primary" id="message-prompt-copy" onclick={copy}>
                {#if copied}<Check size={14} />Copied{:else}<Copy size={14} />Copy prompt{/if}
              </button>
            </div>
          {/if}
        </div>
      {/key}
    </div>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 1000; display: grid; place-items: center; background: rgb(0 0 0 / 0.35); }
  .panel {
    display: flex; flex-direction: column; gap: 14px; width: 560px; max-height: calc(100vh - 48px); overflow: auto; padding: 22px 18px 16px; border-radius: 18px;
    background: var(--popover); color: var(--popover-foreground); box-shadow: 0 0 0 1px oklch(0.5 0 0 / 0.18), 0 16px 40px rgb(0 0 0 / 0.4);
  }
  .stage { display: grid; grid-template-columns: minmax(0, 1fr); }
  .label { padding: 0 4px; font-size: 12.5px; font-weight: 600; color: var(--muted-foreground); }
  .dot { width: 6px; height: 6px; border-radius: 999px; background: oklch(0.5 0 0 / 0.25); transition: width 0.3s var(--ease), background 0.3s; }
  .dot.done { background: color-mix(in oklab, var(--primary) 55%, transparent); }
  .dot.now { width: 18px; background: var(--primary); }
  .choice {
    display: flex; align-items: center; gap: 16px; padding: 16px 18px; border-radius: 14px;
    box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.16); transition: box-shadow 0.15s, background 0.15s;
  }
  .choice:hover, .choice.on { background: color-mix(in oklab, var(--tint) 6%, transparent); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--tint) 55%, transparent); }
  .tile {
    display: grid; place-items: center; width: 48px; height: 48px; flex-shrink: 0; border-radius: 12px;
    color: var(--tint); background: color-mix(in oklab, var(--tint) 15%, transparent);
  }
  .preview {
    max-height: 300px; overflow: auto; padding: 12px 14px; border-radius: 10px; font: 11.5px/1.55 var(--font-mono, ui-monospace, monospace);
    white-space: pre-wrap; word-break: break-word; color: var(--muted-foreground); background: var(--quinary);
  }
  .ghost, .primary { display: flex; align-items: center; gap: 6px; height: 32px; padding: 0 14px; border-radius: 9px; font-size: 12.5px; font-weight: 600; }
  .ghost { color: var(--muted-foreground); }
  .ghost:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.08); }
  .primary { color: white; background: var(--primary); }
</style>
