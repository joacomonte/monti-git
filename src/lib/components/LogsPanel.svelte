<!-- Top-right "Logs": what you did from the app (with Undo where it's safe) and every git command it ran, with the full
     output. Grouped by repo and branch, newest group first and always open, the others folded; inside, each action
     with the commands it ran, and the rest on their own. Filters: text, errors, repo, kind. -->
<script lang="ts">
  import { Check, ChevronRight, Copy, ScrollText, Search, Undo2 } from "@lucide/svelte";
  import { api, type GitLog } from "$lib/api";
  import * as Popover from "$lib/components/ui/popover";
  import { undoHint, type Activity, type RepoModel } from "$lib/model.svelte";
  import { CopyFlash } from "$lib/clipboard.svelte";
  import { ago, baseName, gitCommand, ranBy } from "$lib/format";
  import Segmented from "./Segmented.svelte";

  let { model }: { model: RepoModel } = $props();
  let open = $state(false);
  let logs = $state<GitLog[]>([]); // oldest first, session only (Rust keeps the last 500 changes and 500 reads)
  let query = $state("");
  let errorsOnly = $state(false);
  let allRepos = $state(false);
  let showFetches = $state(false); // fetches outside any action: the automatic ones (background sync, checks)
  let kind = $state<"write" | "read" | "all">("write");
  let expanded = $state<number[]>([]);
  const flash = new CopyFlash();
  let now = $state(Date.now());

  // While open: pull new commands every 1.5s (only the ones after the last id seen); a slow pull skips the next tick.
  $effect(() => {
    if (!open) return;
    let pulling = false;
    const pull = async () => {
      if (pulling) return;
      pulling = true;
      const fresh = await api.gitLogs(logs.at(-1)?.id ?? 0).catch(() => []);
      pulling = false;
      if (fresh.length) logs = [...logs, ...fresh].slice(-1500);
      now = Date.now();
    };
    pull();
    const timer = setInterval(pull, 1500);
    return () => clearInterval(timer);
  });

  const name = baseName;
  const when = (ms: number) => ago(ms / 1000, now);
  const output = (l: GitLog) => [l.stdout, l.stderr].filter(Boolean).join(l.stdout && l.stderr ? "\n--- stderr ---\n" : "");

  const matches = (l: GitLog) =>
    (allRepos || l.repo === model.repo) && (!errorsOnly || !l.ok) && (kind === "all" || (kind === "write") === l.write)
    && (!query || (gitCommand(l) + "\n" + output(l)).toLowerCase().includes(query.toLowerCase()));

  type Item = { at: number; action: Activity | null; commands: GitLog[] };
  /** Newest first: each action with the commands that ran while it did, then the commands outside any action. */
  const items = $derived.by(() => {
    const shown = logs.filter(matches);
    const used = new Set<number>();
    const actions: Item[] = model.activity
      .filter((a) => allRepos || a.repo === model.repo)
      .map((a) => {
        const commands = shown.filter((l) => ranBy(l, a));
        commands.forEach((l) => used.add(l.id));
        return { at: a.at, action: a, commands };
      })
      // With a filter on, an action only stays if some of its commands match (or its text does).
      .filter((it) => it.commands.length || (!errorsOnly && kind !== "read" && (!query || it.action!.text.toLowerCase().includes(query.toLowerCase()))));
    // A fetch you started belongs to its action; one on its own is the app's background sync: hidden unless asked for.
    const isAutoFetch = (l: GitLog) => l.args[0] === "fetch";
    const loose: Item[] = shown.filter((l) => !used.has(l.id) && (showFetches || !isAutoFetch(l))).map((l) => ({ at: l.at, action: null, commands: [l] }));
    return [...actions, ...loose].sort((a, b) => b.at - a.at).slice(0, 400);
  });

  /** The items above by repo and branch (checked out when they ran), newest group first. Commands outside any action
   *  don't know their branch: they go in their repo's "Other commands". */
  type Group = { key: string; repo: string; branch: string | null; items: Item[] };
  const groups = $derived.by(() => {
    const byKey = new Map<string, Group>();
    for (const it of items) {
      const repo = it.action?.repo ?? it.commands[0].repo, branch = it.action?.branch ?? null;
      const key = `${repo}\0${branch ?? ""}`;
      const group = byKey.get(key) ?? byKey.set(key, { key, repo, branch, items: [] }).get(key)!;
      group.items.push(it);
    }
    return [...byKey.values()];
  });
  let unfolded = $state<string[]>([]); // groups opened by hand; the newest is always open
  const toggleGroup = (key: string) => (unfolded = unfolded.includes(key) ? unfolded.filter((k) => k !== key) : [...unfolded, key]);

  const toggle = (id: number) => (expanded = expanded.includes(id) ? expanded.filter((x) => x !== id) : [...expanded, id]);
  const copy = (l: GitLog) => flash.copy(String(l.id), `$ ${gitCommand(l)}\n${output(l)}`);
</script>

<!-- Which repo a group's actions and commands ran in. -->
{#snippet repoChip(repo: string)}<span class="repo-chip" title={repo}>{name(repo)}</span>{/snippet}

{#snippet line(l: GitLog)}
  {@const isOpen = expanded.includes(l.id)}
  <div class="cmd" class:failed={!l.ok}>
    <button class="cmd-head" id="log-command-{l.id}" onclick={() => toggle(l.id)} aria-expanded={isOpen}>
      <ChevronRight size={11} class="shrink-0 transition-transform {isOpen ? 'rotate-90' : ''}" />
      <span class="dot" class:read={!l.write}></span>
      <span class="min-w-0 flex-1 truncate font-mono" title={gitCommand(l)}>{gitCommand(l)}</span>
      <span class="meta">{#if !l.ok}exit {l.code ?? "?"} · {/if}{l.ms}ms · {when(l.at)}</span>
    </button>
    {#if isOpen}
      <div class="out">
        <button class="copy" id="log-copy-{l.id}" title="Copy the command and its output" onclick={() => copy(l)}>
          {#if flash.copied === String(l.id)}<Check size={11} />{:else}<Copy size={11} />{/if}</button>
        <pre>{output(l) || "(no output)"}</pre>
        {#if l.truncated}<span class="meta">Output cut: reads keep 8 KB, changes 256 KB.</span>{/if}
      </div>
    {/if}
  </div>
{/snippet}

<Popover.Root bind:open>
  <Popover.Trigger class="logs-trigger" id="logs-open" title="What you did from the app, with Undo, and every git command it ran with its full output">
    <ScrollText size={14} />Logs
  </Popover.Trigger>
  <Popover.Content align="end" class="w-[640px] p-0" id="logs-panel">
    <div class="flex flex-col gap-2 border-b p-2.5">
      <div class="flex items-center gap-2">
        <span class="text-[11px] font-semibold text-muted-foreground uppercase">Logs · {allRepos ? "all repos" : model.repo ? name(model.repo) : ""}</span>
        <span class="ml-auto text-[11px] text-tertiary">This session · Undo where it's safe</span>
      </div>
      <div class="flex items-center gap-1.5">
        <label class="search"><Search size={12} /><input id="logs-search" placeholder="Search commands and output" bind:value={query} /></label>
        <Segmented id="logs-kind" value={kind} onchange={(v) => (kind = v)}
          options={[{ value: "write", label: "Changes" }, { value: "read", label: "Reads" }, { value: "all", label: "All" }]} />
        <button class="chip" id="logs-errors" class:on={errorsOnly} aria-pressed={errorsOnly} onclick={() => (errorsOnly = !errorsOnly)}>Errors</button>
        <button class="chip" id="logs-auto-fetch" class:on={showFetches} aria-pressed={showFetches} onclick={() => (showFetches = !showFetches)}
          title="The fetches the app runs by itself (background sync, checks before an action). Hidden by default.">Auto fetch</button>
        <button class="chip" id="logs-all-repos" class:on={allRepos} aria-pressed={allRepos} onclick={() => (allRepos = !allRepos)}>All repos</button>
      </div>
    </div>
    <div class="list">
      {#each groups as g, i (g.key)}
        {@const isOpen = i === 0 || unfolded.includes(g.key)}
        <div class="group">
          <button class="group-head" id="logs-group-{i}" disabled={i === 0} aria-expanded={isOpen} onclick={() => toggleGroup(g.key)}
            title={i === 0 ? "The latest: always open" : isOpen ? "Fold" : "Show what ran here"}>
            <ChevronRight size={12} class="shrink-0 transition-transform {isOpen ? 'rotate-90' : ''}" />
            {@render repoChip(g.repo)}
            <span class="min-w-0 flex-1 truncate text-[12.5px] font-semibold">{g.branch ?? "Other commands"}</span>
            <span class="meta">{g.items.length} · {when(g.items[0].at)}</span>
          </button>
          {#if isOpen}
            <div class="group-body">
              {#each g.items as it (it.action ? `a${it.action.at}${it.action.text}` : `c${it.commands[0].id}`)}
              {#if it.action}
                {@const a = it.action}
                <div class="action">
                  <div class="flex items-center gap-2">
                    <span class="min-w-0 flex-1">
                      <span class="block truncate text-[12.5px] font-semibold" class:line-through={a.undone} title={a.text}>{a.text}</span>
                      <span class="meta">{when(a.at)}{a.undone ? " · undone" : ""}{#if it.commands.length} · {it.commands.length} command{it.commands.length === 1 ? "" : "s"}{/if}</span>
                    </span>
                    {#if a.undo && !a.undone && a.repo === model.repo}
                      <button class="undo" id="logs-undo-{a.at}" disabled={model.isSyncing}
                        title={undoHint(a.undo)}
                        onclick={() => model.undo(a)}><Undo2 size={12} />Undo</button>
                    {/if}
                  </div>
                  {#each it.commands as l (l.id)}{@render line(l)}{/each}
                  <!-- Commands live in memory while the app runs; actions are saved. An older action has none to show. -->
                  {#if !it.commands.length}<span class="meta block px-1.5 pt-1">Its git commands aren't kept: they're only listed while the app stays open.</span>{/if}
                </div>
              {:else}
                {@render line(it.commands[0])}
              {/if}
              {/each}
            </div>
          {/if}
        </div>
      {:else}
        <p class="px-2 py-8 text-center text-[12px] text-tertiary">
          {logs.length ? "Nothing matches these filters." : "Nothing yet. Checkouts, pulls, commits… and the git commands behind them show up here."}</p>
      {/each}
    </div>
  </Popover.Content>
</Popover.Root>

<style>
  :global(.logs-trigger) { display: flex; align-items: center; gap: 6px; height: 30px; padding: 0 10px; border-radius: 8px; font-size: 12.5px; font-weight: 600; color: var(--muted-foreground); }
  :global(.logs-trigger:hover), :global(.logs-trigger[data-state="open"]) { color: var(--foreground); background: oklch(0.5 0 0 / 0.1); }
  .search { display: flex; flex: 1; align-items: center; gap: 6px; height: 28px; padding: 0 9px; border-radius: 8px; color: var(--tertiary); background: oklch(0.5 0 0 / 0.08); }
  .search input { flex: 1; min-width: 0; font-size: 12px; color: var(--foreground); background: none; outline: none; }
  .chip { height: 28px; padding: 0 10px; border-radius: 8px; font-size: 11.5px; font-weight: 600; color: var(--muted-foreground); box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.2); }
  .chip.on { color: var(--primary); background: color-mix(in oklab, var(--primary) 12%, transparent); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--primary) 40%, transparent); }
  .list { display: flex; flex-direction: column; gap: 2px; max-height: min(70vh, 640px); overflow: auto; padding: 6px; }
  .action { display: flex; flex-direction: column; gap: 2px; padding: 7px 8px; border-radius: 10px; background: oklch(0.5 0 0 / 0.05); }
  .action + .action, .cmd + .action, .action + .cmd { margin-top: 4px; }
  .group + .group { margin-top: 4px; }
  .group-head { display: flex; width: 100%; align-items: center; gap: 7px; height: 32px; padding: 0 8px; border-radius: 8px; text-align: left; color: var(--foreground); }
  .group-head:hover:not(:disabled) { background: oklch(0.5 0 0 / 0.08); }
  .group-head:disabled { cursor: default; opacity: 1; }
  .group-body { display: flex; flex-direction: column; gap: 2px; padding: 2px 0 4px 14px; }
  .repo-chip { flex-shrink: 0; max-width: 160px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding: 1px 6px; border-radius: 5px;
    font-size: 10.5px; font-weight: 600; color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.12); }
  .meta { font-size: 10.5px; color: var(--tertiary); white-space: nowrap; }
  .cmd-head { display: flex; width: 100%; align-items: center; gap: 7px; height: 26px; padding: 0 6px; border-radius: 6px; font-size: 11.5px; text-align: left; color: var(--muted-foreground); }
  .cmd-head:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.08); }
  .dot { width: 6px; height: 6px; flex-shrink: 0; border-radius: 99px; background: #34c77b; }
  .dot.read { background: oklch(0.5 0 0 / 0.45); }
  .failed .dot { background: #ef4444; }
  .failed .cmd-head { color: color-mix(in oklab, #ef4444 70%, var(--foreground)); }
  .out { position: relative; margin: 2px 0 6px 18px; }
  .out pre { max-height: 320px; overflow: auto; padding: 8px 10px; border-radius: 8px; font: 11px/1.5 var(--font-mono, ui-monospace, monospace); white-space: pre-wrap; word-break: break-word; color: var(--muted-foreground); background: var(--quinary, oklch(0.5 0 0 / 0.08)); user-select: text; }
  .copy { position: absolute; top: 5px; right: 5px; display: grid; place-items: center; width: 22px; height: 22px; border-radius: 6px; color: var(--tertiary); background: var(--popover); }
  .copy:hover { color: var(--foreground); }
  .undo { display: flex; flex-shrink: 0; align-items: center; gap: 4px; height: 24px; padding: 0 8px; border-radius: 7px; font-size: 11.5px; color: var(--muted-foreground); box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.2); }
  .undo:hover:not(:disabled) { color: var(--foreground); background: oklch(0.5 0 0 / 0.1); }
  .undo:disabled { opacity: 0.45; }
</style>
