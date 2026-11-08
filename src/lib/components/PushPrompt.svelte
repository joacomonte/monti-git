<!-- After a commit that didn't push: asks whether to push it now. Push or Not now; it also goes when another repo opens. -->
<script lang="ts">
  import { CloudUpload } from "@lucide/svelte";
  import { fly } from "svelte/transition";
  import type { RepoModel } from "$lib/model.svelte";

  let { model }: { model: RepoModel } = $props();
  const ask = $derived(model.askPush?.repo === model.repo ? model.askPush : null);

  function push(branch: string) {
    model.askPush = null;
    model.push(branch);
  }
</script>

<svelte:window onkeydown={(e) => { if (ask && e.key === "Escape") model.askPush = null; }} />
{#if ask}
  <div class="prompt" id="push-prompt" role="dialog" aria-labelledby="push-prompt-title" transition:fly={{ y: 16, duration: 220 }}>
    <span class="icon"><CloudUpload size={18} /></span>
    <span class="min-w-0 flex-1">
      <span class="title" id="push-prompt-title">Push {ask.branch} too?</span>
      <span class="detail">Committed on this computer only. Pushing sends it to the server.</span>
    </span>
    <button class="later" id="push-prompt-later" onclick={() => (model.askPush = null)}>Not now</button>
    <button class="push" id="push-prompt-push" disabled={model.isSyncing} onclick={() => push(ask.branch)}>Push</button>
  </div>
{/if}

<style>
  .prompt { position: fixed; right: 16px; bottom: 16px; z-index: 1000; display: flex; align-items: center; gap: 10px; width: min(440px, calc(100vw - 32px));
    padding: 12px 12px 12px 14px; border-radius: 14px; background: var(--popover); box-shadow: 0 0 0 1px oklch(0.5 0 0 / 0.18), 0 12px 32px rgb(0 0 0 / 0.35); }
  .icon { display: grid; flex-shrink: 0; place-items: center; width: 34px; height: 34px; border-radius: 10px; color: var(--primary); background: color-mix(in oklab, var(--primary) 14%, transparent); }
  .title { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; font-weight: 700; }
  .detail { display: block; font-size: 12px; color: var(--muted-foreground); }
  button { flex-shrink: 0; height: 30px; padding: 0 12px; border-radius: 8px; font-size: 12.5px; font-weight: 600; }
  .later { color: var(--muted-foreground); }
  .later:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.12); }
  .push { color: white; background: var(--primary); }
  .push:disabled { opacity: 0.5; }
</style>
