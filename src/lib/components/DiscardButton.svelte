<!-- Destructive action with the app's two-step confirm (DiscardButton.swift): first click arms it, ✕ cancels, ✓ confirms. -->
<script lang="ts">
  import { Check, Undo2, X } from "@lucide/svelte";

  type Props = { title: string; hint: string; id: string; label?: boolean; onconfirm: () => void };
  let { title, hint, id, label = false, onconfirm }: Props = $props();
  let isConfirming = $state(false);

  $effect(() => {
    if (!isConfirming) return;
    const timer = setTimeout(() => (isConfirming = false), 4000); // not confirmed: back to the plain icon
    return () => clearTimeout(timer);
  });
</script>

{#if isConfirming}
  <span class="flex shrink-0 items-center gap-1">
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <span role="button" tabindex="0" class="circle bg-quinary text-muted-foreground" title="Cancel" id="{id}-cancel"
      onclick={(e) => { e.stopPropagation(); isConfirming = false; }}><X size={8} strokeWidth={3.5} /></span>
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <span role="button" tabindex="0" class="circle bg-primary text-white" title="Confirm. This can't be undone." id="{id}-confirm"
      onclick={(e) => { e.stopPropagation(); isConfirming = false; onconfirm(); }}><Check size={8} strokeWidth={3.5} /></span>
  </span>
{:else}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <span role="button" tabindex="0" class="flex h-[18px] shrink-0 items-center gap-1 text-[10px] font-semibold text-muted-foreground hover:text-foreground {label ? 'px-[7px]' : 'px-1'}"
    title={hint} {id} onclick={(e) => { e.stopPropagation(); isConfirming = true; }}>
    <Undo2 size={10} strokeWidth={2.5} />{#if label}{title}{/if}
  </span>
{/if}

<style>
  .circle { display: grid; place-items: center; width: 18px; height: 18px; border-radius: 999px; }
</style>
