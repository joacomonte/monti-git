<!-- One row's slice of the graph. Rows touch, so segments drawn by consecutive rows join into continuous lines. -->
<script lang="ts" module>
  export const LANE = 18;
  export const PAD = 4;
  // Neon: P3 primaries, as saturated as the display allows. Index 0 is main's blue.
  const PALETTE = [
    "color(display-p3 0.15 0.55 1)", "color(display-p3 1 0.15 0.45)", "color(display-p3 0.05 1 0.5)",
    "color(display-p3 1 0.65 0)", "color(display-p3 0.65 0.25 1)", "color(display-p3 0 0.9 1)",
    "color(display-p3 1 0.2 0.9)", "color(display-p3 0.7 1 0)",
  ];
  export const laneColor = (i: number) => PALETTE[i % PALETTE.length];
</script>

<script lang="ts">
  import type { Lane, Row } from "$lib/api";

  type Props = {
    index: number; // commit index, matches Lane targets
    row: Row;
    height: number;
    width: number;
    isWorktree: boolean; // uncommitted changes: dashed line to HEAD, hollow dot
    isShelved?: boolean; // WIP saved when leaving a branch: amber dashed fork, hollow square (real commits are circles)
    isMerge: boolean;
    isHead?: boolean; // the checked-out commit: bigger dot with a soft pulsing halo
    names: (string | null)[];
    hidden?: number; // Commits collapsed: commits folded under this dot, drawn as a capsule instead of a dot
    spot?: number | null; // a hovered branch (its line color id): its line lights up, every other line fades
    grey?: boolean; // Commits collapsed: every line and dot muted except the checked-out branch's
  };
  let { index, row, height, width, isWorktree, isShelved = false, isMerge, isHead = false, names, grey = false, spot = null, hidden = 0 }: Props = $props();

  const x = (lane: number) => PAD + lane * LANE + LANE / 2;
  const mid = $derived(height / 2);

  type Seg = { d: string; color: string; width: number; dashed: boolean; glow: boolean; line: number; fade?: string }; // fade: the color it blends into at the bottom

  function path(x1: number, y1: number, x2: number, y2: number) {
    if (x1 === x2) return `M${x1} ${y1}L${x2} ${y2}`;
    const my = (y1 + y2) / 2;
    return `M${x1} ${y1}C${x1} ${my} ${x2} ${my} ${x2} ${y2}`;
  }

  function seg(d: string, lane: NonNullable<Lane>, current: boolean, dashed = false): Seg {
    const behind = (lane[2] & 1) !== 0;
    current ||= (lane[2] & 2) !== 0;
    return {
      // Thin lines; only the checked-out branch's line is thick and glows, so it stands out.
      d, dashed, line: lane[1], glow: !behind && current, color: behind ? "var(--tertiary)" : grey && !current ? soft(lane[1]) : laneColor(lane[1]),
      width: behind ? 1.5 : current ? 3.5 : 2,
    };
  }

  const segments = $derived.by(() => {
    const out: Seg[] = [];
    row.before.forEach((lane, i) => {
      if (!lane) return;
      out.push(seg(lane[0] === index ? path(x(i), 0, x(row.column), mid) : path(x(i), 0, x(i), height), lane, false));
    });
    row.parents.forEach((parent, p) => {
      const i = row.after.findIndex((l) => l?.[0] === parent);
      const lane = row.after[i];
      // Only the uncommitted row's own link to HEAD is dashed; lanes passing through stay solid.
      if (!lane) return;
      // A branch forking off another lane: the curve takes the new branch's color, not the lane it leaves from.
      const own: NonNullable<Lane> = p === 0 && i !== row.column ? [lane[0], row.color, 0] : lane;
      const s = seg(path(x(row.column), mid, x(i), height), own, row.isCurrent && p === 0, isWorktree || isShelved);
      const fade = own !== lane ? ((lane[2] & 1) !== 0 ? "var(--tertiary)" : grey && (lane[2] & 2) === 0 ? soft(lane[1]) : laneColor(lane[1])) : undefined;
      out.push(isShelved ? { ...s, color: SHELVED, glow: false, width: 2 } : { ...s, fade });
    });
    return out;
  });

  const nameOf = (target: number | undefined) => (target != null && target >= 0 ? names[target] : null) ?? "unnamed branch";
  const columns = $derived(Math.max(row.before.length, row.after.length, row.column + 1));
  const r = $derived(isMerge ? 3 : isHead ? 7.5 : 4.5); // merges recede, real commits stand out
  // Collapsed: a muted hint of each line's color, so the checked-out branch still stands out.
  const soft = (i: number) => `color-mix(in oklab, ${laneColor(i)} 65%, var(--muted-foreground))`;
  const dot = $derived(grey && !row.isCurrent && !isHead ? soft(row.color) : laneColor(row.color));
  const SHELVED = "color(display-p3 1 0.7 0.1)";
</script>

<svg {width} {height} class="block overflow-hidden">
  {#each segments as s, k}
    {#if s.fade}
      <!-- Fork curve: the new branch's color at its dot, blending into the lane it leaves from. -->
      <defs><linearGradient id="fork-{index}-{k}" gradientUnits="userSpaceOnUse" x1="0" y1={mid} x2="0" y2={height}>
        <stop offset="0" stop-color={s.color} /><stop offset="1" stop-color={s.fade} />
      </linearGradient></defs>
    {/if}
    {@const lit = spot != null && s.line === spot}
    <path d={s.d} stroke={s.fade ? `url(#fork-${index}-${k})` : s.color} stroke-width={lit ? Math.max(s.width, 3.25) : s.width} fill="none" stroke-dasharray={s.dashed ? "3 3" : undefined}
      style:opacity={spot != null && !lit ? 0.15 : 1}
      style:filter={s.glow || lit ? `drop-shadow(0 0 ${s.width > 2 || lit ? 3 : 2}px ${s.color})` : undefined} />
  {/each}
  <g style:opacity={spot != null && row.color !== spot ? 0.2 : 1}>
  {#if isShelved}
    <rect x={x(row.column) - 5} y={mid - 5} width="10" height="10" rx="2" fill="var(--background)" stroke={SHELVED} stroke-width="2" stroke-dasharray="3 2" />
  {:else if isMerge || isWorktree}
    <circle cx={x(row.column)} cy={mid} {r} fill="var(--background)" stroke={dot} stroke-width={isMerge ? 1.25 : 2} />
  {:else}
    {#if isHead}
      <!-- Two rings radiating out, half a beat apart: "you are here". -->
      {#each [0, 1.1] as delay (delay)}
        <circle class="halo" cx={x(row.column)} cy={mid} r={r} fill="none" stroke={dot} stroke-width="2"
          style:transform-origin="{x(row.column)}px {mid}px" style:animation-delay="{delay}s" />
      {/each}
    {/if}
    {#if hidden > 0 && !isHead}
      <!-- More commits folded in: the dot stretched into a short capsule along its line (same mark whatever the count). -->
      <rect x={x(row.column) - r} y={mid - r - 4} width={r * 2} height={r * 2 + 8} rx={r} fill={dot} />
    {:else}
      <circle class:core={isHead} cx={x(row.column)} cy={mid} {r} fill={dot}
        style:transform-origin="{x(row.column)}px {mid}px" style:filter={isHead ? `drop-shadow(0 0 5px ${dot})` : undefined} />
    {/if}
  {/if}
  </g>
  <!-- Native tooltips: the commit's own column, otherwise the lane passing through (top half in, bottom half out). -->
  {#each { length: columns } as _, lane}
    <rect x={PAD + lane * LANE} y="0" width={LANE} height={mid} fill="transparent">
      <title>{lane === row.column ? nameOf(index) : nameOf(row.before[lane]?.[0])}</title>
    </rect>
    <rect x={PAD + lane * LANE} y={mid} width={LANE} height={mid} fill="transparent">
      <title>{lane === row.column ? nameOf(index) : nameOf(row.after[lane]?.[0])}</title>
    </rect>
  {/each}
</svg>

<style>
  path, g { transition: opacity 0.15s; }
  .halo { opacity: 0; animation: halo 2.2s cubic-bezier(0.22, 1, 0.36, 1) infinite; }
  .core { animation: core 2.2s ease-in-out infinite; }
  @keyframes halo { 0% { opacity: 0.8; transform: scale(1); } 100% { opacity: 0; transform: scale(2.1); } }
  @keyframes core { 0%, 100% { transform: scale(1); } 50% { transform: scale(1.12); } }
  @media (prefers-reduced-motion: reduce) { .halo, .core { animation: none; } }
</style>
