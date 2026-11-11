<!-- A branch card's deploy on GitHub: a rocket by its state and, while it goes, a bar along the card's bottom edge
     that fills step by step (a Deployment from Vercel & co. has no steps: the bar sweeps instead). Done: a small
     green rocket stays; failed: red. Click opens the run or the deploy's log. -->
<script lang="ts">
  import { Rocket } from "@lucide/svelte";
  import { api, type Deploy } from "$lib/api";

  let { deploy, id }: { deploy: Deploy; id: string } = $props();
  const going = $derived(deploy.state === "queued" || deploy.state === "running");
  const steps = $derived(deploy.total > 0); // a workflow run: its steps; a Deployment: none to count
  // Never empty while it goes (queued shows a sliver), never full until it's done.
  const percent = $derived(going ? Math.min(96, Math.max(4, deploy.total ? (deploy.done / deploy.total) * 100 : 0)) : 100);
  const HEAD = { queued: "Deploy queued", running: "Deploying", success: "Deployed", failure: "Deploy failed", cancelled: "Deploy cancelled" };
  const title = $derived(
    [`${HEAD[deploy.state]}${deploy.state === "running" && steps ? ` · ${deploy.done} of ${deploy.total} steps` : ""}`, deploy.name, steps ? "Click: open the run on GitHub" : "Click: open its log"].join("\n"),
  );
</script>

<button class="deploy {deploy.state}" {id} {title} onclick={(e) => { e.stopPropagation(); api.openUrl(deploy.url); }}>
  <Rocket size={12} strokeWidth={2.5} />
</button>
{#if going || deploy.state === "failure"}
  <span class="track" aria-hidden="true"><span class="fill {deploy.state}" class:sweep={going && !steps} style:width="{going && !steps ? 35 : percent}%"></span></span>
{/if}

<style>
  .deploy { display: grid; flex-shrink: 0; place-items: center; width: 22px; height: 22px; border-radius: 99px; color: var(--c); background: color-mix(in oklab, var(--c) 15%, transparent); }
  .deploy:hover { background: color-mix(in oklab, var(--c) 26%, transparent); }
  .queued, .running { --c: #4f8cff; }
  .success { --c: #34c77b; }
  .failure { --c: #f43f5e; }
  .cancelled { --c: #8b93a1; }
  .running :global(svg) { animation: lift 1.2s ease-in-out infinite; }
  @keyframes lift { 50% { transform: translate(1px, -1.5px); } }

  /* Along the card's bottom edge, inside its rounded corners. */
  .track { position: absolute; right: 10px; bottom: 3px; left: 10px; height: 3px; overflow: hidden; border-radius: 99px; background: oklch(0.5 0 0 / 0.18); pointer-events: none; }
  .fill { display: block; height: 100%; border-radius: inherit; transition: width 0.6s cubic-bezier(0.2, 0.8, 0.2, 1); }
  .fill.queued, .fill.running {
    background: linear-gradient(90deg, #4f8cff, #a78bfa, #4f8cff); background-size: 200% 100%;
    box-shadow: 0 0 8px color-mix(in oklab, #4f8cff 60%, transparent); animation: flow 1.6s linear infinite;
  }
  .fill.failure { background: #f43f5e; }
  .fill.sweep { animation: flow 1.6s linear infinite, sweep 1.4s ease-in-out infinite; transition: none; }
  @keyframes sweep { from { transform: translateX(-100%); } to { transform: translateX(290%); } }
  @keyframes flow { to { background-position: -200% 0; } }
  @media (prefers-reduced-motion: reduce) { .fill, .running :global(svg) { animation: none; } }
</style>
