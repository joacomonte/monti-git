<!-- Auto-update from GitHub Releases. At launch the app waits behind a splash until the check answers: a new version
     downloads and installs there (animated, with progress) and the app restarts already updated. One found while the
     app is open (checked every 30 min and on window focus) only shows a toast, so nothing restarts under the user's hands:
     it installs on the next launch, or right away from the toast's Restart button. -->
<script lang="ts">
  import { Sparkles, X } from "@lucide/svelte";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import type { Snippet } from "svelte";

  let { children }: { children: Snippet } = $props();
  let phase = $state<"checking" | "installing" | "ready">(import.meta.env.DEV ? "ready" : "checking"); // dev never updates
  let version = $state("");
  let progress = $state<number | null>(null); // 0…1 once the download reports its size
  let available = $state.raw<Update | null>(null); // found while open

  // Offline, slow network or no release yet: start the app anyway and try again later.
  const find = () => check({ timeout: 4000 }).catch((e) => { console.warn("update check failed:", e); return null; });

  /** Downloads and installs behind the splash (with progress), then restarts into the new version. */
  async function install(update: Update) {
    version = update.version;
    phase = "installing";
    let total = 0;
    let done = 0;
    try {
      await update.downloadAndInstall((e) => {
        if (e.event === "Started") total = e.data.contentLength ?? 0;
        else if (e.event === "Progress" && total) progress = Math.min(1, (done += e.data.chunkLength) / total);
      });
      await relaunch();
    } catch (e) {
      console.warn("update install failed:", e);
      phase = "ready";
    }
  }

  $effect(() => {
    if (import.meta.env.DEV) return;
    find().then((update) => {
      if (update) install(update);
      else phase = "ready";
    });
    // While open: every 30 min and whenever the window comes back to the front (at most once a minute).
    let last = Date.now();
    const recheck = async () => {
      if (Date.now() - last < 60 * 1000) return;
      last = Date.now();
      const update = await find();
      if (update) available = update;
    };
    const timer = setInterval(recheck, 30 * 60 * 1000);
    window.addEventListener("focus", recheck);
    return () => { clearInterval(timer); window.removeEventListener("focus", recheck); };
  });
</script>

{#if phase === "ready"}
  {@render children()}
{:else}
  <div data-tauri-drag-region class="flex h-full flex-col items-center justify-center gap-5 bg-background" id="update-splash">
    <!-- The app icon's graph; while installing, the branch keeps drawing itself and merging back. -->
    <svg viewBox="100 100 824 824" class="size-24 drop-shadow-lg" class:installing={phase === "installing"}>
      <defs>
        <linearGradient id="splash-bg" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#6d5dfc" /><stop offset="1" stop-color="#1fb6a6" />
        </linearGradient>
      </defs>
      <rect x="100" y="100" width="824" height="824" rx="185" fill="url(#splash-bg)" />
      <g fill="none" stroke="#fff" stroke-width="44" stroke-linecap="round">
        <path d="M390 300 V724" />
        <path class="branch" pathLength="1" d="M390 380 C390 470 634 440 634 530 V560 C634 650 390 620 390 700" stroke-opacity="0.85" />
      </g>
      <g fill="#fff">
        <circle cx="390" cy="300" r="46" /><circle class="tip" cx="634" cy="545" r="46" /><circle cx="390" cy="724" r="46" />
      </g>
      <circle cx="390" cy="300" r="22" fill="#6d5dfc" />
    </svg>
    {#if phase === "installing"}
      <div class="flex flex-col items-center gap-2.5">
        <span class="text-[13px] font-semibold">Updating to {version}…</span>
        <div class="h-1 w-[180px] overflow-hidden rounded-full bg-quinary">
          {#if progress == null}
            <div class="indeterminate h-full w-1/3 rounded-full bg-primary"></div>
          {:else}
            <div class="h-full rounded-full bg-primary transition-[width]" style:width="{progress * 100}%"></div>
          {/if}
        </div>
        <span class="text-[11px] text-tertiary">The app restarts by itself when it's done.</span>
      </div>
    {/if}
  </div>
{/if}

{#if available && phase === "ready"}
  <div class="enter fixed right-4 bottom-4 z-50 flex items-center gap-2.5 rounded-xl border bg-popover px-3.5 py-2.5 text-[12px] shadow-lg" id="update-toast">
    <Sparkles size={14} class="shrink-0 text-primary" />
    <span>Monti Git {available.version} is out.</span>
    <button class="restart" id="update-toast-restart" onclick={() => available && install(available)}>Restart to update</button>
    <button class="grid size-5 place-items-center text-muted-foreground" title="Dismiss" id="update-toast-dismiss" onclick={() => (available = null)}>
      <X size={11} strokeWidth={3} />
    </button>
  </div>
{/if}

<style>
  .restart { height: 26px; padding: 0 11px; border-radius: 8px; font-size: 12px; font-weight: 600; color: white; background: var(--primary); }
  .restart:hover { background: color-mix(in oklab, var(--primary) 85%, black); }
  .installing .branch { stroke-dasharray: 1; animation: draw 1.6s ease-in-out infinite; }
  .installing .tip { transform-box: fill-box; transform-origin: center; animation: pulse 1.6s ease-in-out infinite; }
  .indeterminate { animation: slide 1.2s ease-in-out infinite; }
  @keyframes draw { 0% { stroke-dashoffset: 1; } 60%, 100% { stroke-dashoffset: 0; } }
  @keyframes pulse { 0%, 30% { transform: scale(0); } 45% { transform: scale(1.25); } 60%, 100% { transform: scale(1); } }
  @keyframes slide { from { transform: translateX(-100%); } to { transform: translateX(300%); } }
</style>
