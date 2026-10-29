<!-- Tiny looping animations for "What's new": a sketch of the feature in action, drawn in HTML/CSS so it follows the
     theme and never goes stale like a screenshot. `name` comes from a change's "demo" in src/lib/changelog.json. -->
<script lang="ts">
  let { name }: { name: string } = $props();
</script>

<div class="demo" aria-hidden="true">
  {#if name === "search"}
    <!-- ⌘K: the query types itself, results from several repos drop in with their environments. -->
    <div class="bar"><span class="k">⌘K</span><span class="typed">MOA-854</span><span class="caret"></span></div>
    <div class="group g1">frontend</div>
    <div class="hit h1"><span class="sub">feat(<b>MOA-854</b>): sim filters</span><span class="env">✓ qa</span><span class="env">✓ main</span></div>
    <div class="group g2">moabits-monorepo</div>
    <div class="hit h2"><span class="sub">fix(<b>MOA-854</b>): export dates</span><span class="env">✓ qa</span></div>
  {:else if name === "copy-ai"}
    <!-- Right-click a commit → "Copy for AI" → copied, ready to paste into the agent. -->
    <div class="row"><span class="dot"></span><span class="sub">feat(connectivity-events): day picker</span></div>
    <div class="menu">
      <div class="item pick">✦ Copy for AI <span class="hint">repo · hash</span></div>
      <div class="item">▤ Copy for AI (full)</div>
      <div class="item">⧉ Copy hash</div>
    </div>
    <div class="toast">Copied ✓ <span class="hint">paste it into your agent</span></div>
  {:else if name === "focus"}
    <!-- Busy graph → "Only this branch": the other lines fade away, one line remains. -->
    <svg viewBox="0 0 220 100" class="graph">
      <path class="other" d="M30 0 V100" stroke="#4f8cff" />
      <path class="other" d="M30 70 C30 60 60 60 60 50 V10 C60 0 30 0 30 0" stroke="#34c77b" fill="none" />
      <path class="other" d="M30 90 C30 80 90 80 90 70 V30" stroke="#f5a524" fill="none" />
      <path class="mine" d="M120 0 V100" stroke="#ff3b8b" />
      {#each [15, 40, 65, 90] as y (y)}<circle class="mine" cx="120" cy={y} r="5" fill="#ff3b8b" />{/each}
      {#each [20, 50, 80] as y (y)}<circle class="other" cx="30" cy={y} r="4" fill="#4f8cff" />{/each}
      <circle class="other" cx="60" cy="30" r="4" fill="#34c77b" /><circle class="other" cx="90" cy="45" r="4" fill="#f5a524" />
    </svg>
    <div class="chip-focus">⌖ Only this branch</div>
  {:else if name === "files"}
    <!-- "Show changes only" off → the whole project shows up, changed files still marked. -->
    <div class="switchrow"><span class="sw"><span class="knob"></span></span>Show changes only</div>
    <div class="file changed">● api/sims.ts <span class="add">+12</span></div>
    <div class="more">
      <div class="file">○ api/users.ts</div>
      <div class="file">○ app/routes.ts</div>
    </div>
    <div class="file changed">● app/filters.ts <span class="add">+5</span></div>
  {:else if name === "details"}
    <!-- A commit's title, then "see more…" unfolds everything about it. -->
    <div class="title">feat: temporary day picker</div>
    <div class="see">see more…</div>
    <div class="unfold">
      <div class="meta"><span>Hash</span>423fbcfca3</div>
      <div class="meta"><span>Author</span>Joaquin · 2d ago</div>
      <div class="meta"><span>In</span><em>production</em><em>qa</em></div>
    </div>
  {/if}
</div>

<style>
  .demo {
    position: relative; height: 112px; margin-top: 8px; padding: 10px 12px; overflow: hidden; border-radius: 10px;
    font-size: 11px; background: color-mix(in oklab, var(--background) 70%, transparent); box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.14);
  }
  .demo * { animation-duration: 5s; animation-iteration-count: infinite; animation-timing-function: ease-out; }
  .hint { color: var(--tertiary); font-size: 10px; }
  .env { padding: 0 5px; border-radius: 99px; font-size: 9.5px; font-weight: 600; color: rgb(52 199 89); background: rgb(52 199 89 / 0.13); }
  .sub { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  /* search */
  .bar { display: flex; align-items: center; gap: 6px; height: 22px; padding: 0 8px; border-radius: 6px; background: oklch(0.5 0 0 / 0.1); }
  .k { color: var(--tertiary); font-size: 10px; }
  .typed { display: inline-block; overflow: hidden; white-space: nowrap; font-family: var(--font-mono, ui-monospace); width: 7ch; animation-name: type; animation-timing-function: steps(7, end); }
  .caret { width: 1px; height: 12px; background: var(--primary); animation-name: blink; animation-duration: 1s; }
  .group { margin: 6px 0 2px; font-size: 9px; font-weight: 700; color: var(--tertiary); text-transform: uppercase; }
  .hit { display: flex; align-items: center; gap: 5px; height: 18px; padding: 0 6px; border-radius: 5px; background: color-mix(in oklab, var(--primary) 12%, transparent); }
  .hit b { color: var(--primary); }
  .g1, .h1 { animation-name: show1; } .g2, .h2 { animation-name: show2; }
  @keyframes type { 0%, 8% { width: 0; } 35%, 100% { width: 7ch; } }
  @keyframes blink { 50% { opacity: 0; } }
  @keyframes show1 { 0%, 38% { opacity: 0; transform: translateY(4px); } 46%, 100% { opacity: 1; transform: none; } }
  @keyframes show2 { 0%, 48% { opacity: 0; transform: translateY(4px); } 56%, 100% { opacity: 1; transform: none; } }

  /* copy-ai */
  .row { display: flex; align-items: center; gap: 8px; height: 22px; padding: 0 6px; border-radius: 5px; background: oklch(0.5 0 0 / 0.1); }
  .dot { width: 8px; height: 8px; flex-shrink: 0; border-radius: 99px; background: var(--primary); }
  .menu {
    position: absolute; left: 70px; top: 30px; width: 170px; padding: 3px; border-radius: 7px; background: var(--popover);
    box-shadow: 0 0 0 1px oklch(0.5 0 0 / 0.2), 0 6px 16px rgb(0 0 0 / 0.3); transform-origin: top left; animation-name: menu;
  }
  .item { display: flex; gap: 6px; height: 18px; align-items: center; padding: 0 6px; border-radius: 4px; }
  .item .hint { margin-left: auto; }
  .pick { animation-name: pick; }
  .toast { position: absolute; left: 12px; bottom: 10px; padding: 3px 8px; border-radius: 99px; font-weight: 600; color: rgb(52 199 89); background: rgb(52 199 89 / 0.13); animation-name: toast; }
  @keyframes menu { 0%, 12% { opacity: 0; transform: scale(0.9); } 20%, 58% { opacity: 1; transform: none; } 64%, 100% { opacity: 0; transform: scale(0.96); } }
  @keyframes pick { 0%, 32% { background: transparent; } 40%, 58% { background: color-mix(in oklab, var(--primary) 22%, transparent); } }
  @keyframes toast { 0%, 60% { opacity: 0; transform: translateY(4px); } 66%, 92% { opacity: 1; transform: none; } 100% { opacity: 0; } }

  /* focus */
  .graph { position: absolute; inset: 6px 0 6px 20px; width: 220px; height: 100px; }
  .graph path { stroke-width: 2.5; fill: none; }
  .other { animation-name: fade-out; }
  .mine { animation-name: glow; }
  .chip-focus { position: absolute; right: 12px; top: 10px; padding: 2px 8px; border-radius: 6px; font-weight: 600; color: var(--tertiary); background: oklch(0.5 0 0 / 0.1); animation-name: chip-on; }
  @keyframes fade-out { 0%, 30% { opacity: 1; } 42%, 85% { opacity: 0.08; } 95%, 100% { opacity: 1; } }
  @keyframes glow { 0%, 30% { filter: none; } 42%, 85% { filter: drop-shadow(0 0 4px #ff3b8b); } 95% { filter: none; } }
  @keyframes chip-on { 0%, 25% { color: var(--tertiary); background: oklch(0.5 0 0 / 0.1); } 32%, 85% { color: var(--primary); background: color-mix(in oklab, var(--primary) 16%, transparent); } }

  /* files */
  .switchrow { display: flex; align-items: center; gap: 7px; margin-bottom: 5px; color: var(--muted-foreground); }
  .sw { position: relative; width: 22px; height: 12px; border-radius: 99px; animation-name: sw; }
  .knob { position: absolute; top: 1px; left: 1px; width: 10px; height: 10px; border-radius: 99px; background: white; animation-name: knob; }
  .file { height: 16px; color: var(--tertiary); }
  .file.changed { color: rgb(230 180 0 / 0.9); font-weight: 600; }
  .add { color: rgb(52 199 89); font-weight: 500; }
  .more { overflow: hidden; animation-name: grow; }
  @keyframes sw { 0%, 25% { background: var(--primary); } 32%, 80% { background: oklch(0.5 0 0 / 0.35); } 88%, 100% { background: var(--primary); } }
  @keyframes knob { 0%, 25% { transform: translateX(10px); } 32%, 80% { transform: none; } 88%, 100% { transform: translateX(10px); } }
  @keyframes grow { 0%, 28% { max-height: 0; opacity: 0; } 38%, 80% { max-height: 40px; opacity: 1; } 88%, 100% { max-height: 0; opacity: 0; } }

  /* details */
  .title { font-size: 12px; font-weight: 700; }
  .see { margin: 2px 0 4px; color: var(--tertiary); animation-name: see; }
  .unfold { overflow: hidden; animation-name: unfold; }
  .meta { display: flex; gap: 8px; height: 16px; align-items: center; }
  .meta span { width: 42px; color: var(--tertiary); }
  .meta em { font-style: normal; padding: 0 5px; border-radius: 99px; background: oklch(0.5 0 0 / 0.13); }
  @keyframes see { 0%, 22% { color: var(--tertiary); } 26%, 30% { color: var(--foreground); } 34%, 100% { opacity: 0; } }
  @keyframes unfold { 0%, 30% { max-height: 0; opacity: 0; } 42%, 88% { max-height: 60px; opacity: 1; } 96%, 100% { max-height: 0; opacity: 0; } }

  @media (prefers-reduced-motion: reduce) { .demo * { animation: none !important; } }
</style>
