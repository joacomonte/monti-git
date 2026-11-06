<!-- Deleting untracked files: git has no copy, so it can't be undone. Always a dialog with what goes; a repo inside this
     one says what only it has (commits on no remote, its own changes). -->
<script lang="ts">
  import { FolderGit2, FileX2, LoaderCircle, Trash2, TriangleAlert } from "@lucide/svelte";
  import { api, type Doomed } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import { plural } from "$lib/format";

  let { model, paths, onclose }: { model: RepoModel; paths: string[]; onclose: () => void } = $props();
  let found = $state<Doomed[] | null>(null);
  let failed = $state("");
  $effect(() => {
    let stale = false;
    api.untrackedCheck(model.repo!, paths).then((d) => { if (!stale) found = d; }, (e) => { if (!stale) failed = String(e); });
    return () => (stale = true);
  });
  const repos = $derived(found?.filter((d) => d.nested) ?? []);
  const ready = $derived(!!found);

  /** What only that repo has, in words; empty when it's all safe on a remote. */
  function risk(n: NonNullable<Doomed["nested"]>) {
    return [
      !n.hasRemote && "no remote: its whole history exists only here",
      n.hasRemote && n.unpushed && `${plural(n.unpushed, "commit")} on no remote`,
      n.uncommitted && `${plural(n.uncommitted, "uncommitted file")}`,
    ].filter(Boolean).join(" · ");
  }

  function confirm() {
    model.deleteUntracked(paths);
    onclose();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="overlay" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="delete-untracked-title" id="delete-untracked">
    <h2 id="delete-untracked-title"><Trash2 size={18} />Delete {paths.length === 1 ? paths[0] : plural(paths.length, "untracked file")}?</h2>
    <p class="lead">Untracked: git has no copy of {paths.length === 1 ? "it" : "them"}. They're deleted from disk and this can't be undone.</p>

    {#if failed}
      <p class="warn"><TriangleAlert size={14} />Couldn't check them, so nothing is deleted. {failed}</p>
    {:else if !found}
      <p class="lead flex items-center gap-2"><LoaderCircle size={14} class="animate-spin" />Checking what's inside…</p>
    {:else}
      <ul class="list">
        {#each found as d (d.path)}
          <li>
            {#if d.nested}<FolderGit2 size={14} class="shrink-0 text-orange-500" />{:else}<FileX2 size={14} class="shrink-0 text-tertiary" />{/if}
            <span class="min-w-0 flex-1 truncate font-mono" title={d.path}>{d.path}</span>
            {#if d.nested}<span class="tag">git repo</span>{/if}
          </li>
          {#if d.nested && risk(d.nested)}<li class="risk"><TriangleAlert size={12} class="shrink-0" />{risk(d.nested)}</li>{/if}
        {/each}
      </ul>
      {#if repos.length}
        <p class="warn"><TriangleAlert size={14} class="mt-px shrink-0" />
          {repos.length === 1 ? `${repos[0].path} is a whole git repo` : `${repos.length} of them are whole git repos`}, with {repos.length === 1 ? "its" : "their"} own history. Whatever isn't on a remote is lost.</p>
      {/if}
    {/if}

    <div class="buttons">
      <button class="cancel" id="delete-untracked-cancel" onclick={onclose}>Cancel</button>
      <button class="danger" id="delete-untracked-confirm" disabled={!ready} onclick={confirm}><Trash2 size={14} />Delete from disk</button>
    </div>
  </div>
</div>

<style>
  .overlay { position: fixed; inset: 0; z-index: 60; display: grid; place-items: center; padding: 16px; background: rgb(0 0 0 / 0.45); }
  .dialog { display: flex; width: min(480px, 100%); max-height: 80vh; flex-direction: column; gap: 12px; padding: 18px; border-radius: 14px;
    background: var(--popover); box-shadow: 0 0 0 1px oklch(0.5 0 0 / 0.2), 0 20px 50px rgb(0 0 0 / 0.4); }
  h2 { display: flex; align-items: center; gap: 8px; font-size: 15px; font-weight: 700; word-break: break-all; }
  .lead { font-size: 12.5px; line-height: 1.45; color: var(--muted-foreground); }
  .list { display: flex; flex-direction: column; gap: 2px; overflow-y: auto; padding: 6px; border-radius: 8px; background: oklch(0.5 0 0 / 0.08); }
  .list li { display: flex; align-items: center; gap: 8px; min-height: 24px; padding: 0 4px; font-size: 12px; }
  .list li.risk { padding-left: 26px; font-size: 11.5px; color: #f97316; }
  .tag { flex-shrink: 0; padding: 1px 6px; border-radius: 5px; font-size: 10.5px; font-weight: 600; color: #f97316; background: rgb(249 115 22 / 0.14); }
  .warn { display: flex; align-items: flex-start; gap: 8px; font-size: 12.5px; line-height: 1.45; color: #f97316; }
  .buttons { display: flex; justify-content: flex-end; gap: 8px; padding-top: 4px; }
  .buttons button { display: flex; align-items: center; gap: 6px; height: 32px; padding: 0 14px; border-radius: 8px; font-size: 13px; font-weight: 600; }
  .cancel { color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.12); }
  .cancel:hover { color: var(--foreground); }
  .danger { color: white; background: #dc2626; }
  .danger:hover:not(:disabled) { background: #b91c1c; }
  .danger:disabled { opacity: 0.4; }
</style>
