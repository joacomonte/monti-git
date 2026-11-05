<!-- Top-right toast for the git action running (model.act): what it is, what it does, elapsed time and a progress bar.
     It can be folded into a small pill (never closed) and unfolded again; it goes away by itself once finished. -->
<script lang="ts">
  import { Check, ChevronDown, ChevronUp, LoaderCircle, X } from "@lucide/svelte";
  import { fade } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import type { RepoModel } from "$lib/model.svelte";
  import { prefs } from "$lib/prefs";

  let { model }: { model: RepoModel } = $props();
  let folded = $state(prefs.get("actionToastFolded", false));
  let now = $state(Date.now());

  $effect(() => {
    if (model.toast?.state !== "busy") return;
    const timer = setInterval(() => (now = Date.now()), 200);
    return () => clearInterval(timer);
  });

  /** Grows out of (and shrinks back into) the Logs button in the header. */
  function logs(node: HTMLElement, { duration }: { duration: number }) {
    const from = document.getElementById("logs-open")?.getBoundingClientRect();
    if (!from) return { duration: 0 };
    const box = node.getBoundingClientRect();
    const dx = from.left + from.width / 2 - (box.left + box.width / 2);
    const dy = from.top + from.height / 2 - (box.top + box.height / 2);
    return {
      duration, easing: cubicOut,
      css: (t: number, u: number) => `transform: translate(${dx * u}px, ${dy * u}px) scale(${0.08 + 0.92 * t}); opacity: ${Math.min(1, t * 2)};`,
    };
  }
  /** The Logs button gives a small bump when the toast lands in it. */
  const pulseLogs = () => document.getElementById("logs-open")?.animate(
    [{ transform: "scale(1)" }, { transform: "scale(1.15)", color: "var(--primary)" }, { transform: "scale(1)" }], { duration: 380, easing: "ease-out" });

  function fold(on: boolean) {
    folded = on;
    prefs.set("actionToastFolded", on);
  }

  const t = $derived(model.toast);
  const seconds = $derived(t ? Math.max(0, Math.floor((now - t.started) / 1000)) : 0);
  // git doesn't report progress for these: the bar is an estimate that eases toward 90% and fills when it finishes.
  const progress = $derived(!t ? 0 : t.state !== "busy" ? 100 : 90 * (1 - Math.exp(-(now - t.started) / 4000)));
  // The step it's probably on, spread over the same estimate; all done once finished.
  const step = $derived(!t ? 0 : t.state !== "busy" ? t.details.length : Math.min(t.details.length - 1, Math.floor((progress / 90) * t.details.length)));
</script>

{#if t}
  <!-- Comes out of the Logs button and goes back into it when done (where its commands are). -->
  <div class="dock" in:logs={{ duration: 420 }} out:logs={{ duration: 360 }} onoutroend={pulseLogs}>
  {#key folded}
    <div class="toast" class:folded class:failed={t.state === "failed"} id="action-toast" role="status" aria-live="polite" in:fade={{ duration: 160 }}>
      {#if folded}
        <button class="pill" id="action-toast-unfold" title="Show details" onclick={() => fold(false)}>
          {@render icon()}
          <span class="truncate">{t.state === "busy" ? t.title : t.result}</span>
          <ChevronDown size={13} class="shrink-0 opacity-60" />
        </button>
      {:else}
        <div class="flex items-center gap-2.5">
          {@render icon()}
          <span class="min-w-0 flex-1 truncate text-[13px] font-semibold">{t.title}</span>
          <span class="text-[11.5px] text-tertiary tabular-nums">{seconds}s</span>
          <button class="fold" id="action-toast-fold" title="Hide (it keeps going)" onclick={() => fold(true)}><ChevronUp size={14} /></button>
        </div>
        {#if t.details.length}
          <ol class="steps">
            {#each t.details as detail, i (detail)}
              <li class:done={i < step} class:now={i === step && t.state === "busy"} style:animation-delay="{i * 70}ms">
                <span class="mark">{#if i < step}<Check size={10} strokeWidth={3} />{/if}</span>{detail}
              </li>
            {/each}
          </ol>
        {/if}
        {#if t.state !== "busy"}<span class="result" id="action-toast-result">{t.result}</span>{/if}
      {/if}
      <span class="bar"><span style:width="{progress}%"></span></span>
    </div>
  {/key}
  </div>
{/if}

{#snippet icon()}
  {#if t?.state === "busy"}<LoaderCircle size={15} class="spin shrink-0 text-primary" />
  {:else if t?.state === "done"}<Check size={15} strokeWidth={2.5} class="shrink-0 text-green-500" />
  {:else}<X size={15} strokeWidth={2.5} class="shrink-0 text-red-500" />{/if}
{/snippet}

<style>
  .dock { position: fixed; top: 60px; right: 14px; z-index: 1100; } /* under the header, so the Logs button stays reachable */
  .toast {
    position: relative; display: flex; flex-direction: column; gap: 8px;
    width: 380px; max-width: calc(100vw - 24px); padding: 12px 14px 14px; overflow: hidden; border-radius: 14px;
    background: var(--popover); color: var(--popover-foreground); box-shadow: 0 0 0 1px oklch(0.5 0 0 / 0.18), 0 12px 32px rgb(0 0 0 / 0.35);
  }
  .toast.folded { width: auto; max-width: 320px; padding: 0; border-radius: 999px; }
  .pill { display: flex; align-items: center; gap: 7px; min-width: 0; height: 30px; padding: 0 12px; font-size: 12px; font-weight: 600; }
  .fold { display: grid; place-items: center; width: 22px; height: 22px; border-radius: 6px; color: var(--tertiary); }
  .fold:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.12); }
  .steps { display: flex; flex-direction: column; gap: 4px; padding-left: 2px; font-size: 12px; color: var(--tertiary); }
  .steps li { display: flex; align-items: center; gap: 8px; opacity: 0; animation: step-in 0.3s cubic-bezier(0.22, 1, 0.36, 1) forwards; transition: color 0.3s; }
  .steps li.done { color: var(--muted-foreground); }
  .steps li.now { color: var(--foreground); }
  .mark { display: grid; place-items: center; width: 14px; height: 14px; flex-shrink: 0; border-radius: 999px; color: white; box-shadow: inset 0 0 0 1.5px oklch(0.5 0 0 / 0.35); transition: background 0.3s; }
  .done .mark { background: var(--green, #34c77b); box-shadow: none; }
  .now .mark { box-shadow: inset 0 0 0 1.5px var(--primary); animation: pulse 1.2s ease-in-out infinite; }
  .result { font-size: 12px; color: var(--muted-foreground); word-break: break-word; }
  .failed .result { color: color-mix(in oklab, #ef4444 70%, var(--foreground)); }
  .bar { position: absolute; inset: auto 0 0 0; height: 2px; background: oklch(0.5 0 0 / 0.12); }
  .bar span { display: block; height: 100%; background: var(--primary); transition: width 0.3s linear; }
  .failed .bar span { background: #ef4444; }
  :global(.spin) { animation: spin 0.9s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @keyframes step-in { from { opacity: 0; transform: translateY(-4px); } to { opacity: 1; transform: none; } }
  @keyframes pulse { 50% { box-shadow: inset 0 0 0 3px color-mix(in oklab, var(--primary) 40%, transparent); } }
  @media (prefers-reduced-motion: reduce) { .steps li, .now .mark, :global(.spin) { animation: none; opacity: 1; } }
</style>
