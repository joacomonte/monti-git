<!-- Squash in three steps (SquashPanel in Dashboard.swift): edit the message (with plain-language warnings), squash,
     then offer the force push. -->
<script lang="ts">
  import { CircleArrowUp, CircleCheck, CloudUpload, Layers, TriangleAlert } from "@lucide/svelte";
  import { untrack, type Component } from "svelte";
  import type { Card } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import { Button } from "$lib/components/ui/button";

  type Props = { card: Card; model: RepoModel; dismiss: () => void };
  let { card, model, dismiss }: Props = $props();

  // Taken once when the panel opens: the card reloads after the squash, but the message and the undo hint must not change.
  const opened = untrack(() => card);
  const oldestFirst = opened.featureCommits.map((c) => c.subject).reverse();
  let message = $state(oldestFirst[0] + (oldestFirst.length > 1 ? "\n\n" + oldestFirst.slice(1).map((s) => "- " + s).join("\n") : ""));
  const oldTip = opened.last?.id ?? "";
  let step = $state<"edit" | "squashed" | "pushed">("edit");
  let error = $state<string | null>(null);
  let isWorking = $state(false);

  const localName = (ref: string) => (/^(origin|upstream)\//.test(ref) ? ref.slice(ref.indexOf("/") + 1) : ref);
  const isPushed = $derived(card.summary?.hasUpstream === true);
  const places = opened.alreadyIn.join(", ");

  async function squash() {
    isWorking = true;
    error = null;
    try {
      await model.squash(opened, message.endsWith("\n") ? message : message + "\n");
      step = "squashed";
    } catch (e) {
      error = "Squash failed: " + String(e);
    }
    isWorking = false;
  }

  async function push() {
    isWorking = true;
    error = null;
    try {
      await model.forcePush(opened.local);
      step = "pushed";
    } catch (e) {
      error = "Force push failed (nothing was overwritten): " + String(e);
    }
    isWorking = false;
  }

  /** ⌘↩ squashes; plain Return is a new line in the message. */
  function keydown(e: KeyboardEvent) {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && !isWorking && message.trim()) {
      e.preventDefault();
      squash();
    }
  }
</script>

{#snippet title(Icon: Component, text: string, detail: string)}
  <div class="flex items-start gap-2.5">
    <Icon size={20} class="shrink-0 text-primary" />
    <div class="flex flex-col gap-0.5">
      <span class="text-[13px] font-semibold">{text}</span>
      <span class="text-[11px] text-muted-foreground">{detail}</span>
    </div>
  </div>
{/snippet}

{#snippet note(Icon: Component, orange: boolean, heading: string, body: string)}
  <div class="flex flex-col gap-1 rounded-lg p-2.5 {orange ? 'bg-orange-500/10' : 'bg-primary/10'}">
    <span class="flex items-center gap-1.5 text-[12px] font-semibold {orange ? 'text-orange-500' : 'text-primary'}"><Icon size={13} />{heading}</span>
    <span class="text-[11px] whitespace-pre-line text-muted-foreground">{body}</span>
  </div>
{/snippet}

<div class="flex w-[440px] flex-col gap-3 p-1.5">
  {#if step === "edit"}
    {@render title(Layers, `Squash ${opened.featureCommits.length} commits into one`,
      `Same final files, one clean commit on top of ${opened.parent ? localName(opened.parent) : "its base"}. Uncommitted changes aren't touched.`)}
    {#if places}
      {@render note(TriangleAlert, true, `Some of these commits are already in ${places}`,
        `Squashing doesn't edit those commits: it creates a brand-new one with the same changes. ${places} keep the old commits, so git sees this branch as having new work again:
• the dashboard will show it as pending for ${places}
• the next merge there brings the same changes a second time (usually no conflicts, but duplicate history)
It's fine if those branches get rebuilt or your team is OK with that. If you're not sure, ask before squashing.`)}
    {/if}
    {#if isPushed}
      {@render note(CloudUpload, false, "This branch is already on the remote",
        "After squashing you'll be offered a force push so the remote gets the clean version too.")}
    {/if}
    <label for="squash-message" class="text-[11px] font-medium text-muted-foreground">Commit message</label>
    <textarea id="squash-message" bind:value={message} onkeydown={keydown}
      class="h-[140px] resize-none rounded-lg bg-quinary p-2 text-[12px] outline-none focus:ring-2 focus:ring-primary/40"></textarea>
    <div class="flex justify-end gap-2">
      <Button variant="secondary" size="sm" onclick={dismiss} id="squash-cancel" title="Close without changing anything">Cancel</Button>
      <Button size="sm" onclick={squash} disabled={isWorking || !message.trim()} title="Combine the commits into one with this message (⌘↩). The old ones stay recoverable in the reflog." id="squash-confirm">Squash</Button>
    </div>
  {:else if step === "squashed"}
    {@render title(CircleCheck, "Squashed into one commit", isPushed ? "The remote still has the old commits." : "It's only local: push it whenever you're ready.")}
    {#if isPushed}
      {@render note(CircleArrowUp, false, "Force push the clean version?",
        `This replaces the branch on the remote with your squashed one. "With lease" means git refuses if someone else pushed to it meanwhile, so you can't overwrite their work.`)}
    {/if}
    <div class="flex flex-col gap-0.5">
      <span class="text-[11px] text-tertiary">Changed your mind? This puts the old commits back:</span>
      <span class="font-mono text-[11px] select-text">git update-ref refs/heads/{opened.local} {oldTip.slice(0, 12)}</span>
    </div>
    <div class="flex justify-end gap-2">
      <Button variant="secondary" size="sm" onclick={dismiss} id="squash-close" title="Close: the squash stays only on your machine until you push">{isPushed ? "Not now" : "Done"}</Button>
      {#if isPushed}<Button size="sm" onclick={push} disabled={isWorking} id="squash-force-push" title="Replace the branch on the server with the squashed one (force push with lease: refuses if someone else pushed)">Force push</Button>{/if}
    </div>
  {:else}
    {@render title(CircleCheck, "Pushed", "The remote now has the single squashed commit.")}
    <div class="flex justify-end"><Button size="sm" onclick={dismiss} id="squash-done" title="Close">Done</Button></div>
  {/if}
  {#if error}<span class="font-mono text-[11px] whitespace-pre-wrap text-red-500 select-text">{error}</span>{/if}
</div>
