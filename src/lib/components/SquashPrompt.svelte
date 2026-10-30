<!-- Squash, done by the user's AI agent, one step at a time: pick the commits, write the final message, choose the
     optional safety bits, then copy a prompt that already answers what the agent would otherwise ask. -->
<script lang="ts">
  import { ArrowLeft, ArrowRight, Check, Copy, Layers } from "@lucide/svelte";
  import { untrack } from "svelte";
  import { fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { api, type Card } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import Example from "./Example.svelte";

  type Props = { card: Card; model: RepoModel; dismiss: () => void };
  let { card, model, dismiss }: Props = $props();

  const steps = ["commits", "message", "options", "copy"] as const;
  type Step = (typeof steps)[number];
  let step = $state<Step>("commits");
  let direction = $state(1); // slide forward or back
  const at = $derived(steps.indexOf(step));
  const go = (to: Step) => { direction = steps.indexOf(to) > at ? 1 : -1; step = to; };

  const commits = untrack(() => card.featureCommits); // newest first; taken once so a reload doesn't reset the picks
  let picked = $state(new Set(commits.map((c) => c.id)));
  let message = $state(""); // empty: the agent reads the commits and proposes one
  let copied = $state(false);

  // Full descriptions, so each commit can be judged without opening it.
  let bodies = $state<Record<string, string>>({});
  const repo = untrack(() => model.repo!);
  for (const c of commits) api.commitInfo(repo, c.id).then((info) => (bodies[c.id] = info.body.replace(/^Co-Authored-By:.*$/gim, "").trim().replace(/(?<!\n)\n(?!\n)/g, " ")));

  const ago = (seconds: number) => {
    const minutes = Math.floor((Date.now() / 1000 - seconds) / 60);
    if (minutes < 60) return `${minutes}m ago`;
    if (minutes < 1440) return `${Math.floor(minutes / 60)}h ago`;
    const days = Math.floor(minutes / 1440);
    return days < 30 ? `${days}d ago` : days < 365 ? `${Math.floor(days / 30)}mo ago` : `${Math.floor(days / 365)}y ago`;
  };

  function setAll(on: boolean) {
    picked = new Set(on ? commits.map((c) => c.id) : []);
  }

  function toggle(id: string) {
    const next = new Set(picked);
    if (!next.delete(id)) next.add(id);
    picked = next;
  }

  // What the app already knows, so the agent doesn't have to ask.
  const isPushed = untrack(() => card.summary?.hasUpstream === true);
  const isCurrent = untrack(() => model.isCurrent(card.ref));
  const currentName = untrack(() => model.dashboard?.current ?? null);
  const dirtyFiles = untrack(() => {
    const d = model.dashboard;
    const current = d ? [d.base, ...d.environments, ...d.pinned, ...d.features, ...d.old].find((c) => c && model.isCurrent(c.ref)) : null;
    return current?.uncommitted?.files ?? 0;
  });
  const tip = untrack(() => card.last?.id ?? null);

  // Optional choices (step 3).
  let push = $state(false); // rewriting pushed history is the user's call: off unless asked
  let backup = $state(true);
  let stash = $state(true); // only matters when squashing the checked-out branch with uncommitted changes

  const prompt = $derived([
    `Repo: ${model.repo}`,
    `Branch: ${card.local}`,
    `Squash into one commit (oldest → newest): ${commits.filter((c) => picked.has(c.id)).reverse().map((c) => c.id.slice(0, 7)).join(" ")}`,
    card.squashBase ? `Base: ${card.squashBase.slice(0, 7)} (where the branch starts; leave it and everything below untouched)` : null,
    tip ? `Expected tip: ${tip.slice(0, 7)}. Abort if ${card.local} no longer points there (someone pushed meanwhile).` : null,
    message.trim()
      ? `Message, use exactly this (no extra body, no trailers):\n${message.trim()}`
      : "Message: read the picked commits (messages and diff) and propose a short one in the same style as their subjects, no trailers. Show it to me and wait for my OK before squashing.",
    !isCurrent
      ? `Checked out now: ${currentName ?? "another branch"}${dirtyFiles ? ` with ${dirtyFiles} uncommitted files` : ""}. Don't touch it: squash in a temporary git worktree and remove it after.`
      : dirtyFiles
        ? stash ? `It's checked out with ${dirtyFiles} uncommitted files: stash them first (include untracked) and restore them after.`
          : `It's checked out with ${dirtyFiles} uncommitted files: don't stash or touch them; stop and tell me if they get in the way.`
        : null,
    backup ? `Before squashing, create a backup branch backup/${card.local} at the current tip.` : null,
    isPushed ? (push ? "It's already on origin: after squashing, push with --force-with-lease." : "It's already on origin, so pushing will need --force-with-lease. Don't push; I'll do it myself.") : "Don't push.",
  ].filter((line) => line != null).join("\n"));

  async function copy() {
    await navigator.clipboard.writeText(prompt);
    copied = true;
    setTimeout(dismiss, 1100);
  }
</script>

{#snippet toggleRow(id: string, on: boolean, set: (value: boolean) => void, title: string, detail: string, example: string)}
  <button class="option" {id} role="switch" aria-checked={on} onclick={() => set(!on)}>
    <span class="flex min-w-0 flex-1 flex-col text-left">
      <span class="text-[12.5px] font-semibold">{title}</span>
      <span class="text-[11.5px] leading-snug text-muted-foreground">{detail}</span>
      <Example text={example} />
    </span>
    <span class="switch" class:on><span></span></span>
  </button>
{/snippet}

<div class="flex w-[620px] flex-col gap-3 p-1.5" id="squash-prompt">
  <div class="flex items-center gap-2">
    <Layers size={15} class="shrink-0 text-primary" />
    <span class="min-w-0 truncate text-[13px] font-semibold" title={card.local}>Squash · {card.local}</span>
    <span class="ml-auto flex shrink-0 items-center gap-1">
      {#each steps as s, i (s)}<span class="dot" class:done={i < at} class:now={i === at}></span>{/each}
    </span>
  </div>

  <div class="stage">
    {#key step}
      <div class="flex flex-col gap-2.5" in:fly={{ x: 28 * direction, duration: 260, easing: cubicOut }}>
        {#if step === "commits"}
          <span class="label">Which commits become one?</span>
          {@const all = picked.size === commits.length}
          <div class="max-h-[340px] overflow-auto rounded-lg bg-quinary">
            <button class="tr head" id="squash-pick-all" onclick={() => setAll(!all)}>
              <span class="box" class:on={picked.size > 0}>{#if all}<Check size={10} strokeWidth={3} />{:else if picked.size}<span class="dash"></span>{/if}</span>
              <span>{all ? "Deselect all" : "Select all"}</span>
            </button>
            {#each commits as commit (commit.id)}
              {@const on = picked.has(commit.id)}
              <button class="tr" class:off={!on} id="squash-pick-{commit.id}" onclick={() => toggle(commit.id)}>
                <span class="box" class:on>{#if on}<Check size={10} strokeWidth={3} />{/if}</span>
                <span class="flex min-w-0 flex-col gap-1">
                  <span class="text-[12.5px] font-semibold">{commit.subject}</span>
                  {#if bodies[commit.id]}<span class="body">{bodies[commit.id]}</span>{/if}
                  <span class="meta"><span class="font-mono">{commit.id.slice(0, 7)}</span> · {commit.author} · <span title={new Date(commit.date * 1000).toLocaleString()}>{ago(commit.date)}</span></span>
                </span>
              </button>
            {/each}
          </div>
          <span class="text-[11px] text-tertiary">{picked.size} of {commits.length} picked · pick at least 2</span>
          <div class="flex justify-end gap-2">
            <button class="ghost" id="squash-cancel" onclick={dismiss}>Cancel</button>
            <button class="primary" id="squash-next-message" disabled={picked.size < 2} onclick={() => go("message")}>Next<ArrowRight size={13} /></button>
          </div>
        {:else if step === "message"}
          <label for="squash-message" class="label">Message of the one commit that stays <span class="font-normal text-tertiary">· optional</span></label>
          <!-- svelte-ignore a11y_autofocus -->
          <input id="squash-message" bind:value={message} autofocus class="field"
            placeholder="Leave empty and your agent will propose one" onkeydown={(e) => { if (e.key === "Enter") go("options"); }} />
          <span class="text-[11px] text-tertiary">{message.trim() ? "Used exactly as written." : "The agent reads the commits, suggests a short message and waits for your OK."}</span>
          <div class="flex justify-between gap-2">
            <button class="ghost" id="squash-back-commits" onclick={() => go("commits")}><ArrowLeft size={13} />Back</button>
            <button class="primary" id="squash-next-options" onclick={() => go("options")}>Next<ArrowRight size={13} /></button>
          </div>
        {:else if step === "options"}
          <span class="label">Optional</span>
          <div class="flex flex-col gap-1">
            {@render toggleRow("squash-option-backup", backup, (v) => (backup = v), "Keep a backup branch",
              `Saves the current state as backup/${card.local}, so you can go back if you don't like the result.`,
              "the agent squashed one commit too many: the backup branch still has all of them.")}
            {#if isPushed}
              {@render toggleRow("squash-option-push", push, (v) => (push = v), "Push when done",
                "It's already on origin, so it needs a force push (with lease: refuses if someone else pushed). Updates the open PR, if any.",
                "your PR is open and you want reviewers to see the single clean commit right away.")}
            {/if}
            {#if isCurrent && dirtyFiles}
              {@render toggleRow("squash-option-stash", stash, (v) => (stash = v), `Put my ${dirtyFiles} uncommitted files aside`,
                "Stashes them before squashing and brings them back after. Off: the agent leaves them alone and stops if they block it.",
                "you have half-written code you don't want in the squash: it's set aside and back when it's done.")}
            {/if}
          </div>
          {#if !isCurrent}
            <span class="note">You're on {currentName ?? "another branch"}{dirtyFiles ? ` with ${dirtyFiles} uncommitted files` : ""}: the agent works in a temporary copy, so nothing of yours gets touched.</span>
          {/if}
          <div class="flex justify-between gap-2">
            <button class="ghost" id="squash-back-message" onclick={() => go("message")}><ArrowLeft size={13} />Back</button>
            <button class="primary" id="squash-next-copy" onclick={() => go("copy")}>Review<ArrowRight size={13} /></button>
          </div>
        {:else}
          <span class="label">Paste this into your AI agent</span>
          <pre class="preview" id="squash-preview">{prompt}</pre>
          <div class="flex justify-between gap-2">
            <button class="ghost" id="squash-back-options" onclick={() => go("options")}><ArrowLeft size={13} />Back</button>
            <button class="primary" id="squash-copy" onclick={copy}>
              {#if copied}<span class="pop flex items-center gap-1.5"><Check size={13} />Copied</span>{:else}<Copy size={13} />Copy prompt{/if}
            </button>
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
  .stage { display: grid; grid-template-columns: minmax(0, 1fr); } /* minmax: long subjects truncate instead of widening the panel */
  .tr { display: grid; grid-template-columns: 14px minmax(0, 1fr); align-items: start; column-gap: 12px; width: 100%; padding: 10px 12px; font-size: 12px; line-height: 1.45; text-align: left; transition: opacity 0.15s, background 0.15s; }
  .tr + .tr { box-shadow: inset 0 1px oklch(0.5 0 0 / 0.12); }
  .tr .box { margin-top: 2px; }
  .tr.off { opacity: 0.5; }
  .tr:hover { background: oklch(0.5 0 0 / 0.1); }
  .tr.head { position: sticky; top: 0; z-index: 1; padding-block: 6px; font-size: 10.5px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.03em; color: var(--tertiary); background: var(--quinary); }
  .dash { width: 7px; height: 2px; border-radius: 1px; background: white; }
  .body { font-size: 12px; line-height: 1.5; color: color-mix(in oklab, var(--foreground) 75%, transparent); white-space: pre-wrap; word-break: break-word; }
  .meta { font-size: 10.5px; color: var(--tertiary); opacity: 0.8; }
  .box { display: grid; place-items: center; width: 14px; height: 14px; flex-shrink: 0; border-radius: 4px; box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.4); color: white; transition: background 0.15s; }
  .box.on { background: var(--primary); box-shadow: none; }
  .field { height: 32px; padding: 0 10px; border-radius: 8px; font-size: 12.5px; outline: none; background: var(--quinary); }
  .field:focus { box-shadow: 0 0 0 2px color-mix(in oklab, var(--primary) 40%, transparent); }
  .option { display: flex; align-items: center; gap: 12px; padding: 8px 10px; border-radius: 10px; }
  .option:hover { background: oklch(0.5 0 0 / 0.08); }
  .switch { position: relative; width: 30px; height: 18px; flex-shrink: 0; border-radius: 999px; background: oklch(0.5 0 0 / 0.3); transition: background 0.2s; }
  .switch span { position: absolute; top: 2px; left: 2px; width: 14px; height: 14px; border-radius: 999px; background: white; transition: transform 0.2s cubic-bezier(0.34, 1.56, 0.64, 1); }
  .switch.on { background: var(--primary); }
  .switch.on span { transform: translateX(12px); }
  .note { padding: 7px 10px; border-radius: 8px; font-size: 11.5px; color: var(--muted-foreground); background: color-mix(in oklab, var(--primary) 8%, transparent); }
  .preview { margin: 0; max-height: 220px; overflow: auto; padding: 9px 11px; border-radius: 8px; font: 11px/1.55 var(--font-mono, ui-monospace, monospace); white-space: pre-wrap; word-break: break-word; color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.08); user-select: text; }
  .primary { display: inline-flex; align-items: center; gap: 6px; height: 32px; padding: 0 14px; border-radius: 9px; font-size: 12.5px; font-weight: 600; color: white; background: var(--primary); }
  .primary:disabled { opacity: 0.4; }
  .primary :global(svg) { transition: transform 0.15s; }
  .primary:hover:not(:disabled) :global(.lucide-arrow-right) { transform: translateX(2px); }
  .ghost { display: inline-flex; align-items: center; gap: 5px; height: 32px; padding: 0 10px; border-radius: 9px; font-size: 12px; color: var(--muted-foreground); }
  .ghost:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.08); }
  .pop { animation: pop 0.35s cubic-bezier(0.34, 1.56, 0.64, 1) both; }
  @keyframes pop { from { transform: scale(0.6); opacity: 0; } to { transform: scale(1); opacity: 1; } }
  @media (prefers-reduced-motion: reduce) { .pop { animation: none; } .switch span, .dot { transition: none; } }
</style>
