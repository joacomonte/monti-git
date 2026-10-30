<!-- Open PR, done by the user's AI agent: pick where it goes, copy a minimal prompt (title, body, ticket: the agent's job). -->
<script lang="ts">
  import { GitPullRequest } from "@lucide/svelte";
  import type { Card } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import { Button } from "$lib/components/ui/button";

  type Props = { card: Card; model: RepoModel; dismiss: () => void };
  let { card, model, dismiss }: Props = $props();

  const localName = (ref: string) => (/^(origin|upstream)\//.test(ref) ? ref.slice(ref.indexOf("/") + 1) : ref);
  // Its parent first, then the base and the environments.
  const targets = $derived([...new Set(
    [card.parent, model.dashboard?.base?.ref, ...(model.dashboard?.environments ?? []).map((c) => c.ref)]
      .filter((r): r is string => r != null).map(localName).filter((r) => r !== card.local),
  )]);
  let picked = $state<string | null>(null);
  const target = $derived(picked ?? targets[0] ?? "main");
  let copied = $state(false);

  const prompt = $derived(`Repo: ${model.repo}\nOpen a PR from ${card.local} into ${target}.`);

  async function copy() {
    await navigator.clipboard.writeText(prompt);
    copied = true;
    setTimeout(dismiss, 900);
  }
</script>

<div class="flex w-[400px] flex-col gap-2.5 p-1.5" id="pr-prompt">
  <span class="flex items-center gap-2 text-[13px] font-semibold"><GitPullRequest size={15} class="text-primary" />Open PR</span>
  <div class="flex flex-wrap items-center gap-1.5 text-[12px]">
    <span class="max-w-[200px] truncate font-medium" title={card.local}>{card.local}</span>
    <span class="text-tertiary">→</span>
    {#each targets as name (name)}
      <button class="chip" class:on={target === name} id="pr-target-{name}" onclick={() => (picked = name)}>{name}</button>
    {/each}
  </div>
  <pre class="preview" id="pr-preview">{prompt}</pre>
  <div class="flex justify-end gap-2">
    <Button variant="secondary" size="sm" onclick={dismiss} id="pr-cancel">Cancel</Button>
    <Button size="sm" onclick={copy} id="pr-copy">{copied ? "Copied ✓" : "Copy prompt for AI"}</Button>
  </div>
</div>

<style>
  .chip { height: 24px; padding: 0 10px; border-radius: 999px; font-weight: 600; color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.08); }
  .chip:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.14); }
  .chip.on { color: white; background: var(--primary); }
  .preview { margin: 0; padding: 8px 10px; border-radius: 8px; font-size: 11px; line-height: 1.5; white-space: pre-wrap; word-break: break-all; color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.08); }
</style>
