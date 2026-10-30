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
    names: (string | null)[];
    highlight: Set<number>; // commit indices of the branch shown from the dashboard: its line glows, everything else dims
  };
  let { index, row, height, width, isWorktree, isShelved = false, isMerge, names, highlight }: Props = $props();

  const x = (lane: number) => PAD + lane * LANE + LANE / 2;
  const mid = $derived(height / 2);

  type Seg = { d: string; color: string; width: number; dashed: boolean; glow: boolean; opacity: number };
  const focusing = $derived(highlight.size > 0);

  function path(x1: number, y1: number, x2: number, y2: number) {
    if (x1 === x2) return `M${x1} ${y1}L${x2} ${y2}`;
    const my = (y1 + y2) / 2;
    return `M${x1} ${y1}C${x1} ${my} ${x2} ${my} ${x2} ${y2}`;
  }

  /** `focus`: null when nothing is highlighted, else whether this segment is on the highlighted branch. */
  function seg(d: string, lane: NonNullable<Lane>, current: boolean, focus: boolean | null, dashed = false): Seg {
    const behind = (lane[2] & 1) !== 0;
    current ||= (lane[2] & 2) !== 0 || focus === true;
    return {
      d, dashed, glow: !behind && focus !== false, color: behind ? "var(--tertiary)" : laneColor(lane[1]),
      width: focus === true ? 5 : behind ? 1.5 : current ? 4 : 2, opacity: focus === false ? (behind ? 0.3 : 0.2) : 1,
    };
  }

  const segments = $derived.by(() => {
    const out: Seg[] = [];
    row.before.forEach((lane, i) => {
      if (!lane) return;
      const focus = focusing ? highlight.has(lane[0]) : null;
      out.push(seg(lane[0] === index ? path(x(i), 0, x(row.column), mid) : path(x(i), 0, x(i), height), lane, false, focus));
    });
    row.parents.forEach((parent, p) => {
      const i = row.after.findIndex((l) => l?.[0] === parent);
      const lane = row.after[i];
      // Only the uncommitted row's own link to HEAD is dashed; lanes passing through stay solid.
      const focus = focusing ? highlight.has(index) && highlight.has(parent) : null;
      if (!lane) return;
      const s = seg(path(x(row.column), mid, x(i), height), lane, row.isCurrent && p === 0, focus, isWorktree || isShelved);
      out.push(isShelved ? { ...s, color: SHELVED, glow: false, width: 2 } : s);
    });
    return out;
  });

  const nameOf = (target: number | undefined) => (target != null && target >= 0 ? names[target] : null) ?? "unnamed branch";
  const columns = $derived(Math.max(row.before.length, row.after.length, row.column + 1));
  const isFocused = $derived(highlight.has(index));
  const r = $derived(isMerge ? 3.5 : isFocused ? 7 : 5.5); // merges recede, real commits stand out
  const dotOpacity = $derived(focusing && !isFocused ? 0.25 : 1);
  const dot = $derived(laneColor(row.color));
  const SHELVED = "color(display-p3 1 0.7 0.1)";
</script>

<svg {width} {height} class="block overflow-hidden">
  {#each segments as s}
    <path d={s.d} stroke={s.color} stroke-width={s.width} fill="none" stroke-dasharray={s.dashed ? "3 3" : undefined} opacity={s.opacity}
      style:filter={s.glow ? `drop-shadow(0 0 ${s.width > 2 ? 3 : 2}px ${s.color})` : undefined} />
  {/each}
  {#if isShelved}
    <rect x={x(row.column) - 5} y={mid - 5} width="10" height="10" rx="2" fill="var(--background)" stroke={SHELVED} stroke-width="2" stroke-dasharray="3 2" opacity={dotOpacity} />
  {:else if isMerge || isWorktree}
    <circle cx={x(row.column)} cy={mid} {r} fill="var(--background)" stroke={dot} stroke-width={isMerge ? 1.5 : 2} stroke-opacity={dotOpacity} />
  {:else}
    <circle cx={x(row.column)} cy={mid} {r} fill={dot} fill-opacity={dotOpacity} />
  {/if}
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
