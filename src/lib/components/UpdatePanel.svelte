<!-- "Update from main" (UpdatePanel in Dashboard.swift): pick merge (recommended) or rebase, run it, then show the
     result. Conflicts abort cleanly and list the files; a rebase of a pushed branch offers the force push. -->
<script lang="ts">
  import { ArrowUpDown, CircleCheck, EyeOff, GitMerge, LoaderCircle, TriangleAlert } from "@lucide/svelte";
  import type { Card } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import { Button } from "$lib/components/ui/button";
  import ActionRow from "./ActionRow.svelte";

  type Props = { card: Card; model: RepoModel; isCurrent: boolean; dismiss: () => void };
  let { card, model, isCurrent, dismiss }: Props = $props();
  let step = $state<"choose" | "working" | "done" | "pushed">("choose");
  let error = $state<string | null>(null);
  let usedRebase = $state(false);

  const localName = (ref: string) => (/^(origin|upstream)\//.test(ref) ? ref.slice(ref.indexOf("/") + 1) : ref);
  const parentName = $derived(card.parent ? localName(card.parent) : "its base");
  const isPushed = $derived(card.summary?.hasUpstream === true);
  const n = $derived(card.behindParent);

  async function run(rebase: boolean) {
    usedRebase = rebase;
    error = null;
    step = "working";
    try {
      await model.updateFromParent(card, rebase);
      step = "done";
    } catch (e) {
      error = String(e);
      step = "choose";
    }
  }

  async function push() {
    error = null;
    try {
      await model.forcePush(card.local);
      step = "pushed";
    } catch (e) {
      error = String(e);
    }
  }
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

<div class="flex w-[420px] flex-col gap-3 p-1.5">
  {#if step === "choose"}
    <div class="flex flex-col gap-1 px-2">
      <span class="text-[13px] font-semibold">Bring in {parentName}'s {n} new commit{n === 1 ? "" : "s"}</span>
      <span class="text-[12px] leading-snug text-muted-foreground">
        Your teammates added {n} change{n === 1 ? "" : "s"} to {parentName} after you created this branch.
        Bring them in so you work with the latest code and your PR doesn't hit surprises. Your own work isn't lost.
      </span>
    </div>
    <div class="flex flex-col gap-1">
      <ActionRow title="Merge (recommended)" detail="The easy, safe option. Copies {parentName}'s new changes into your branch with one extra commit. Nothing of yours is rewritten." icon={GitMerge} color="#34c77b"
        example="{parentName} got 3 new commits: they come in with one extra “Merge” commit, and your commits stay exactly as they were."
        id="update-option-merge" onclick={() => run(false)} />
      <ActionRow title="Rebase" detail={`As if you had created the branch today: your commits move on top of ${parentName}'s. Cleaner history, but your commits get rewritten.` + (isPushed ? " Since you already pushed, you'll need to force push after." : "")}
        icon={ArrowUpDown} color="#8b7bff" id="update-option-rebase" onclick={() => run(true)}
        example="your 4 commits get re-applied after {parentName}'s 3 new ones, as if you had branched off this morning. No merge commit." />
      {#if !model.isUpdateDismissed(card)}
        <ActionRow title="Hide this notice" detail="Not now. The number turns grey, and goes back to orange when {parentName} gets new changes." icon={EyeOff} grey
          id="update-option-hide" onclick={() => { model.dismissUpdate(card); dismiss(); }}
          example="you're in the middle of something and will update right before opening the PR." />
      {/if}
    </div>
    <span class="px-2 text-[10.5px] text-tertiary">
      If you and a teammate changed the same lines, it stops and puts everything back as it was: nothing breaks.{#if !isCurrent} You don't need to switch branches for this.{/if}
    </span>
  {:else if step === "working"}
    <div class="flex items-center gap-2.5 text-[12px]">
      <LoaderCircle size={14} class="animate-spin" />{usedRebase ? `Rebasing onto ${parentName}…` : `Merging ${parentName}…`}
    </div>
  {:else if step === "done"}
    {@render title(`Up to date with ${parentName}`,
      usedRebase ? `Your commits now sit on top of ${parentName}.` : `${parentName}'s changes are merged into this branch.`)}
    {#if usedRebase && isPushed}
      <span class="text-[11px] text-muted-foreground">The remote still has the old commits. Force push (with lease) replaces them, and refuses if someone else pushed meanwhile.</span>
    {/if}
    <div class="flex justify-end gap-2">
      <Button variant="secondary" size="sm" onclick={dismiss} id="update-close">{usedRebase && isPushed ? "Not now" : "Done"}</Button>
      {#if usedRebase && isPushed}<Button size="sm" onclick={push} id="update-force-push">Force push</Button>{/if}
    </div>
  {:else}
    {@render title("Pushed", "The remote now has the rebased branch.")}
    <div class="flex justify-end"><Button size="sm" onclick={dismiss} id="update-done">Done</Button></div>
  {/if}

  {#if error}
    <div class="flex flex-col gap-1.5 rounded-lg bg-orange-500/10 p-2.5">
      <span class="flex items-center gap-1.5 text-[12px] font-semibold text-orange-500">
        <TriangleAlert size={13} />{step === "done" ? "Force push failed. Nothing was overwritten." : "Couldn't update. Nothing changed: it was undone."}
      </span>
      <span class="font-mono text-[11px] whitespace-pre-wrap text-muted-foreground select-text">{error}</span>
      {#if step !== "done"}
        <span class="text-[11px] text-tertiary">Conflicts mean both branches changed the same lines. Fix them by hand from a terminal (git merge {parentName}), or ask someone to pair on it.</span>
      {/if}
    </div>
  {/if}
</div>
