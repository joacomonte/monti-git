<!-- Tiny looping animations for "What's new": a sketch of the feature in action, drawn in HTML/CSS so it follows the
     theme and never goes stale like a screenshot. `name` comes from a change's "demo" in src/lib/changelog.json.
     Every "new" change needs one: .github/scripts/release.mjs refuses to publish a release without it. -->
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
  {:else if name === "dashboard"}
    <!-- One row per branch: its columns fill in one after another. -->
    <div class="drow">
      <span class="age"><b>3</b>days</span>
      <span class="dname">fix/<b>paginado</b><i>✓ Nothing to pull</i></span>
      <span class="dcol c1"><em class="amber">↓ 3 behind</em><i>main</i></span>
      <span class="dcol c2"><em class="badge">✓ qa</em><em class="badge off">× prod</em></span>
      <span class="dcol c3"><em class="amber"><span class="beat"></span>4 files</em><i><span class="add">+120</span> <span class="del">−30</span></i></span>
    </div>
    <div class="drow ghost"><span class="age"><b>8</b>min</span><span class="dname"><b>main</b><i>✓ Nothing to pull</i></span></div>
  {:else if name === "hover-actions"}
    <!-- The pointer reaches a row: options, check out and "view in graph" slide in at its end. -->
    <div class="drow hovered">
      <span class="age"><b>3</b>days</span>
      <span class="dname">feat/<b>export-csv</b><i>✓ Nothing to pull</i></span>
      <span class="acts"><em class="circle">•••</em><em class="circle blue">⑂</em><em class="arrow">→</em></span>
      <span class="pointer"></span>
    </div>
    <div class="tip-label">Check out</div>
  {:else if name === "finished"}
    <!-- Every environment has it: the columns give way to "Finished" and the clean-up buttons. -->
    <div class="drow">
      <span class="age"><b>2</b>wk</span>
      <span class="dname">feat/<b>sim-filters</b><i>✓ Nothing to pull</i></span>
      <span class="swap">
        <span class="before"><em class="badge">✓ qa</em><em class="badge">✓ prod</em></span>
        <span class="after"><em class="okb">✓ Finished</em><em class="btn">Delete…</em><em class="btn quiet">Hide</em></span>
      </span>
    </div>
  {:else if name === "new-branch"}
    <!-- Kind, ticket and a few words: the branch name builds itself. -->
    <div class="dots"><span class="on"></span><span class="on"></span><span></span></div>
    <div class="kinds"><em class="kind pick">feat</em><em class="kind">fix</em><em class="kind">chore</em></div>
    <div class="bname"><span class="pre">feat/</span><span class="typed2">moa-900-export-csv</span><span class="caret"></span></div>
    <div class="go">Create branch →</div>
  {:else if name === "squash"}
    <!-- Three messy commits fold into one clean one. -->
    <div class="stack">
      <div class="c s1">wip</div>
      <div class="c s2">fix typo</div>
      <div class="c s3">fix again</div>
      <div class="c one">feat: export csv</div>
    </div>
    <div class="toast">Prompt copied ✓ <span class="hint">paste it into your agent</span></div>
  {:else if name === "pr"}
    <!-- Pick where it goes, copy the prompt. -->
    <div class="prline"><span class="pname">feat/export-csv</span>→<em class="target t-main">main</em><em class="target t-qa">qa</em></div>
    <div class="go copy">Copy prompt for AI</div>
    <div class="toast">Copied ✓ <span class="hint">your agent opens the PR</span></div>
  {:else if name === "wip"}
    <!-- Switching away: your unsaved work forks off as a dashed amber row, and comes back later. -->
    <svg viewBox="0 0 220 96" class="graph">
      <path d="M30 0 V96" stroke="#4f8cff" />
      <path class="fork" pathLength="1" d="M30 70 C30 52 70 52 70 38 V30" stroke="#f5b324" stroke-dasharray="0.06 0.04" />
      {#each [20, 70] as y (y)}<circle cx="30" cy={y} r="5" fill="#4f8cff" />{/each}
      <rect class="box" x="64" y="20" width="12" height="12" rx="3" fill="var(--background)" stroke="#f5b324" stroke-width="2" />
    </svg>
    <div class="wip-label"><b>WIP saved</b> from feat/export-csv<i>4 files · +120 −30 · comes back on checkout</i></div>
  {:else if name === "pill-menu"}
    <!-- Click a branch label in the graph: check it out or squash it. -->
    <div class="pill">⑂ feat/export-csv</div>
    <div class="menu small">
      <div class="item pick">⑂ Check out</div>
      <div class="item">▤ Squash commits…</div>
    </div>
  {:else if name === "notifications"}
    <!-- A system banner slides in when someone pushes to your branch. -->
    <div class="desk"></div>
    <div class="banner"><span class="appicon"></span><span><b>Monti Git · monorepo</b><i>2 new commits on the remote of feat/export-csv: someone pushed to it</i></span></div>
  {:else if name === "restart"}
    <!-- The "new version" toast: Restart, a short progress, and you're on it. -->
    <div class="utoast"><span class="spark">✦</span>Monti Git 0.3.0 is out.<em class="restart">Restart to update</em></div>
    <div class="progress"><span></span></div>
  {:else if name === "examples"}
    <!-- Hovering an option unfolds a real-life example under its description. -->
    <div class="opt">
      <b>Stash them</b><i>Put aside on the branch you're leaving.</i>
      <div class="ex">e.g. you're halfway through a fix and must review a teammate's branch…</div>
      <span class="pointer"></span>
    </div>
  {:else if name === "checkout-dialog"}
    <!-- Where you are → where you're going, then a clear choice. -->
    <div class="route"><em>feat/notifications</em>→<em class="to">fix/paginado</em></div>
    <div class="choice ch1"><span class="sq blue"></span>Stash them <em class="rec">Recommended</em></div>
    <div class="choice ch2"><span class="sq green"></span>Save as a WIP commit</div>
    <div class="choice ch3"><span class="sq amber"></span>Take them with me</div>
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


  /* dashboard rows (shared by dashboard, hover-actions, finished) */
  .drow { position: relative; display: flex; align-items: center; gap: 10px; height: 44px; padding: 0 8px; border-radius: 9px; background: oklch(0.5 0 0 / 0.08); }
  .drow.ghost { margin-top: 6px; opacity: 0.5; }
  .age { display: grid; place-items: center; width: 30px; height: 32px; flex-shrink: 0; border-radius: 7px; font-size: 8px; line-height: 1; color: var(--tertiary); background: oklch(0.5 0 0 / 0.12); }
  .age b { font-size: 12px; color: var(--foreground); }
  .dname { display: flex; flex-direction: column; min-width: 0; flex: 1; color: var(--muted-foreground); white-space: nowrap; }
  .dname b { color: var(--foreground); }
  .dname i, .dcol i { font-style: normal; font-size: 9.5px; color: rgb(52 199 89); }
  .dcol { display: flex; flex-direction: column; gap: 2px; font-size: 10px; }
  .dcol i { color: var(--tertiary); }
  .dcol.c2 { flex-direction: row; }
  em { font-style: normal; }
  .amber { display: flex; align-items: center; gap: 4px; font-weight: 600; color: #f5a524; }
  .badge { padding: 0 5px; border-radius: 99px; font-size: 9.5px; font-weight: 600; color: rgb(52 199 89); background: rgb(52 199 89 / 0.13); }
  .badge.off { color: var(--tertiary); background: oklch(0.5 0 0 / 0.12); }
  .beat { width: 6px; height: 6px; border-radius: 99px; background: #f5a524; animation-name: beat; animation-duration: 1.2s; }
  .del { color: rgb(255 99 99); }
  .c1 { animation-name: col1; } .c2 { animation-name: col2; } .c3 { animation-name: col3; }
  @keyframes col1 { 0%, 10% { opacity: 0; transform: translateY(4px); } 18%, 100% { opacity: 1; transform: none; } }
  @keyframes col2 { 0%, 22% { opacity: 0; transform: translateY(4px); } 30%, 100% { opacity: 1; transform: none; } }
  @keyframes col3 { 0%, 34% { opacity: 0; transform: translateY(4px); } 42%, 100% { opacity: 1; transform: none; } }
  @keyframes beat { 50% { transform: scale(1.5); opacity: 0.6; } }

  /* hover-actions */
  .hovered { animation-name: row-hover; }
  .acts { display: flex; align-items: center; gap: 5px; animation-name: acts; }
  .circle { display: grid; place-items: center; width: 22px; height: 22px; border-radius: 99px; font-size: 9px; color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.14); }
  .circle.blue { font-size: 12px; color: var(--primary); background: color-mix(in oklab, var(--primary) 18%, transparent); animation-name: press; }
  .arrow { color: var(--primary); }
  .pointer { position: absolute; width: 10px; height: 14px; background: white; clip-path: polygon(0 0, 0 100%, 30% 72%, 55% 100%, 70% 92%, 45% 66%, 85% 66%); filter: drop-shadow(0 1px 2px rgb(0 0 0 / 0.5)); animation-name: pointer; }
  .drow .pointer { left: 40%; top: 60%; }
  .tip-label { position: absolute; right: 30px; top: 62px; padding: 2px 7px; border-radius: 5px; font-size: 9.5px; color: var(--foreground); background: oklch(0.3 0 0); animation-name: tip; }
  @keyframes row-hover { 0%, 20% { box-shadow: none; } 28%, 90% { box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--primary) 55%, transparent); } }
  @keyframes acts { 0%, 22% { opacity: 0; transform: translateX(8px); } 32%, 90% { opacity: 1; transform: none; } 100% { opacity: 0; } }
  @keyframes pointer { 0% { transform: translate(-60px, 20px); opacity: 0; } 10% { opacity: 1; } 25% { transform: none; } 45%, 55% { transform: translate(var(--to, 150px), -6px); } 58% { transform: translate(var(--to, 150px), -6px) scale(0.85); } 62%, 90% { transform: translate(var(--to, 150px), -6px); opacity: 1; } 100% { opacity: 0; } }
  @keyframes press { 0%, 54% { color: var(--primary); background: color-mix(in oklab, var(--primary) 18%, transparent); } 58%, 85% { color: white; background: var(--primary); } }
  @keyframes tip { 0%, 48% { opacity: 0; } 54%, 85% { opacity: 1; } 92%, 100% { opacity: 0; } }

  /* finished */
  .swap { position: relative; width: 190px; height: 24px; }
  .swap > span { position: absolute; inset: 0; display: flex; align-items: center; gap: 5px; }
  .before { animation-name: swap-out; }
  .after { animation-name: swap-in; }
  .okb { font-weight: 700; color: rgb(52 199 89); }
  .btn { padding: 2px 7px; border-radius: 6px; font-size: 9.5px; font-weight: 600; color: var(--primary); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--primary) 45%, transparent); }
  .btn.quiet { color: var(--muted-foreground); box-shadow: none; background: oklch(0.5 0 0 / 0.12); }
  @keyframes swap-out { 0%, 30% { opacity: 1; transform: none; } 40%, 85% { opacity: 0; transform: translateY(-6px); } 95%, 100% { opacity: 1; transform: none; } }
  @keyframes swap-in { 0%, 34% { opacity: 0; transform: translateY(6px); } 44%, 85% { opacity: 1; transform: none; } 95%, 100% { opacity: 0; } }

  /* new-branch */
  .dots { display: flex; gap: 4px; margin-bottom: 6px; }
  .dots span { width: 6px; height: 6px; border-radius: 99px; background: oklch(0.5 0 0 / 0.3); }
  .dots .on { background: var(--primary); }
  .kinds { display: flex; gap: 5px; margin-bottom: 6px; }
  .kind { padding: 1px 8px; border-radius: 6px; background: oklch(0.5 0 0 / 0.12); }
  .kind.pick { animation-name: kind-pick; }
  .bname { display: flex; align-items: center; height: 22px; padding: 0 8px; border-radius: 6px; font-family: var(--font-mono, ui-monospace); background: oklch(0.5 0 0 / 0.1); }
  .pre { color: var(--tertiary); }
  .typed2 { display: inline-block; overflow: hidden; white-space: nowrap; width: 18ch; animation-name: type2; animation-timing-function: steps(18, end); }
  .go { position: absolute; right: 12px; bottom: 10px; padding: 3px 9px; border-radius: 7px; font-weight: 600; color: white; background: var(--primary); animation-name: go; }
  @keyframes kind-pick { 0%, 12% { color: inherit; background: oklch(0.5 0 0 / 0.12); } 18%, 100% { color: white; background: var(--primary); } }
  @keyframes type2 { 0%, 22% { width: 0; } 62%, 100% { width: 18ch; } }
  @keyframes go { 0%, 64% { opacity: 0.35; } 70%, 85% { opacity: 1; transform: scale(1); } 88% { transform: scale(0.94); } 92%, 100% { opacity: 1; transform: none; } }

  /* squash */
  .stack { position: relative; height: 66px; }
  .c { position: absolute; left: 0; right: 90px; height: 18px; padding: 0 8px; line-height: 18px; border-radius: 5px; background: oklch(0.5 0 0 / 0.12); }
  .s1 { top: 0; animation-name: fold1; } .s2 { top: 22px; animation-name: fold2; } .s3 { top: 44px; animation-name: fold3; }
  .one { top: 22px; font-weight: 700; color: white; background: #ec5fa8; animation-name: one; }
  @keyframes fold1 { 0%, 25% { transform: none; opacity: 1; } 40%, 90% { transform: translateY(22px); opacity: 0; } 100% { opacity: 1; } }
  @keyframes fold2 { 0%, 25% { opacity: 1; } 40%, 90% { opacity: 0; } 100% { opacity: 1; } }
  @keyframes fold3 { 0%, 25% { transform: none; opacity: 1; } 40%, 90% { transform: translateY(-22px); opacity: 0; } 100% { opacity: 1; } }
  @keyframes one { 0%, 36% { opacity: 0; transform: scale(0.9); } 46%, 90% { opacity: 1; transform: none; } 100% { opacity: 0; } }

  /* pr */
  .prline { display: flex; align-items: center; gap: 6px; height: 22px; }
  .pname { font-family: var(--font-mono, ui-monospace); color: var(--muted-foreground); }
  .target { padding: 1px 8px; border-radius: 99px; background: oklch(0.5 0 0 / 0.12); }
  .t-main { animation-name: kind-pick; }
  .go.copy { left: 12px; right: auto; bottom: auto; top: 40px; animation-name: go; }

  /* wip */
  .fork { animation-name: draw; stroke-width: 2.5; }
  .box { animation-name: box-in; }
  .wip-label { position: absolute; left: 110px; top: 22px; right: 10px; display: flex; flex-direction: column; gap: 2px; animation-name: box-in; }
  .wip-label b { color: #f5b324; }
  .wip-label i { font-style: normal; font-size: 9.5px; color: var(--tertiary); }
  @keyframes draw { 0%, 15% { opacity: 0; } 25%, 90% { opacity: 1; } 100% { opacity: 0; } }
  @keyframes box-in { 0%, 25% { opacity: 0; transform: scale(0.8); } 35%, 90% { opacity: 1; transform: none; } 100% { opacity: 0; } }

  /* pill-menu */
  .pill { display: inline-flex; padding: 2px 8px; border-radius: 99px; font-weight: 600; color: #ffb36b; background: rgb(255 179 107 / 0.15); box-shadow: inset 0 0 0 1px rgb(255 179 107 / 0.3); animation-name: pill-hover; }
  .menu.small { left: 24px; top: 38px; width: 150px; }
  @keyframes pill-hover { 0%, 15% { background: rgb(255 179 107 / 0.15); } 20%, 60% { background: rgb(255 179 107 / 0.3); } }

  /* notifications */
  .desk { position: absolute; inset: 0; background: linear-gradient(135deg, rgb(109 93 252 / 0.15), rgb(31 182 166 / 0.1)); }
  .banner { position: absolute; right: 10px; top: 12px; width: 250px; display: flex; gap: 8px; padding: 8px 10px; border-radius: 12px; background: color-mix(in oklab, var(--popover) 92%, transparent); box-shadow: 0 6px 18px rgb(0 0 0 / 0.35); animation-name: banner; }
  .banner span:last-child { display: flex; flex-direction: column; gap: 1px; }
  .banner i { font-style: normal; font-size: 10px; color: var(--muted-foreground); }
  .appicon { width: 22px; height: 22px; flex-shrink: 0; border-radius: 6px; background: linear-gradient(135deg, #6d5dfc, #1fb6a6); }
  @keyframes banner { 0%, 15% { opacity: 0; transform: translateX(40px); } 25%, 80% { opacity: 1; transform: none; } 90%, 100% { opacity: 0; transform: translateX(40px); } }

  /* restart */
  .utoast { display: flex; align-items: center; gap: 8px; padding: 7px 10px; border-radius: 10px; background: var(--popover); box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.2); }
  .spark { color: var(--primary); }
  .restart { margin-left: auto; padding: 2px 8px; border-radius: 6px; font-weight: 600; color: white; background: var(--primary); animation-name: go; }
  .progress { height: 4px; margin: 14px auto 0; width: 60%; overflow: hidden; border-radius: 99px; background: oklch(0.5 0 0 / 0.15); animation-name: show-late; }
  .progress span { display: block; height: 100%; background: var(--primary); animation-name: fill; }
  @keyframes show-late { 0%, 70% { opacity: 0; } 74%, 96% { opacity: 1; } 100% { opacity: 0; } }
  @keyframes fill { 0%, 72% { width: 0; } 96%, 100% { width: 100%; } }

  /* examples */
  .opt { position: relative; display: flex; flex-direction: column; gap: 2px; padding: 8px 10px; border-radius: 9px; animation-name: row-hover; }
  .opt i { font-style: normal; color: var(--muted-foreground); }
  .ex { overflow: hidden; margin-top: 4px; padding: 0 8px; border-radius: 6px; font-style: italic; color: var(--muted-foreground); background: oklch(0.5 0 0 / 0.1); animation-name: unfold-ex; }
  .opt .pointer { left: 60%; top: 30%; --to: 0px; }
  @keyframes unfold-ex { 0%, 28% { max-height: 0; padding-block: 0; opacity: 0; } 38%, 88% { max-height: 40px; padding-block: 4px; opacity: 1; } 96%, 100% { max-height: 0; opacity: 0; } }

  /* checkout-dialog */
  .route { display: flex; align-items: center; gap: 6px; margin-bottom: 6px; color: var(--tertiary); }
  .route em { padding: 1px 6px; border-radius: 5px; font-family: var(--font-mono, ui-monospace); font-size: 10px; background: oklch(0.5 0 0 / 0.12); }
  .route .to { color: var(--primary); background: color-mix(in oklab, var(--primary) 15%, transparent); }
  .choice { display: flex; align-items: center; gap: 7px; height: 20px; padding: 0 6px; margin-bottom: 3px; border-radius: 6px; box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.15); }
  .sq { width: 10px; height: 10px; border-radius: 3px; }
  .sq.blue { background: #4f8cff; } .sq.green { background: #34c77b; } .sq.amber { background: #f5a524; }
  .rec { padding: 0 5px; border-radius: 99px; font-size: 9px; color: var(--primary); background: color-mix(in oklab, var(--primary) 15%, transparent); }
  .ch1 { animation-name: col1, ch-pick; animation-duration: 5s, 5s; } .ch2 { animation-name: col2; } .ch3 { animation-name: col3; }
  @keyframes ch-pick { 0%, 55% { box-shadow: inset 0 0 0 1px oklch(0.5 0 0 / 0.15); } 62%, 90% { box-shadow: inset 0 0 0 1px #4f8cff; background: rgb(79 140 255 / 0.08); } }

  @media (prefers-reduced-motion: reduce) { .demo * { animation: none !important; } }
</style>
