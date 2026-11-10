<!-- In place of the commit form while a merge, rebase or cherry-pick is stopped on conflicts: what's going on, the
     files still conflicted (click one: the diff pane shows its blocks to pick from), and Continue or Abort. -->
<script lang="ts">
  import { CircleCheck, GitMerge, TriangleAlert, Undo2 } from "@lucide/svelte";
  import type { Conflict } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import { plural } from "$lib/format";

  let { model, conflict }: { model: RepoModel; conflict: Conflict } = $props();
  const WHAT = { merge: "Merging", rebase: "Rebasing onto", cherryPick: "Copying", revert: "Reverting" };
  const left = $derived(conflict.files.length);
</script>

<div class="box" id="conflict-box">
  <div class="flex items-center gap-2">
    <span class="icon" class:done={!left}>{#if left}<TriangleAlert size={16} />{:else}<CircleCheck size={16} />{/if}</span>
    <span class="min-w-0 flex-1">
      <span class="title">{WHAT[conflict.operation]} {conflict.incoming || "…"}</span>
      <span class="detail">{left ? `${plural(left, "file")} with conflicts: open each one and pick which lines stay` : "Every conflict is resolved: continue to finish"}</span>
    </span>
  </div>
  {#if left}
    <div class="files">
      {#each conflict.files as path (path)}
        <button class="file" class:on={model.fileFilter === path} id="conflict-file-{path}" onclick={() => model.showFile(path)}>
          <GitMerge size={12} class="shrink-0 text-orange-500" /><span class="truncate">{path}</span>
        </button>
      {/each}
    </div>
  {/if}
  <div class="flex items-center justify-end gap-2">
    <button class="abort" id="conflict-abort" disabled={model.isSyncing} onclick={() => model.abortOperation()}
      title="Back to where the branch was before it started: nothing of this {conflict.operation} stays"><Undo2 size={13} />Abort</button>
    <button class="continue" id="conflict-continue" disabled={!!left || model.isSyncing} onclick={() => model.continueOperation()}
      title={left ? "Resolve every file first" : "Finish it with the resolved files"}>Continue</button>
  </div>
</div>

<style>
  .box { display: flex; height: 100%; flex-direction: column; gap: 10px; padding: 12px; overflow: auto; }
  .icon { display: grid; flex-shrink: 0; place-items: center; width: 30px; height: 30px; border-radius: 9px; color: #ea580c; background: color-mix(in oklab, #ea580c 14%, transparent); }
  .icon.done { color: #34c77b; background: color-mix(in oklab, #34c77b 14%, transparent); }
  .title { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; font-weight: 700; }
  .detail { display: block; font-size: 11.5px; color: var(--muted-foreground); }
  .files { display: flex; flex-direction: column; gap: 2px; }
  .file { display: flex; align-items: center; gap: 6px; height: 26px; padding: 0 8px; border-radius: 7px; font-size: 12px; text-align: left; }
  .file:hover { background: oklch(0.5 0 0 / 0.1); }
  .file.on { background: color-mix(in oklab, var(--primary) 14%, transparent); }
  button.abort, button.continue { display: flex; align-items: center; gap: 5px; height: 30px; padding: 0 12px; border-radius: 8px; font-size: 12.5px; font-weight: 600; }
  .abort { color: var(--muted-foreground); }
  .abort:hover:not(:disabled) { color: var(--foreground); background: oklch(0.5 0 0 / 0.12); }
  .continue { color: white; background: var(--primary); }
  button:disabled { opacity: 0.45; }
</style>
