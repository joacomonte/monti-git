<!-- "Edit commits": a branch's own commits since the base, oldest on top (the order they were made). Each one is kept,
     squashed into the one above, or dropped; ↑/↓ reorder them, and clicking a message rewords it. Saving rebuilds the
     branch in a temporary copy (a conflict changes nothing). Commits already on the server: a warning, then a force push. -->
<script lang="ts">
  import { ArrowDown, ArrowUp, Combine, LoaderCircle, PenLine, Trash2, TriangleAlert } from "@lucide/svelte";
  import { api, type OwnCommit, type RewriteStep } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import { ago, plural } from "$lib/format";
  import { localName } from "$lib/refs";
  import { Button } from "$lib/components/ui/button";
  import Segmented from "./Segmented.svelte";

  type Props = { branch: string; model: RepoModel; dismiss: () => void };
  let { branch, model, dismiss }: Props = $props();
  type Line = OwnCommit & { action: RewriteStep["action"]; edited: string | null };
  let lines = $state<Line[] | null>(null);
  let error = $state<string | null>(null);
  let editing = $state<string | null>(null); // id of the message being reworded
  let saving = $state(false);
  let original: string[] = []; // the order git had: anything else is a change to save

  const base = $derived(model.history.base ? localName(model.history.base) : null);
  $effect(() => {
    const repo = model.repo, b = model.history.base;
    if (!repo || !b) return;
    api.ownCommits(repo, branch, b).then(
      (found) => {
        original = found.map((c) => c.id);
        lines = found.map((c) => ({ ...c, action: "pick", edited: null }));
      },
      (e) => (error = String(e)),
    );
  });

  const kept = $derived(lines?.filter((l) => l.action !== "drop") ?? []);
  const pushed = $derived(lines?.some((l) => l.pushed) ?? false);
  const changed = $derived(!!lines && lines.some((l, i) => l.action !== "pick" || l.edited != null || l.id !== original[i]));
  // The first commit kept has nothing above it to squash into.
  const badSquash = $derived(kept[0]?.action === "squash");
  const result = $derived(kept.filter((l) => l.action === "pick").length);
  const subjectOf = (l: Line) => (l.edited ?? l.message).split("\n")[0];

  function move(i: number, by: number) {
    if (!lines) return;
    const next = [...lines];
    [next[i], next[i + by]] = [next[i + by], next[i]];
    lines = next;
  }

  async function save() {
    if (!lines) return;
    saving = true;
    error = null;
    const steps: RewriteStep[] = lines.map((l) => ({ id: l.id, action: l.action, message: l.edited }));
    const outcome = await model.rewriteBranch(branch, steps, pushed);
    saving = false;
    if (outcome.ok) dismiss();
    else error = outcome.error;
  }
</script>

<div class="flex w-[560px] flex-col gap-3 p-1.5">
  <div class="flex flex-col gap-1 px-2">
    <span class="text-[13px] font-semibold">Edit {branch}'s commits</span>
    <span class="text-[12px] leading-snug text-muted-foreground">Its own commits since {base}, oldest on top. Reorder them, squash one into the one above, drop it, or click a message to reword it.</span>
  </div>

  {#if !lines && !error}
    <div class="flex items-center gap-2 px-2 text-[12px]"><LoaderCircle size={14} class="animate-spin" />Reading the commits…</div>
  {:else if lines}
    <ol class="list" id="edit-commits-list">
      {#each lines as line, i (line.id)}
        <li class="line" class:dropped={line.action === "drop"} class:squashed={line.action === "squash"} id="edit-commit-{line.id.slice(0, 7)}">
          <span class="order">
            <button id="edit-commit-up-{line.id.slice(0, 7)}" title="Move up (earlier)" disabled={i === 0} onclick={() => move(i, -1)}><ArrowUp size={12} /></button>
            <button id="edit-commit-down-{line.id.slice(0, 7)}" title="Move down (later)" disabled={i === lines.length - 1} onclick={() => move(i, 1)}><ArrowDown size={12} /></button>
          </span>
          <span class="min-w-0 flex-1">
            {#if editing === line.id}
              <!-- svelte-ignore a11y_autofocus -->
              <textarea class="reword" id="edit-commit-message-{line.id.slice(0, 7)}" autofocus value={line.edited ?? line.message}
                onblur={(e) => { const text = e.currentTarget.value.trim(); line.edited = text && text !== line.message ? text : null; editing = null; }}
                onkeydown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); e.currentTarget.blur(); } if (e.key === "Escape") { e.stopPropagation(); editing = null; } }}></textarea>
            {:else}
              <button class="message" id="edit-commit-reword-{line.id.slice(0, 7)}" title="Click to reword" disabled={line.action !== "pick"} onclick={() => (editing = line.id)}>
                {#if line.edited != null}<PenLine size={11} class="shrink-0 text-primary" />{/if}<span class="truncate">{subjectOf(line)}</span>
              </button>
            {/if}
            <span class="meta">{line.id.slice(0, 7)} · {line.author} · {ago(line.date)}{line.pushed ? " · on the server" : ""}</span>
          </span>
          <Segmented id="edit-commit-action-{line.id.slice(0, 7)}" value={line.action} onchange={(v) => (line.action = v)}
            options={[{ value: "pick", label: "Keep" }, { value: "squash", icon: Combine, title: "Squash into the commit above (its changes join that one)", disabled: i === 0 },
              { value: "drop", icon: Trash2, title: "Drop: its changes go away" }]} />
        </li>
      {/each}
    </ol>
    <span class="px-2 text-[11.5px] text-muted-foreground" id="edit-commits-summary">{plural(lines.length, "commit")} → {plural(result, "commit")}</span>
    {#if pushed}
      <p class="warn" id="edit-commits-pushed-warning"><TriangleAlert size={14} class="mt-px shrink-0" />
        Some of these are already on the server: saving replaces them there too (force push). Anyone who pulled them will have to reset.</p>
    {/if}
    {#if badSquash}<p class="warn" id="edit-commits-bad-squash"><TriangleAlert size={14} class="mt-px shrink-0" />The first commit kept can't be squashed: nothing is above it.</p>{/if}
  {/if}

  {#if error}
    <div class="flex flex-col gap-1 rounded-lg bg-orange-500/10 p-2.5" id="edit-commits-error">
      <span class="flex items-center gap-1.5 text-[12px] font-semibold text-orange-500"><TriangleAlert size={13} />Nothing was changed</span>
      <span class="text-[11.5px] whitespace-pre-wrap text-muted-foreground select-text">{error}</span>
    </div>
  {/if}

  <div class="flex justify-end gap-2">
    <Button variant="secondary" size="sm" onclick={dismiss} id="edit-commits-cancel">Cancel</Button>
    <Button size="sm" onclick={save} disabled={!changed || badSquash || !kept.length || saving || model.isSyncing} id="edit-commits-save"
      class={pushed ? "bg-orange-600 hover:bg-orange-600/90" : ""}>{saving ? "Saving…" : pushed ? "Save & Force push" : "Save"}</Button>
  </div>
</div>

<style>
  .list { display: flex; max-height: 420px; flex-direction: column; gap: 4px; overflow: auto; }
  .line { display: flex; align-items: center; gap: 8px; padding: 6px 8px 6px 4px; border-radius: 10px; background: oklch(0.5 0 0 / 0.06); }
  .line.squashed { margin-left: 22px; }
  .line.dropped .message, .line.dropped .meta { text-decoration: line-through; opacity: 0.5; }
  .order { display: flex; flex-direction: column; }
  .order button { display: grid; place-items: center; width: 20px; height: 15px; border-radius: 4px; color: var(--muted-foreground); }
  .order button:hover:not(:disabled) { color: var(--foreground); background: oklch(0.5 0 0 / 0.12); }
  .order button:disabled { opacity: 0.3; }
  .message { display: flex; max-width: 100%; align-items: center; gap: 5px; font-size: 12.5px; font-weight: 600; text-align: left; }
  .message:hover:not(:disabled) { color: var(--primary); }
  .meta { display: block; font-size: 11px; color: var(--tertiary); }
  .reword { width: 100%; min-height: 54px; resize: vertical; padding: 4px 6px; border-radius: 6px; font-size: 12.5px; background: var(--quinary, oklch(0.5 0 0 / 0.1)); outline: none; }
  .warn { display: flex; gap: 6px; padding: 8px 10px; border-radius: 8px; font-size: 11.5px; line-height: 1.35; color: #ea580c; background: color-mix(in oklab, #ea580c 10%, transparent); }
</style>
