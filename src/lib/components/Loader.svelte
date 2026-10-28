<!-- A tiny looping git graph: a glowing head walks along main while a feature branch forks off and merges back. -->
<script lang="ts">
  let { title, detail = "" }: { title: string; detail?: string } = $props();
</script>

<div class="flex h-full flex-col items-center justify-center gap-3.5">
  <svg width="160" height="50" viewBox="0 0 160 50" aria-label="Loading">
    <path d="M12 38H148" class="track" />
    <path d="M46 38C46 12 62 12 80 12C98 12 114 12 114 38" class="track" />
    <path d="M12 38H148" class="walk" pathLength="1" />
    {#each [12, 46, 80, 114, 148] as x, i}<circle cx={x} cy="38" r="5" class="dot" style:animation-delay="{i * 0.6 - 0.1}s" />{/each}
    <circle cx="80" cy="12" r="5" class="dot" style:animation-delay="1.1s" />
    <circle cy="38" r="9" class="head" />
  </svg>
  <div class="flex flex-col items-center gap-1">
    <span class="text-[13px] font-semibold">{title}</span>
    <span class="text-[11px] text-muted-foreground tabular-nums">{detail ? detail + "…" : " "}</span>
  </div>
</div>

<style>
  .track { stroke: var(--tertiary); stroke-opacity: 0.35; stroke-width: 2; fill: none; }
  .walk { stroke: var(--primary); stroke-width: 2; fill: none; stroke-dasharray: 1; animation: walk 2.4s linear infinite; }
  .dot { fill: var(--background); stroke: var(--tertiary); stroke-width: 1.5; animation: lit 2.4s step-end infinite; }
  .head { fill: var(--primary); fill-opacity: 0.3; animation: head 2.4s linear infinite; }
  @keyframes walk { from { stroke-dashoffset: 1; } to { stroke-dashoffset: 0; } }
  @keyframes head { from { cx: 12px; } to { cx: 148px; } }
  @keyframes lit { 0% { fill: var(--background); } 50% { fill: var(--primary); stroke: var(--primary); } }
</style>
