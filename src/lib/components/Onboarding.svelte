<!-- First run: add repos (same two ways as the sidebar's +), then, with more than one, a step that suggests starring favorites. -->
<script lang="ts">
  import { ArrowRight, FolderGit2, FolderTree, Star } from "@lucide/svelte";
  import type { Component } from "svelte";
  import { api, type RepoEntry } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";

  let { model }: { model: RepoModel } = $props();
  let added = $state<RepoEntry[]>([]); // non-empty = favorites step

  async function add(kind: "repo" | "folder") {
    model.onboarding = true; // stay mounted once hasSources flips, so the favorites step can show
    await model.addRepos(kind);
    const { folders, repos, hidden } = model.sources;
    const list = model.hasSources ? (await api.listRepos(folders, repos)).filter((r) => !hidden.includes(r.path)) : [];
    if (list.length > 1) added = list.sort((a, b) => b.lastActivity - a.lastActivity);
    else finish(list);
  }

  function finish(list = added) {
    model.onboarding = false;
    model.setShowGraph(false);
    if (!model.repo && list.length) model.open((list.find((r) => model.favorites.includes(r.path)) ?? list[0]).path);
  }

  const nameOf = (path: string) => path.split(/[\\/]/).pop();
  const starred = $derived(added.filter((r) => model.favorites.includes(r.path)).length);
</script>

{#snippet option(kind: "repo" | "folder", Icon: Component, color: string, title: string, detail: string, delay: number)}
  <button class="option enter flex flex-1 flex-col items-start gap-3 rounded-[16px] p-5 text-left" id="onboarding-add-{kind}" title={detail}
    style:animation-delay="{delay}ms" style:--tint={color} onclick={() => add(kind)}>
    <span class="tile grid size-10 place-items-center rounded-[12px]"><Icon size={19} strokeWidth={2.2} /></span>
    <span class="flex flex-col gap-1">
      <span class="text-[14px] font-semibold">{title}</span>
      <span class="text-[12px] leading-[1.45] text-muted-foreground">{detail}</span>
    </span>
  </button>
{/snippet}

<div class="relative flex h-full flex-col items-center justify-center gap-8 overflow-hidden px-8" id="onboarding">
  <div class="glow pointer-events-none absolute" class:gold={added.length}></div>
  {#if !added.length}
    <div class="enter relative flex flex-col items-center gap-4 text-center">
      <img src="/app-icon.png" alt="" class="icon size-[84px]" />
      <div class="flex flex-col gap-1.5">
        <h1 class="text-[26px] font-bold tracking-tight">Welcome to Monti Git</h1>
        <span class="max-w-[420px] text-[13px] leading-[1.5] text-muted-foreground">
          Add the repositories you work on. You can add more anytime with the + at the top of the sidebar.
        </span>
      </div>
    </div>
    <div class="relative flex w-full max-w-[560px] gap-3">
      {@render option("repo", FolderGit2, "#8b7bff", "Add repository", "Pick one or more git repos, one by one.", 80)}
      {@render option("folder", FolderTree, "#2fc4b2", "Add folder of repos", "Pick a folder with several repos inside: all of them are listed, and new ones show up by themselves.", 140)}
    </div>
    {#if model.syncStatus}<span class="relative max-w-[560px] text-center text-[12px] text-orange-500" id="onboarding-status">{model.syncStatus}</span>{/if}
  {:else}
    <div class="enter relative flex flex-col items-center gap-3 text-center">
      <span class="badge grid size-14 place-items-center rounded-[18px]"><Star size={26} fill="currentColor" /></span>
      <div class="flex flex-col gap-1.5">
        <h1 class="text-[24px] font-bold tracking-tight">Star your favorites</h1>
        <span class="max-w-[440px] text-[13px] leading-[1.5] text-muted-foreground">
          {added.length} repos added. Favorites stay at the top of the sidebar, so the ones you open every day are always one click away.
        </span>
      </div>
    </div>
    <div class="relative grid max-h-[46vh] w-full max-w-[600px] grid-cols-2 gap-2 overflow-auto p-1" id="onboarding-favorites">
      {#each added as repo, i (repo.path)}
        {@const on = model.favorites.includes(repo.path)}
        <button class="repo enter flex items-center gap-3 rounded-[12px] px-3.5 py-2.5 text-left" class:on id="onboarding-favorite-{nameOf(repo.path)}"
          style:animation-delay="{60 + Math.min(i, 12) * 35}ms" onclick={() => model.toggleFavorite(repo.path)}>
          <span class="flex min-w-0 flex-1 flex-col">
            <span class="truncate text-[13px] font-semibold">{nameOf(repo.path)}</span>
            <span class="truncate text-[11px] text-muted-foreground">{repo.owner}</span>
          </span>
          {#key on}<span class="star shrink-0" class:pop={on}><Star size={16} fill={on ? "currentColor" : "none"} /></span>{/key}
        </button>
      {/each}
    </div>
    <button class="cta enter relative flex h-10 items-center gap-2 rounded-full px-5 text-[13px] font-semibold" id="onboarding-favorites-done"
      style:animation-delay="200ms" class:skip={!starred} onclick={() => finish()}>
      {starred ? `Continue with ${starred} favorite${starred === 1 ? "" : "s"}` : "Skip for now"}
      <ArrowRight size={15} />
    </button>
  {/if}
</div>

<style>
  .glow {
    top: 50%; left: 50%; width: 640px; height: 480px; transform: translate(-50%, -62%);
    background: radial-gradient(closest-side, rgb(109 93 252 / 0.22), rgb(31 182 166 / 0.08) 60%, transparent);
    transition: background 0.6s ease;
  }
  .glow.gold { background: radial-gradient(closest-side, rgb(245 165 36 / 0.18), rgb(109 93 252 / 0.08) 60%, transparent); }
  .icon { filter: drop-shadow(0 12px 28px rgb(109 93 252 / 0.45)); animation: float 4s ease-in-out infinite; }
  .option {
    background: color-mix(in oklab, var(--quinary) 70%, transparent);
    box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.12);
  }
  .option:hover { background: color-mix(in oklab, var(--tint) 10%, transparent); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--tint) 40%, transparent); }
  .tile { color: var(--tint); background: color-mix(in oklab, var(--tint) 16%, transparent); }
  .badge { color: #f5a524; background: color-mix(in oklab, #f5a524 16%, transparent); filter: drop-shadow(0 10px 24px rgb(245 165 36 / 0.35)); animation: float 4s ease-in-out infinite; }
  .repo {
    background: color-mix(in oklab, var(--quinary) 70%, transparent);
    box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.12);
  }
  .repo:hover { background: color-mix(in oklab, #f5a524 7%, transparent); }
  .repo.on { background: color-mix(in oklab, #f5a524 13%, transparent); box-shadow: inset 0 0 0 1px color-mix(in oklab, #f5a524 45%, transparent); }
  .star { color: var(--muted-foreground); display: grid; }
  .repo:hover .star, .repo.on .star { color: #f5a524; }
  .cta { color: white; background: var(--primary); box-shadow: 0 8px 22px color-mix(in oklab, var(--primary) 35%, transparent); }
  .cta.skip { color: var(--foreground); background: color-mix(in oklab, var(--quinary) 80%, transparent); box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.15); }
  @media (prefers-reduced-motion: no-preference) {
    .pop { animation: pop 0.35s cubic-bezier(0.3, 1.6, 0.5, 1); }
  }
  @keyframes pop { 40% { transform: scale(1.4) rotate(-12deg); } }
  @keyframes float { 50% { transform: translateY(-4px); } }
</style>
