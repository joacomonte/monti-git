<!-- Branch/tag pill tinted with its graph line's color; the checked-out branch is filled. Next to a branch, plain-word chips
     say where it is: a cloud (on the server at this commit) or Local committed (not on the server at this commit). -->
<script lang="ts">
  import { CircleCheck, Cloud, Laptop, Tag } from "@lucide/svelte";
  import { isHead as headRef, isRemote as remoteRef, isTag as tagRef, localName, refName } from "$lib/refs";

  // onclick: the pill becomes a button (with hover feedback) that opens the branch's options.
  type Props = { ref: string; isSynced?: boolean; color?: string; onclick?: (e: MouseEvent) => void; oncontextmenu?: (e: MouseEvent) => void };
  let { ref, isSynced = false, color = "var(--primary)", onclick, oncontextmenu }: Props = $props();
  // Clicking the pill again closes its menu: keep the window's "mousedown closes menus" from closing it first.
  const stop = (e: MouseEvent) => e.stopPropagation();
  const isHead = $derived(headRef(ref));
  const isTag = $derived(tagRef(ref));
  const name = $derived(refName(ref).replace("tag: ", ""));
  const isRemote = $derived(remoteRef(name));
  // The cloud already says "on the server", so drop the "origin/" prefix.
  const label = $derived(localName(name));
  const remoteName = $derived(isRemote ? name.slice(0, name.indexOf("/")) : "origin");
  // No icon for a plain branch (it's what every pill is): a check for the checked-out one, a tag for tags.
  const Icon = $derived(isTag ? Tag : isHead ? CircleCheck : null);
</script>

<span class="group">
  <svelte:element this={onclick ? "button" : "span"} class="badge" class:head={isHead} class:clickable={onclick} style:--lane={color} id="ref-badge-{name.replace(/[^\w/.-]/g, '-')}" role={onclick ? "button" : undefined} {onclick} {oncontextmenu} onmousedown={onclick ? stop : undefined}>
    {#if Icon}<Icon size={12} strokeWidth={2.5} class="shrink-0 opacity-80" />{/if}
    <span class="truncate">{label}</span>
    <!-- On the server at this commit: a small cloud inside the pill (the collapsed card draws its own). -->
    {#if !isTag && (isRemote || isSynced)}
      <span class="cloud" title={isSynced ? `On ${remoteName} (the server) too, at this same commit: nothing to push or pull.` : `On ${remoteName} (the server) at this commit.`}><Cloud size={12} strokeWidth={2.5} /></span>
    {/if}
  </svelte:element>
  {#if !isTag && !isRemote && !isSynced}
    <span class="where local" title="Committed on this computer, not pushed yet (or the server has a different version)."><Laptop size={11} strokeWidth={2.5} />Local committed</span>
  {/if}
</span>

<style>
  /* Shrinks by truncating the branch name; the chips keep their size, so nothing spills over the date. */
  .group { display: inline-flex; align-items: center; gap: 4px; min-width: 0; flex-shrink: 1; overflow: hidden; }
  .where {
    display: inline-flex; flex-shrink: 0; align-items: center; gap: 4px; height: 22px; padding: 0 8px; border-radius: 999px;
    font-size: 11.5px; font-weight: 700; color: var(--tint); background: color-mix(in oklab, var(--tint) 14%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--tint) 35%, transparent);
  }
  .cloud { display: flex; flex-shrink: 0; margin-left: 2px; opacity: 0.6; }
  .local { --tint: #f5a524; } /* amber: work that only exists here */
  .badge {
    display: inline-flex; align-items: center; gap: 4px; min-width: 0; flex-shrink: 1;
    padding: 2px 6px; border-radius: 999px; font-size: 13px; font-weight: 600; white-space: nowrap;
    /* Soft tint of the line color; text mixed toward the foreground so it stays readable. */
    color: color-mix(in oklab, var(--lane) 25%, var(--foreground));
    background: color-mix(in oklab, var(--lane) 9%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--lane) 18%, transparent);
  }
  .clickable { cursor: pointer; transition: background 0.15s, box-shadow 0.15s, transform 0.1s; }
  .clickable:hover { background: color-mix(in oklab, var(--lane) 28%, transparent); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--lane) 55%, transparent); }
  .clickable:active { transform: scale(0.97); }
  .head.clickable:hover { background: color-mix(in oklab, var(--lane) 82%, black); }
  /* Checked-out branch: filled, darkened so white text keeps its contrast on neon colors. */
  .head { color: white; background: color-mix(in oklab, var(--lane) 70%, black); box-shadow: none; font-weight: 700; }
</style>
