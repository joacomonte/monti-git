<!-- A conflicted file in the diff pane: the lines both sides agree on, and each conflict block with yours and theirs
     side by side to pick from (or both). Saving writes the picks and marks the file resolved; "Use mine / theirs"
     takes one side for the whole file. -->
<script lang="ts">
  import { LoaderCircle, TriangleAlert } from "@lucide/svelte";
  import { api, type ConflictPart, type Pick } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";

  let { model, path }: { model: RepoModel; path: string } = $props();
  let parts = $state.raw<ConflictPart[] | null>(null);
  let picks = $state<(Pick | null)[]>([]);
  let error = $state<string | null>(null);

  $effect(() => {
    const repo = model.repo;
    void model.history; // reread after any action (another tool may have touched the file)
    if (!repo) return;
    let stale = false;
    api.conflictParts(repo, path).then(
      (found) => { if (stale) return; parts = found; picks = found.filter((p) => p.kind === "conflict").map(() => null); error = null; },
      (e) => { if (!stale) error = String(e); },
    );
    return () => (stale = true);
  });

  const blocks = $derived(parts?.filter((p) => p.kind === "conflict").length ?? 0);
  const chosen = $derived(picks.filter((p) => p != null).length);
  // Each conflict part's block number, so the picks line up with them.
  const blockOf = $derived.by(() => {
    let n = 0;
    return (parts ?? []).map((p) => (p.kind === "conflict" ? n++ : -1));
  });
  const lines = (text: string) => text.replace(/\n$/, "").split("\n");
</script>

<div class="flex min-h-0 flex-1 flex-col">
  <div class="bar">
    <span class="text-[12px] text-muted-foreground" id="conflict-progress">{blocks ? `${chosen} of ${blocks} conflicts picked` : "No conflict markers left in the file"}</span>
    <span class="flex-1"></span>
    <button class="side" id="conflict-use-mine" disabled={model.isSyncing} onclick={() => model.takeSide(path, true)} title="Your version of the whole file">Use mine</button>
    <button class="side" id="conflict-use-theirs" disabled={model.isSyncing} onclick={() => model.takeSide(path, false)} title="Their version of the whole file">Use theirs</button>
    <button class="save" id="conflict-save" disabled={!blocks || chosen < blocks || model.isSyncing}
      onclick={() => model.resolveConflict(path, picks.filter((p): p is Pick => p != null))}>Save & mark resolved</button>
  </div>
  {#if error}
    <div class="flex items-start gap-2 p-3 text-[12px] text-orange-500"><TriangleAlert size={14} class="mt-px shrink-0" />{error}</div>
  {:else if !parts}
    <div class="flex items-center gap-2 p-3 text-[12px]"><LoaderCircle size={14} class="animate-spin" />Reading the conflicts…</div>
  {:else}
    <div class="code min-h-0 flex-1 overflow-auto" id="conflict-view">
      {#each parts as part, i (i)}
        {#if part.kind === "same"}
          <pre class="same">{part.text.replace(/\n$/, "")}</pre>
        {:else}
          {@const b = blockOf[i]}
          <div class="block" id="conflict-block-{b}">
            {#each [["mine", "Mine", part.mine], ["theirs", "Theirs", part.theirs]] as [side, label, text] (side)}
              <button class="choice {side}" class:on={picks[b] === side || picks[b] === "both"} id="conflict-block-{b}-{side}"
                onclick={() => (picks[b] = picks[b] === side ? null : (side as Pick))}>
                <span class="label">{label}{picks[b] === side ? " ✓" : ""}</span>
                <pre>{#each lines(text) as line, k (k)}{line}{"\n"}{/each}</pre>
              </button>
            {/each}
            <button class="both" class:on={picks[b] === "both"} id="conflict-block-{b}-both" onclick={() => (picks[b] = picks[b] === "both" ? null : "both")}
              title="Keep both: mine first, then theirs">Both</button>
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .bar { display: flex; align-items: center; gap: 6px; padding: 6px 10px; border-bottom: 1px solid var(--border); }
  .side, .save { height: 26px; padding: 0 10px; border-radius: 7px; font-size: 12px; font-weight: 600; }
  .side { color: var(--muted-foreground); box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.2); }
  .side:hover:not(:disabled) { color: var(--foreground); background: oklch(0.5 0 0 / 0.1); }
  .save { color: white; background: var(--primary); }
  button:disabled { opacity: 0.45; }
  .code { padding: 8px 10px; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; line-height: 1.5; }
  pre { margin: 0; white-space: pre-wrap; word-break: break-all; font: inherit; }
  .same { color: var(--muted-foreground); }
  .block { display: grid; grid-template-columns: 1fr 1fr auto; gap: 6px; margin: 6px 0; }
  .choice { display: flex; flex-direction: column; gap: 2px; padding: 6px 8px; border-radius: 8px; text-align: left; box-shadow: inset 0 0 0 1px var(--c); background: color-mix(in oklab, var(--c) 7%, transparent); }
  .choice.mine { --c: color-mix(in oklab, #4f8cff 55%, transparent); }
  .choice.theirs { --c: color-mix(in oklab, #a78bfa 55%, transparent); }
  .choice.on { background: color-mix(in oklab, var(--c) 30%, transparent); box-shadow: inset 0 0 0 2px var(--c); }
  .label { font-family: system-ui, sans-serif; font-size: 10.5px; font-weight: 700; letter-spacing: 0.04em; text-transform: uppercase; color: var(--muted-foreground); }
  .both { align-self: center; height: 26px; padding: 0 9px; border-radius: 7px; font-family: system-ui, sans-serif; font-size: 11.5px; font-weight: 600; color: var(--muted-foreground); box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.2); }
  .both.on { color: var(--foreground); background: oklch(0.5 0 0 / 0.16); }
</style>
