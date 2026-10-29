<!-- "What's new" after an update: every change between the version the user last saw and this one, from
     src/lib/changelog.json. A fresh install shows nothing; it only remembers the version. -->
<script lang="ts">
  import { Sparkles, TrendingUp, Wrench } from "@lucide/svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import type { Component } from "svelte";
  import changelog from "$lib/changelog.json";
  import { prefs } from "$lib/model.svelte";

  type Kind = "new" | "improved" | "fixed";
  type Release = { version: string; changes: { kind: Kind; title: string; detail: string }[] };

  const KINDS: Record<Kind, { icon: Component; label: string; color: string }> = {
    new: { icon: Sparkles, label: "New", color: "#8b7bff" },
    improved: { icon: TrendingUp, label: "Improved", color: "#2fc4b2" },
    fixed: { icon: Wrench, label: "Fixed", color: "#f5a524" },
  };

  let current = $state("");
  let from = $state<string | null>(null);
  let releases = $state<Release[]>([]);

  /** "0.1.10" > "0.1.9": compares number by number. */
  const newer = (a: string, b: string) => {
    const [x, y] = [a, b].map((v) => v.split(".").map(Number));
    for (let i = 0; i < Math.max(x.length, y.length); i++) if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) > (y[i] ?? 0);
    return false;
  };

  getVersion().then((version) => {
    current = version;
    const seen = prefs.get<string | null>("lastSeenVersion", null);
    prefs.set("lastSeenVersion", version);
    // Nothing saved: a fresh install shows nothing; someone updating from 0.1.5 (before this screen existed) has other
    // prefs already, so they get this version's notes.
    if (seen == null && prefs.get("lastRepo", null) == null) return;
    if (seen != null && !newer(version, seen)) return;
    from = seen;
    releases = (changelog as Release[]).filter((r) => !newer(r.version, version) && (seen == null ? r.version === version : newer(r.version, seen)));
  });

  const close = () => (releases = []);
  // Position of each release's first change across the whole list, for the staggered entrance.
  const starts = $derived(releases.map((_, i) => releases.slice(0, i).reduce((n, r) => n + r.changes.length, 0)));
</script>

<svelte:window onkeydown={(e) => { if (e.key === "Escape" && releases.length) close(); }} />

{#if releases.length}
  <div class="backdrop fixed inset-0 z-50 grid place-items-center bg-black/45 p-6 backdrop-blur-[6px]" id="whats-new"
    role="presentation" onclick={(e) => { if (e.target === e.currentTarget) close(); }}>
    <div class="panel flex max-h-[min(640px,90vh)] w-[480px] flex-col overflow-hidden rounded-[22px] border bg-popover shadow-2xl"
      role="dialog" aria-modal="true" aria-labelledby="whats-new-title">
      <!-- Header: the app icon on a soft glow of its own gradient. -->
      <div class="hero relative flex flex-col items-center gap-3 px-8 pt-9 pb-6 text-center">
        <img src="/app-icon.png" alt="" class="icon relative size-[72px]" />
        <div class="relative flex flex-col gap-1">
          <h2 id="whats-new-title" class="text-[22px] font-bold tracking-tight">What's new</h2>
          <span class="text-[12.5px] text-muted-foreground">
            {from ? `You're on ${current} · updated from ${from}` : `Monti Git ${current}`}
          </span>
        </div>
      </div>

      <div class="flex flex-col gap-5 overflow-auto px-6 pb-2">
        {#each releases as release, r (release.version)}
          <section class="flex flex-col gap-2">
            {#if releases.length > 1}
              <span class="px-1 text-[10.5px] font-semibold tracking-[0.8px] text-tertiary uppercase">Version {release.version}</span>
            {/if}
            {#each release.changes as change, c (change.title)}
              {@const kind = KINDS[change.kind]}
              <div class="item flex items-start gap-3 rounded-[14px] p-3" style:--tint={kind.color} style:animation-delay="{120 + (starts[r] + c) * 45}ms">
                <span class="tile grid size-8 shrink-0 place-items-center rounded-[10px]"><kind.icon size={15} strokeWidth={2.4} /></span>
                <div class="flex min-w-0 flex-col gap-0.5">
                  <div class="flex items-center gap-2">
                    <span class="text-[13.5px] font-semibold">{change.title}</span>
                    <span class="tag rounded-full px-1.5 text-[9.5px] font-bold tracking-[0.5px] uppercase">{kind.label}</span>
                  </div>
                  <span class="text-[12.5px] leading-[1.45] text-muted-foreground">{change.detail}</span>
                </div>
              </div>
            {/each}
          </section>
        {/each}
      </div>

      <div class="flex justify-end px-6 pt-3 pb-5">
        <button class="cta h-9 rounded-[10px] px-5 text-[13px] font-semibold text-white" id="whats-new-close" title="Close: shows again only after the next update" onclick={close}>Got it</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop { animation: fade 0.25s ease-out both; }
  .panel { animation: rise 0.45s cubic-bezier(0.2, 0.9, 0.25, 1.15) both; }
  .hero::before {
    content: ""; position: absolute; inset: -40px 0 auto; height: 220px; pointer-events: none;
    background: radial-gradient(closest-side, rgb(109 93 252 / 0.35), rgb(31 182 166 / 0.12) 60%, transparent);
  }
  .icon { filter: drop-shadow(0 10px 24px rgb(109 93 252 / 0.45)); animation: float 4s ease-in-out infinite; }
  .item {
    background: color-mix(in oklab, var(--tint) 7%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--tint) 16%, transparent);
    animation: rise 0.4s ease-out both;
  }
  .tile { color: var(--tint); background: color-mix(in oklab, var(--tint) 18%, transparent); }
  .tag { color: var(--tint); background: color-mix(in oklab, var(--tint) 15%, transparent); }
  .cta { background: linear-gradient(135deg, #6d5dfc, #1fb6a6); box-shadow: 0 6px 18px rgb(109 93 252 / 0.35); transition: filter 0.15s; }
  .cta:hover { filter: brightness(1.1); }
  @keyframes fade { from { opacity: 0; } }
  @keyframes rise { from { opacity: 0; transform: translateY(10px) scale(0.98); } }
  @keyframes float { 50% { transform: translateY(-4px); } }
</style>
