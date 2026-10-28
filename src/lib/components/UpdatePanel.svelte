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
    <span class="px-2 text-[13px] font-semibold text-muted-foreground">Bring in {parentName}'s {n} new commit{n === 1 ? "" : "s"}</span>
    <div class="flex flex-col gap-1">
      <ActionRow title="Merge (recommended)" detail="Adds one merge commit. Safest: no force push needed." icon={GitMerge}
        id="update-option-merge" onclick={() => run(false)} />
      <ActionRow title="Rebase" detail={"Replays your commits on top for a straight history." + (isPushed ? " Needs a force push." : "")}
        icon={ArrowUpDown} id="update-option-rebase" onclick={() => run(true)} />
      {#if !model.isUpdateDismissed(card)}
        <ActionRow title="Hide this notice" detail="Stays as a grey tag; turns blue again when {parentName} gets new commits." icon={EyeOff} grey
          id="update-option-hide" onclick={() => { model.dismissUpdate(card); dismiss(); }} />
      {/if}
    </div>
    {#if !isCurrent}<span class="px-2 text-[10px] text-tertiary">Done in a temporary copy: your current checkout isn't touched.</span>{/if}
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
