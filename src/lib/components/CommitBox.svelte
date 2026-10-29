<!-- Commit form above the file tree, on the uncommitted-changes row. Commits only what's checked (staged) in the tree;
     the placeholders show the previous commit's message as an example. -->
<script lang="ts">
  import { ChevronDown, CloudUpload } from "@lucide/svelte";
  import { api } from "$lib/api";
  import type { RepoModel } from "$lib/model.svelte";
  import * as Popover from "$lib/components/ui/popover";

  let { model }: { model: RepoModel } = $props();
  let title = $state("");
  let description = $state("");
  let previous = $state({ title: "", description: "" });
  let showMenu = $state(false);

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
  <input id="commit-title" bind:value={title} onkeydown={keydown} placeholder={previous.title || "Title"} autocomplete="off"
    class="h-7 rounded-md bg-quinary px-2 text-[12px] font-medium outline-none focus:ring-2 focus:ring-primary/40" />
  <textarea id="commit-description" bind:value={description} onkeydown={keydown} placeholder={previous.description || "Description (optional)"}
    class="h-[64px] resize-none rounded-md bg-quinary px-2 py-1.5 text-[12px] outline-none focus:ring-2 focus:ring-primary/40"></textarea>
  <div class="flex items-center gap-2">
    <span class="flex-1 truncate text-[11px] text-muted-foreground" id="commit-staged-count">
      {stagedCount ? `${stagedCount} of ${total} file${total === 1 ? "" : "s"} staged` : "Check files below to stage them"}
    </span>
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
  .split { color: white; background: var(--primary); }
  .split.disabled { opacity: 0.45; }
  .split > :global(button:not(:disabled):hover) { background: color-mix(in oklab, var(--primary) 85%, black); }
  .option:hover { background: var(--quinary); }
</style>
