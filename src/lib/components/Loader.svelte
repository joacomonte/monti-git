<!-- Three commits on a line, lighting up one after another. -->
<script lang="ts">
  let { title, detail = "" }: { title: string; detail?: string } = $props();
</script>

<div class="flex h-full flex-col items-center justify-center gap-3.5">
  <svg width="88" height="20" viewBox="0 0 88 20" aria-label="Loading">
    <path d="M10 10H78" class="track" />
    {#each [10, 44, 78] as x, i}<circle cx={x} cy="10" r="5" class="dot" style:animation-delay="{i * 0.18}s" />{/each}
  </svg>
  <div class="flex flex-col items-center gap-1">
    <span class="text-[13px] font-semibold">{title}</span>
    <span class="text-[11px] text-muted-foreground tabular-nums">{detail ? detail + "…" : " "}</span>
  </div>
</div>

<style>
  .track { stroke: var(--tertiary); stroke-opacity: 0.3; stroke-width: 2; }
  .dot { fill: var(--primary); transform-box: fill-box; transform-origin: center; animation: pulse 1.1s ease-in-out infinite; }
  @keyframes pulse { 0%, 100% { opacity: 0.25; transform: scale(0.7); } 40% { opacity: 1; transform: scale(1); } }
  @media (prefers-reduced-motion: reduce) { .dot { animation: none; opacity: 0.7; } }
</style>
