<!-- "Safe to delete": checks with the server first (fetch, then the branch against the base and every environment),
     then offers only what applies: here and on the server, only one of them, switching to the base when it's checked out. -->
<script lang="ts">
  import { CloudOff, LoaderCircle, Trash2, TriangleAlert } from "@lucide/svelte";
  import type { Deletion } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import { localName } from "$lib/refs";
  import ActionRow from "./ActionRow.svelte";

  let { model, branch, dismiss }: { model: RepoModel; branch: string; dismiss: () => void } = $props();
  let check = $state<Deletion | null | "failed">(null);
  $effect(() => {
    let stale = false;
    model.deletion(branch).then((found) => { if (!stale) check = found ?? "failed"; }, () => { if (!stale) check = "failed"; });
    return () => (stale = true);
  });

  const base = $derived(localName(model.history.base ?? "main"));
  const ok = $derived(check && check !== "failed" && !check.target && !check.missing.length && !check.dirty ? check : null);
  const first = $derived(ok?.current ? `Switches to ${base}, then deletes it` : "Deletes it");

  function remove(remote: boolean) {
    model.deleteBranch(branch, remote);
    dismiss();
  }
</script>

<div class="flex w-[400px] flex-col gap-1">
  <span class="px-1.5 pt-1 text-[13px] font-semibold">Delete {branch}?</span>
  {#if check === null}
    <span class="flex items-center gap-2 px-1.5 py-2 text-[12px] text-muted-foreground" id="branch-delete-checking">
      <LoaderCircle size={13} class="animate-spin" />Checking with the server that its work is in {base} and every environment…
    </span>
  {:else if check === "failed" || check.target || check.missing.length || check.dirty}
    <span class="flex items-start gap-1.5 px-1.5 py-1 text-[12px] text-orange-500" id="branch-delete-refused">
      <TriangleAlert size={13} class="mt-px shrink-0" />
      {#if check === "failed"}Couldn't check it, so it stays.
      {:else if check.target}It's {base} or an environment: it isn't deleted from here.
      {:else if check.missing.length}Not safe anymore: its work isn't in {check.missing.join(", ")} yet. Deleting it would lose it.
      {:else}It's checked out with uncommitted changes. Commit or discard them first.{/if}
    </span>
  {:else if ok}
    <span class="px-1.5 pb-1 text-[12px] text-muted-foreground">
      Its work is already in {base} and every environment: nothing gets lost.{#if !ok.fetched} Couldn't reach the server, so this is as of the last fetch.{/if}
    </span>
    {#if ok.local && ok.remote}
      <ActionRow title="Delete here and on the server" detail="{first} on your machine and on origin, for the whole team." icon={CloudOff} color="#f5a524"
        id="branch-delete-everywhere" onclick={() => remove(true)} />
    {/if}
    {#if ok.local}
      <ActionRow title={ok.remote ? "Only on my machine" : "Delete branch"} detail="{first} on your machine.{ok.remote ? ' The server copy stays.' : ''}" icon={Trash2} color="#f5a524"
        id="branch-delete-local" onclick={() => remove(false)} />
    {:else if ok.remote}
      <ActionRow title="Delete on the server" detail="It's only on origin: removes it for the whole team." icon={CloudOff} color="#f5a524"
        id="branch-delete-remote" onclick={() => remove(true)} />
    {:else}
      <span class="px-1.5 py-1 text-[12px] text-muted-foreground" id="branch-delete-gone">It's already gone, here and on the server.</span>
    {/if}
  {/if}
</div>
