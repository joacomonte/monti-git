<!-- First time a repo opens: which branches are environments, and which one is the base. Big tiles for the branches
     that look like one (pre-checked when detected), a search for any other. Saving shrinks the panel into the toolbar's
     Configure button, where the same picks live from then on. -->
<script lang="ts">
  import { Check, House, Plus, Search } from "@lucide/svelte";
  import { fade } from "svelte/transition";
  import type { RepoModel } from "$lib/model.svelte";
  import { baseName } from "$lib/format";
  import { localName } from "$lib/refs";

  let { model }: { model: RepoModel } = $props();
  // Names that usually mean an environment or production (refs.rs ENVIRONMENT_NAMES, plus the usual bases).
  const LIKELY = ["main", "master", "production", "prod", "qa", "dev", "develop", "development", "staging", "stage", "test", "testing", "uat", "preprod", "pre-prod", "release"];

  const all = $derived([...new Set([...model.history.local, ...model.history.allBranches.map(localName)])]);
  // svelte-ignore state_referenced_locally
  let base = $state(model.history.base ? localName(model.history.base) : null);
  // svelte-ignore state_referenced_locally
  let picked = $state<string[]>([...(base ? [base] : []), ...model.history.environments.map(localName)]);
  let added = $state<string[]>([]); // found with the search: they get a tile too
  let query = $state("");
  let panel = $state<HTMLElement | null>(null);

  const tiles = $derived([...new Set([...picked, ...added, ...all.filter((n) => LIKELY.includes(n.toLowerCase()))])]);
  const found = $derived(query.trim() ? all.filter((n) => !tiles.includes(n) && n.toLowerCase().includes(query.trim().toLowerCase())).slice(0, 8) : []);

  function toggle(name: string) {
    if (name === base) return; // the base stays checked: move the house to change it
    picked = picked.includes(name) ? picked.filter((n) => n !== name) : [...picked, name];
  }
  function makeBase(name: string) {
    base = name;
    if (!picked.includes(name)) picked = [...picked, name];
  }
  function add(name: string) {
    added = [...added, name];
    picked = [...picked, name];
    query = "";
  }

  /** The panel flies into Configure and shrinks away, the button bumps: that's where these picks are now. */
  async function save() {
    const target = document.getElementById("compare-open")?.getBoundingClientRect();
    const from = panel?.getBoundingClientRect();
    if (panel && target && from) {
      const dx = target.left + target.width / 2 - (from.left + from.width / 2), dy = target.top + target.height / 2 - (from.top + from.height / 2);
      await panel.animate([{ transform: "none", opacity: 1 }, { transform: `translate(${dx}px, ${dy}px) scale(0.06)`, opacity: 0.4 }],
        { duration: 650, easing: "cubic-bezier(0.55, 0, 0.75, 0.2)", fill: "forwards" }).finished;
      document.getElementById("compare-open")?.animate([{ transform: "scale(1)" }, { transform: "scale(1.18)" }, { transform: "scale(1)" }], { duration: 420, easing: "ease-out" });
    }
    model.configure(base ?? picked[0], picked);
  }
</script>

<div class="overlay" id="repo-setup" transition:fade={{ duration: 200 }}>
  <div class="panel" bind:this={panel}>
    <h2>Set up {baseName(model.repo ?? "")}</h2>
    <p class="lead">Which branches are environments? Every branch shows whether its work reached each one. The <House size={13} class="inline -mt-0.5" /> marks the base: where branches start, pinned left.</p>

    <div class="tiles">
      {#each tiles as name (name)}
        {@const on = picked.includes(name)}
        {@const isBase = name === base}
        <div class="tile" class:on class:is-base={isBase}>
          <button class="tile-pick" id="setup-environment-{name}" role="checkbox" aria-checked={on} onclick={() => toggle(name)}
            title={isBase ? "The base: give another one the house to change it" : on ? `Not an environment` : `${name} is an environment`}>
            <span class="box">{#if on}<Check size={14} strokeWidth={3} />{/if}</span>
            <span class="name">{name}</span>
            {#if isBase}<span class="tag">Base</span>{/if}
          </button>
          <button class="tile-house" id="setup-base-{name}" disabled={isBase} onclick={() => makeBase(name)} title={isBase ? `${name} is the base` : `Make ${name} the base`}>
            <House size={16} /></button>
        </div>
      {/each}
    </div>

    <label class="search"><Search size={14} /><input id="setup-search" placeholder="Add another branch" bind:value={query} /></label>
    {#if found.length}
      <div class="found">
        {#each found as name (name)}<button class="chip" id="setup-add-{name}" onclick={() => add(name)}><Plus size={12} />{name}</button>{/each}
      </div>
    {/if}

    <div class="buttons">
      <button class="skip" id="setup-skip" title="Keep what was detected by name; change it any time in Configure"
        onclick={() => model.configure(base ?? picked[0] ?? "main", model.history.environments)}>Skip</button>
      <button class="save" id="setup-save" disabled={!picked.length} onclick={save}><Check size={16} />Save</button>
    </div>
  </div>
</div>

<style>
  .overlay { position: absolute; inset: 0; z-index: 30; display: grid; place-items: center; padding: 24px; overflow: auto;
    background: color-mix(in oklab, var(--background) 82%, transparent); backdrop-filter: blur(6px); }
  .panel { display: flex; width: min(640px, 100%); flex-direction: column; gap: 14px; padding: 26px; border-radius: 18px; transform-origin: center;
    background: var(--popover); box-shadow: 0 0 0 1px oklch(0.5 0 0 / 0.18), 0 24px 60px rgb(0 0 0 / 0.35); }
  h2 { font-size: 20px; font-weight: 800; letter-spacing: -0.01em; }
  .lead { font-size: 13px; line-height: 1.5; color: var(--muted-foreground); }
  .tiles { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 10px; }
  .tile { position: relative; border-radius: 14px; background: oklch(0.5 0 0 / 0.07); box-shadow: inset 0 0 0 1.5px oklch(0.5 0 0 / 0.14);
    transition: background 0.2s, box-shadow 0.2s, transform 0.12s; }
  .tile:hover { background: oklch(0.5 0 0 / 0.12); }
  .tile:active { transform: scale(0.97); }
  .tile.on { background: color-mix(in oklab, var(--primary) 14%, transparent); box-shadow: inset 0 0 0 1.5px color-mix(in oklab, var(--primary) 55%, transparent); }
  .tile-pick { display: flex; width: 100%; height: 84px; flex-direction: column; align-items: flex-start; justify-content: space-between; padding: 14px; text-align: left; }
  .box { display: grid; place-items: center; width: 22px; height: 22px; border-radius: 7px; color: white; box-shadow: inset 0 0 0 1.5px oklch(0.5 0 0 / 0.45); transition: background 0.2s; }
  .tile.on .box { background: var(--primary); box-shadow: none; }
  .name { max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 15px; font-weight: 700; }
  .tag { position: absolute; top: 15px; left: 46px; font-size: 10.5px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; color: var(--primary); }
  .tile-house { position: absolute; top: 8px; right: 8px; display: grid; place-items: center; width: 32px; height: 32px; border-radius: 9px; color: var(--tertiary);
    transition: color 0.15s, background 0.15s; }
  .tile-house:hover:not(:disabled) { color: var(--primary); background: oklch(0.5 0 0 / 0.12); }
  .tile.is-base .tile-house { color: var(--primary); cursor: default; }
  .search { display: flex; align-items: center; gap: 8px; height: 36px; padding: 0 12px; border-radius: 10px; color: var(--tertiary); background: oklch(0.5 0 0 / 0.08); }
  .search input { flex: 1; min-width: 0; font-size: 13px; color: var(--foreground); background: none; outline: none; }
  .found { display: flex; flex-wrap: wrap; gap: 6px; margin-top: -6px; }
  .chip { display: flex; align-items: center; gap: 4px; height: 28px; padding: 0 10px; border-radius: 99px; font-size: 12px; font-weight: 600; color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.1); }
  .chip:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.18); }
  .buttons { display: flex; justify-content: flex-end; gap: 10px; padding-top: 4px; }
  .buttons button { display: flex; align-items: center; gap: 6px; height: 40px; padding: 0 20px; border-radius: 10px; font-size: 14px; font-weight: 700; }
  .skip { color: var(--muted-foreground); }
  .skip:hover { color: var(--foreground); background: oklch(0.5 0 0 / 0.1); }
  .save { color: white; background: var(--primary); }
  .save:disabled { opacity: 0.4; }
</style>
