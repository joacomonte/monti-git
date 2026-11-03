<!-- Commit form above the file tree, on the uncommitted-changes row. Commits only what's checked (staged) in the tree;
     the placeholders show the previous commit's message as an example. -->
<script lang="ts">
  import { ChevronDown, CloudUpload, GitCommitHorizontal, PenLine, Sparkles } from "@lucide/svelte";
  import { api } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import * as Popover from "$lib/components/ui/popover";
  import MessagePrompt from "./MessagePrompt.svelte";

  let { model }: { model: RepoModel } = $props();
  let title = $state("");
  let description = $state("");
  let previous = $state({ title: "", description: "" });
  let showMenu = $state(false);
  let showAi = $state<"pick" | "new" | "amend" | null>(null); // the "Message for AI" dialog: pick a mode, or start on one

  const stagedCount = $derived(Object.keys(model.changes).filter((p) => model.staged.has(p)).length);
  const total = $derived(Object.keys(model.changes).length);
  const canCommit = $derived(stagedCount > 0 && title.trim() !== "" && !model.isSyncing);

  $effect(() => {
    const repo = model.repo;
    if (!repo) return;
    api.lastCommitMessage(repo).then((message) => {
      const [first, ...rest] = message.split("\n");
      previous = { title: first, description: rest.join("\n").trim() };
    });
  });

  async function commit(push: boolean) {
    showMenu = false;
    if (!canCommit) return;
    const message = title.trim() + (description.trim() ? "\n\n" + description.trim() : "") + "\n";
    if (await model.commit(message, push)) {
      previous = { title: title.trim(), description: description.trim() };
      title = description = "";
    }
  }

  /** ⌘↩ commits; plain Return in the title jumps to the description. */
  function keydown(e: KeyboardEvent) {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      commit(false);
    } else if (e.key === "Enter" && e.target instanceof HTMLInputElement) {
      e.preventDefault();
      document.getElementById("commit-description")?.focus();
    }
  }
</script>

<div class="enter flex flex-col gap-1.5 border-b p-2" id="commit-box">
  <!-- What to do with the uncommitted changes, with your AI agent writing the message. -->
  <div class="flex items-center gap-1.5">
    <span class="flex-1 text-[11px] font-semibold text-muted-foreground">Uncommitted changes</span>
    <button class="choice" id="quick-commit" title="Commit message by your AI agent, for a new commit" onclick={() => (showAi = "new")}>
      <GitCommitHorizontal size={13} />Commit</button>
    <button class="choice" id="quick-amend" title="Amend the last commit, with your AI agent" onclick={() => (showAi = "amend")}>
      <PenLine size={13} />Amend</button>
  </div>
  <input id="commit-title" bind:value={title} onkeydown={keydown} placeholder={previous.title || "Title"} autocomplete="off"
    class="h-7 rounded-md bg-quinary px-2 text-[12px] font-medium outline-none focus:ring-2 focus:ring-primary/40" />
  <textarea id="commit-description" bind:value={description} onkeydown={keydown} placeholder={previous.description || "Description (optional)"}
    class="h-[64px] resize-none rounded-md bg-quinary px-2 py-1.5 text-[12px] outline-none focus:ring-2 focus:ring-primary/40"></textarea>
  <div class="flex items-center gap-2">
    <span class="flex-1 truncate text-[11px] text-muted-foreground" id="commit-staged-count">
      {stagedCount ? `${stagedCount} of ${total} file${total === 1 ? "" : "s"} staged` : "Check files below to stage them"}
    </span>
    <button class="ai-message" id="commit-message-for-ai" onclick={() => (showAi = "pick")}><Sparkles size={12} />Message for AI</button>
    {#if showAi}<MessagePrompt {model} start={showAi === "pick" ? null : showAi} onclose={() => (showAi = null)} />{/if}
    <!-- Split button: Commit, and Commit & Push in the ⌄ menu. -->
    <div class="split flex h-7 shrink-0 overflow-hidden rounded-md" class:disabled={!canCommit}>
      <button class="px-3 text-[12px] font-semibold" id="commit-button" disabled={!canCommit} onclick={() => commit(false)}
        title="Commit the staged files (⌘↩)">Commit</button>
      <Popover.Root bind:open={showMenu}>
        <Popover.Trigger class="border-l border-white/25 px-1.5" id="commit-menu" disabled={!canCommit} title="More commit options">
          <ChevronDown size={11} strokeWidth={3} />
        </Popover.Trigger>
        <Popover.Content align="end" class="w-auto p-1">
          <button class="option flex h-8 items-center gap-2 rounded-md px-2.5 text-[12px]" id="commit-and-push" onclick={() => commit(true)}
            title="Commit the staged files, then push them to the server (sets the upstream if it has none)">
            <CloudUpload size={13} />Commit & Push
          </button>
        </Popover.Content>
      </Popover.Root>
    </div>
  </div>
</div>

<style>
  .ai-message {
    display: flex; flex-shrink: 0; align-items: center; gap: 5px; height: 28px; padding: 0 9px; border-radius: 6px;
    font-size: 11.5px; font-weight: 600; color: var(--muted-foreground); box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.22);
  }
  .ai-message:hover { color: white; background: #8b5cf6; box-shadow: none; }
  .choice { display: flex; align-items: center; gap: 5px; height: 26px; padding: 0 10px; border-radius: 999px; font-size: 12px; font-weight: 600;
    color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.12); }
  .choice:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.2); }
  .split { color: white; background: var(--primary); }
  .split.disabled { opacity: 0.45; }
  .split > :global(button:not(:disabled):hover) { background: color-mix(in oklab, var(--primary) 85%, black); }
  .option:hover { background: var(--quinary); }
</style>
