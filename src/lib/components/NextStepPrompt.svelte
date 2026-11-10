<!-- What to do right after an action, as a card bottom-right: push a commit that wasn't pushed, or, when a push was
     refused because the server has new commits, bring them in and push. Escape or "Not now" dismisses it; it also
     goes when another repo opens. -->
<script lang="ts">
  import { CloudDownload, CloudUpload } from "@lucide/svelte";
  import { fly } from "svelte/transition";
  import type { RepoModel } from "$lib/model.svelte";

  let { model }: { model: RepoModel } = $props();
  const next = $derived(model.suggestion?.repo === model.repo ? model.suggestion : null);

  function go() {
    const step = next;
    model.suggestion = null;
    if (step?.kind === "push") model.push(step.branch);
    else if (step) model.pullAndPush(step.branch);
  }
</script>

<svelte:window onkeydown={(e) => { if (next && e.key === "Escape") model.suggestion = null; }} />
{#if next}
  {@const pull = next.kind === "pullPush"}
  <div class="prompt" id="next-step-prompt" role="dialog" aria-labelledby="next-step-title" transition:fly={{ y: 16, duration: 220 }}>
    <span class="icon" class:pull>{#if pull}<CloudDownload size={18} />{:else}<CloudUpload size={18} />{/if}</span>
    <span class="min-w-0 flex-1">
      <span class="title" id="next-step-title">{pull ? `The server has new commits on ${next.branch}` : `Push ${next.branch} too?`}</span>
      <span class="detail">{pull ? "Bring them in (rebase, no merge commit) and push yours on top?" : "Committed on this computer only. Pushing sends it to the server."}</span>
    </span>
    <button class="later" id="next-step-later" onclick={() => (model.suggestion = null)}>Not now</button>
    <button class="go" id="next-step-go" disabled={model.isSyncing} onclick={go}>{pull ? "Pull & push" : "Push"}</button>
  </div>
{/if}

<style>
  .prompt { position: fixed; right: 16px; bottom: 16px; z-index: 1000; display: flex; align-items: center; gap: 10px; width: min(460px, calc(100vw - 32px));
    padding: 12px 12px 12px 14px; border-radius: 14px; background: var(--popover); box-shadow: 0 0 0 1px oklch(0.5 0 0 / 0.18), 0 12px 32px rgb(0 0 0 / 0.35); }
  .icon { display: grid; flex-shrink: 0; place-items: center; width: 34px; height: 34px; border-radius: 10px; color: var(--primary); background: color-mix(in oklab, var(--primary) 14%, transparent); }
  .icon.pull { color: #f5a524; background: color-mix(in oklab, #f5a524 14%, transparent); }
  .title { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; font-weight: 700; }
  .detail { display: block; font-size: 12px; color: var(--muted-foreground); }
  button { flex-shrink: 0; height: 30px; padding: 0 12px; border-radius: 8px; font-size: 12.5px; font-weight: 600; }
  .later { color: var(--muted-foreground); }
  .later:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.12); }
  .go { color: white; background: var(--primary); }
  .go:disabled { opacity: 0.5; }
</style>
