<!-- Above the file tree on a selected commit: title, description and every detail, each one hideable from the ⚙ menu. -->
<script lang="ts">
  import { Check, Copy, Settings2 } from "@lucide/svelte";
  import { api, type CommitInfo } from "$lib/api";
  import * as Popover from "$lib/components/ui/popover";
  import { prefs, type RepoModel } from "$lib/model.svelte";

  let { model, id }: { model: RepoModel; id: string } = $props();
  let info = $state<CommitInfo | null>(null);
  let branches = $state<string[] | null>(null);
  let expanded = $state(false); // long description fully shown
  let copied = $state(false);
  let open = $state(prefs.get("commitDetailsOpen", false)); // only the title until "see more…"; remembered

  function setOpen(value: boolean) {
    open = value;
    prefs.set("commitDetailsOpen", value);
  }

  const FIELDS = [
    { key: "body", label: "Description" },
    { key: "hash", label: "Hash" },
    { key: "author", label: "Author" },
    { key: "date", label: "Date" },
    { key: "committer", label: "Committer (when not the author)" },
    { key: "parents", label: "Parents" },
    { key: "refs", label: "Branches and tags pointing here" },
    { key: "branches", label: "Branches that contain it" },
    { key: "stats", label: "Files and lines changed" },
  ] as const;
  type Field = (typeof FIELDS)[number]["key"];
  let hidden = $state<Field[]>(prefs.get("commitDetailsHidden", []));
  const shows = (field: Field) => !hidden.includes(field);

  function toggle(field: Field) {
    hidden = shows(field) ? [...hidden, field] : hidden.filter((f) => f !== field);
    prefs.set("commitDetailsHidden", hidden);
  }

  $effect(() => {
    const repo = model.repo;
    if (!repo) return;
    info = null;
    branches = null;
    expanded = false;
    api.commitInfo(repo, id).then((result) => { if (result.id === id) info = result; });
    api.commitPlaces(repo, id).then((names) => { if (model.repo === repo) branches = names; });
  });

  const totals = $derived(Object.values(model.counts).reduce(([a, r], [x, y]) => [a + x, r + y], [0, 0]));
  const files = $derived(Object.keys(model.changes).length);

  async function copyHash() {
    await navigator.clipboard.writeText(id);
    copied = true;
    setTimeout(() => (copied = false), 1200);
  }

  const date = (seconds: number) => new Date(seconds * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
  function ago(seconds: number) {
    const minutes = Math.floor((Date.now() / 1000 - seconds) / 60);
    if (minutes < 60) return `${minutes}m ago`;
    if (minutes < 1440) return `${Math.floor(minutes / 60)}h ago`;
    const days = Math.floor(minutes / 1440);
    return days < 30 ? `${days}d ago` : days < 365 ? `${Math.floor(days / 30)}mo ago` : `${Math.floor(days / 365)}y ago`;
  }
</script>

<div class="details enter-fade" id="commit-details">
  {#if info}
    <div class="flex items-start gap-2">
      <span class="min-w-0 flex-1 text-[13px] leading-snug font-semibold select-text">{info.subject}</span>
      {#if open}<Popover.Root>
        <Popover.Trigger class="gear" id="commit-details-settings" title="Choose which details to show"><Settings2 size={13} /></Popover.Trigger>
        <Popover.Content align="end" class="w-[250px] p-1.5">
          <span class="block px-2 pt-1 pb-1 text-[11px] font-semibold text-muted-foreground uppercase">Show</span>
          {#each FIELDS as field (field.key)}
            <button class="pick" id="commit-details-field-{field.key}" onclick={() => toggle(field.key)}>
              <span class="box" class:on={shows(field.key)}>{#if shows(field.key)}<Check size={10} strokeWidth={3} />{/if}</span>{field.label}
            </button>
          {/each}
        </Popover.Content>
      </Popover.Root>{/if}
    </div>
    {#if !open}
      <button class="see" id="commit-details-see-more" onclick={() => setOpen(true)}>see more…</button>
    {:else}
    {#if shows("body") && info.body}
      <p class="body select-text" class:clamp={!expanded}>{info.body}</p>
      {#if info.body.split("\n").length > 4 || info.body.length > 280}
        <button class="more" id="commit-details-more" onclick={() => (expanded = !expanded)}>{expanded ? "Show less" : "Show more"}</button>
      {/if}
    {/if}
    <div class="meta">
      {#if shows("hash")}
        <span class="k">Hash</span>
        <span class="flex items-center gap-1.5"><span class="font-mono select-text" title={info.id}>{info.id.slice(0, 10)}</span>
          <button class="icon" id="commit-details-copy-hash" title="Copy full hash" onclick={copyHash}>
            {#if copied}<Check size={11} class="text-green-500" />{:else}<Copy size={11} />{/if}
          </button></span>
      {/if}
      {#if shows("author")}<span class="k">Author</span><span class="truncate select-text" title={info.authorEmail}>{info.author} <span class="dim">{info.authorEmail}</span></span>{/if}
      {#if shows("date")}<span class="k">Date</span><span>{date(info.authorDate)} <span class="dim">· {ago(info.authorDate)}</span></span>{/if}
      {#if shows("committer") && (info.committer !== info.author || Math.abs(info.committerDate - info.authorDate) > 60)}
        <span class="k">Committed</span><span class="truncate" title={info.committerEmail}>{info.committer} <span class="dim">· {date(info.committerDate)}</span></span>
      {/if}
      {#if shows("parents") && info.parents.length}
        <span class="k">{info.parents.length > 1 ? "Parents" : "Parent"}</span>
        <span class="flex flex-wrap gap-1.5">{#each info.parents as parent (parent)}
          <button class="link font-mono" id="commit-details-parent-{parent}" title="Go to {parent}" onclick={() => model.revealCommit(model.repo!, parent)}>{parent.slice(0, 7)}</button>
        {/each}</span>
      {/if}
      {#if shows("refs") && info.refs.length}
        <span class="k">Points here</span>
        <span class="flex flex-wrap gap-1">{#each info.refs as ref (ref)}<span class="chip">{ref.replace("HEAD -> ", "")}</span>{/each}</span>
      {/if}
      {#if shows("branches")}
        <span class="k">In</span>
        <span class="flex flex-wrap gap-1">
          {#if branches == null}<span class="dim">…</span>
          {:else if !branches.length}<span class="dim">no branch (only reachable from a tag or the reflog)</span>
          {:else}
            {#each branches.slice(0, 6) as name (name)}<span class="chip">{name}</span>{/each}
            {#if branches.length > 6}<span class="dim" title={branches.slice(6).join("\n")}>+{branches.length - 6} more</span>{/if}
          {/if}
        </span>
      {/if}
      {#if shows("stats") && files}
        <span class="k">Changes</span>
        <span>{files} {files === 1 ? "file" : "files"} <span class="text-green-500/80">+{totals[0]}</span> <span class="text-red-500/80">−{totals[1]}</span></span>
      {/if}
    </div>
    <button class="see" id="commit-details-see-less" onclick={() => setOpen(false)}>see less</button>
    {/if}
  {:else}
    <span class="dim text-[12px]">Loading commit…</span>
  {/if}
</div>

<style>
  .details { display: flex; flex-direction: column; gap: 6px; max-height: 45%; overflow: auto; padding: 10px 12px; border-bottom: 1px solid var(--border); flex-shrink: 0; }
  .body { font-size: 12px; line-height: 1.45; white-space: pre-wrap; color: var(--muted-foreground); word-break: break-word; }
  .body.clamp { display: -webkit-box; -webkit-line-clamp: 4; line-clamp: 4; -webkit-box-orient: vertical; overflow: hidden; }
  .see { align-self: flex-start; margin-top: -2px; font-size: 11px; color: var(--tertiary); opacity: 0.7; }
  .see:hover { opacity: 1; color: var(--muted-foreground); }
  .more { align-self: flex-start; font-size: 11px; font-weight: 600; color: var(--primary); }
  .meta { display: grid; grid-template-columns: auto 1fr; gap: 3px 10px; font-size: 11.5px; align-items: center; min-width: 0; }
  .k { color: var(--tertiary); }
  .dim { color: var(--tertiary); }
  .chip { max-width: 200px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding: 0 6px; border-radius: 99px; font-size: 11px; background: oklch(0.5 0 0 / 0.12); }
  .link { color: var(--primary); }
  .link:hover { text-decoration: underline; }
  .icon { display: grid; place-items: center; width: 18px; height: 18px; border-radius: 4px; color: var(--tertiary); }
  .icon:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.12); }
  :global(.gear) { display: grid; place-items: center; width: 22px; height: 22px; flex-shrink: 0; border-radius: 6px; color: var(--tertiary); }
  :global(.gear:hover), :global(.gear[data-state="open"]) { color: var(--foreground); background: oklch(0.5 0 0 / 0.12); }
  .pick { display: flex; width: 100%; align-items: center; gap: 8px; height: 28px; padding: 0 8px; border-radius: 6px; font-size: 12px; text-align: left; }
  .pick:hover { background: oklch(0.5 0 0 / 0.1); }
  .box { display: grid; place-items: center; width: 14px; height: 14px; border-radius: 4px; box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.4); color: white; }
  .box.on { background: var(--primary); box-shadow: none; }
</style>
