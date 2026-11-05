<!-- Destructive action with the app's two-step confirm (DiscardButton.swift): first click arms it, ✕ cancels, ✓ confirms. -->
<script lang="ts">
  import { Check, Undo2, X } from "@lucide/svelte";

  type Props = { title: string; hint: string; id: string; label?: boolean; onconfirm: () => void };
  let { title, hint, id, label = false, onconfirm }: Props = $props();
  let isConfirming = $state(false);
  /** Spans, not buttons (they sit inside the tree's row <button>s): Enter and Space work like a click. */
  const activate = (action: () => void) => ({
    onclick: (e: MouseEvent) => { e.stopPropagation(); action(); },
    onkeydown: (e: KeyboardEvent) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); e.stopPropagation(); action(); } },
  });

  $effect(() => {
    if (!isConfirming) return;
    const timer = setTimeout(() => (isConfirming = false), 4000); // not confirmed: back to the plain icon
    return () => clearTimeout(timer);
  });
</script>

{#if isConfirming}
  <span class="flex shrink-0 items-center gap-1">
    <span role="button" tabindex="0" class="circle bg-quinary text-muted-foreground" title="Cancel" id="{id}-cancel"
      {...activate(() => (isConfirming = false))}><X size={8} strokeWidth={3.5} /></span>
    <span role="button" tabindex="0" class="circle bg-primary text-white" title="Confirm. This can't be undone." id="{id}-confirm"
      {...activate(() => { isConfirming = false; onconfirm(); })}><Check size={8} strokeWidth={3.5} /></span>
  </span>
{:else}
  <span role="button" tabindex="0" class="flex h-[18px] shrink-0 items-center gap-1 text-[10px] font-semibold text-muted-foreground hover:text-foreground {label ? 'px-[7px]' : 'px-1'}"
    title={hint} {id} {...activate(() => (isConfirming = true))}>
    <Undo2 size={10} strokeWidth={2.5} />{#if label}<span class="label">{title}</span>{/if}
  </span>
{/if}

<style>
  .circle { display: grid; place-items: center; width: 18px; height: 18px; border-radius: 999px; }
</style>
