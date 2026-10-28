<!-- Branch/tag pill tinted with its graph line's color; the checked-out branch is filled. The icon tells the type apart. -->
<script lang="ts">
  import { CircleCheck, Cloud, GitBranch, Tag } from "@lucide/svelte";

  let { ref, isSynced = false, color = "var(--primary)" }: { ref: string; isSynced?: boolean; color?: string } = $props();
  const isHead = $derived(ref.startsWith("HEAD -> "));
  const isTag = $derived(ref.startsWith("tag: "));
  const name = $derived(ref.replace("HEAD -> ", "").replace("tag: ", ""));
  // shortcut: assumes remotes are named origin/upstream, read `git remote` if other names show up.
  const isRemote = $derived(/^(origin|upstream)\//.test(name));
  // The cloud icon already says "remote", so drop the "origin/" prefix.
  const label = $derived(isRemote ? name.slice(name.indexOf("/") + 1) : name);
  const Icon = $derived(isTag ? Tag : isHead ? CircleCheck : isRemote ? Cloud : GitBranch);
</script>

<span class="badge" class:head={isHead} style:--lane={color} title={name} id="ref-badge-{ref}">
  <Icon size={12} strokeWidth={2.5} class="shrink-0 opacity-80" />
  <span class="truncate">{label}</span>
  {#if isSynced}
    <span title="Same commit as its copy on the server: nothing to push or pull." class="opacity-70"><Cloud size={9} /></span>
  {/if}
</span>

<style>
  .badge {
    display: inline-flex; align-items: center; gap: 4px; min-width: 0; flex-shrink: 1;
    padding: 2px 6px; border-radius: 999px; font-size: 13px; font-weight: 600; white-space: nowrap;
    /* Soft tint of the line color; text mixed toward the foreground so it stays readable. */
    color: color-mix(in oklab, var(--lane) 35%, var(--foreground));
    background: color-mix(in oklab, var(--lane) 16%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--lane) 28%, transparent);
  }
  /* Checked-out branch: filled, darkened so white text keeps its contrast on neon colors. */
  .head { color: white; background: color-mix(in oklab, var(--lane) 70%, black); box-shadow: none; font-weight: 700; flex-shrink: 0; }
</style>
