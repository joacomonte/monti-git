# GitGraph → Tauri: estado de la migración

Fuente original (Swift, sigue intacta): `/Users/monti/.t3/scratch/2026-10-28-best-app-git-graph-i-51136e91/Sources/GitGraph/`.
Proyecto Tauri: `/Users/monti/.t3/scratch/2026-10-28-best-app-git-graph-i-51136e91/tauri/`.
ADR con arquitectura y reglas de producto: codebase-memory, proyecto `gitgraph-mac`.

## Decisiones (tomadas con Joaquín)
- Tauri 2 + Rust core + **Svelte 5 (SvelteKit SPA estático)** + TS. Elegido por "lo más eficiente".
- UI: **shadcn-svelte solo para lo básico** (button, switch, tabs, popover, tooltip, scroll-area); la estética propia
  (graph, badges, pills, árbol, diff) en **CSS scoped / Tailwind**. Portar la estética del Swift.
- Git: se sigue llamando al **binario `git`** (no gix/libgit2), igual que Swift.
- Entrega **por etapas**, Joaquín revisa al final de cada una.
- Rust instalado con rustup (`. "$HOME/.cargo/env"` antes de cargo).

## Cómo correr
- `cd /Users/monti/.t3/scratch/2026-10-28-best-app-git-graph-i-51136e91/tauri && . "$HOME/.cargo/env" && pnpm tauri dev` (log útil: redirigir a `/tmp/gg-tauri-dev.log`).
- Tests Rust: `cd …/tauri/src-tauri && cargo test` (3 tests: layout+names, runs, diff).
- Typecheck: `pnpm exec svelte-check --tsconfig ./tsconfig.json`.
- Captura: el proceso se llama `tauri`; `screencapture -x -o -l<windowId>` buscando owner `tauri` (ver `/tmp/gg-test/shotwin.sh`, que busca `GitGraph`).
- Sin mouse/Accessibility sin permiso: para probar selección se usó un hook temporal `$effect` en `+page.svelte` (ya borrado).

## Etapa 1 — HECHA
Rust (`src-tauri/src/`):
- `git.rs`: run (GIT_TERMINAL_PROMPT=0), log, worktree_commit, main_ref, main_chain, my_branches, changed_files,
  worktree_files, line_counts, all_files, file_diff, commits_touching.
- `graph.rs`: compute (lanes), linear_runs, branch_names (merge subjects sin regex), `to_wire`: lanes como
  `[target, color, flags]` (target = índice del commit; negativo = commit fuera de la página, mismo id por hash;
  flags 1 behind, 2 current) + `parents` con los mismos ids.
- `diff.rs`: side-by-side; `changed` en offsets UTF-16 (para `slice` en JS).
- `library.rs`: repos de `~/Documents/Dev` (+ extras), owner por remote, lastActivity.
- `lib.rs`: comandos `#[tauri::command(async)]`: list_repos, load_history, load_changes, load_diff.
Frontend (`src/`):
- `lib/api.ts` tipos + invoke. `lib/model.svelte.ts` = RepoModel (runes): open con caché por repo,
  reload, loadMore (+300), onlyMine, grupos (expanded), select, árbol (fileTree), filter/stepFile, detail.
  Prefs en localStorage con las mismas claves que UserDefaults (lastRepo, onlyMine, mainBranches, treeShowsAll, treeWidth, diffHeight).
- `lib/components/`: CommitList (virtualizado, alturas 62/26, ↑↓ →←, infinite load), GraphCell (SVG, glow drop-shadow,
  tooltips `<title>` por lane), RefBadge, FileTree, DiffPane (⌥←/⌥→), Sidebar, PaneDivider, Loader.
- `routes/+page.svelte`: layout sidebar 220px | main (top bar con Only mine, refresh bar, graph+tree arriba, diff abajo).
- Ventana: titleBarStyle Overlay + hiddenTitle, `data-tauri-drag-region` en sidebar/header (permiso `core:window:allow-start-dragging`).

## Etapa 2 — HECHA (dashboard)
- Rust `dashboard.rs`: port de `Dashboard.load` + `myActivity`, `branchSummaries`, `shelvedBranches`; cards en paralelo
  (thread::scope, N = núcleos). Comandos `load_dashboard` (prefs por repo vienen del front) y `branch_chain` (highlight).
- Front: `Dashboard.svelte` (Environments / Pinned / Recent activity / Hidden), `BranchCard.svelte`, `InfoButton.svelte` (?).
  Model: `dashboard`, `showGraph`, `highlightChain`, `dismissedUpdates`, `reveal`, `showInGraph` (loadMore hasta el tip),
  `revealWorktree`, `scrollRow` (CommitList lo consume una vez). GraphCell: prop `highlight` (índices) → width 5 / opacity 0.2.
- Switch Dashboard/Graph centrado (Tabs); al elegir otro repo en el sidebar vuelve a Dashboard.
- Pill "<parent> has N new commits": popover solo con "Hide this notice" (Merge/Rebase = etapa 3).
- Prefs se LEEN (pinnedBranches, environmentBranches, featureStates); se escriben desde el menú ⌄ de la etapa 3.
- Sin hacer: progreso paso a paso del loader (Swift mostraba "Comparing x · 3 of 12"; acá texto fijo),
  menú ⌄ por card, context menu "Copy Branch Name".

## Etapa 3 — HECHA (acciones)
- Rust `actions.rs`: port 1:1 de Git.swift: sync (+ diverged report), checkout con WipMode (shelve/commit/carry),
  pull, squash (commit-tree + update-ref con expected tip), update merge/rebase (worktree temporal si no es la actual),
  force push with lease, discard file/all/hunk. Comandos finos en `lib.rs`; `Dashboard.local` para "Change base branch".
  Verificado con un test temporal sobre repo scratch (update en worktree, shelve/WIP ida y vuelta, squash, discard) y borrado.
- Front: model `act()` (isSyncing + syncStatus), `checkout` → `pendingCheckout` si hay cambios, toggles de prefs
  (pin, environment, featureStates, mainBranches) que solo recargan el dashboard.
  Componentes: ActionRow, BranchActions (menú ⌄), UpdatePanel (anclado al header), SquashPanel (anclado a la línea,
  `untrack` del card al abrir), CheckoutPrompt (Enter = Stash, Esc), DiscardButton (2 pasos, 4 s), sync en el sidebar
  con popover de fallas (Copy log) y estado al pie.
- Sin hacer: "Save log…" (necesita plugin dialog), context menu "Copy Branch Name" (está en el ⌄), sync por fila del sidebar.

## Gotchas resueltos
- `shadcn-svelte init` queda en prompt interactivo de preset → `components.json` y `lib/utils.ts` escritos a mano; `add` funciona.
- Registry de shadcn-svelte usa `data-checked/data-open`; bits-ui 2.20.2 setea `data-state` → reemplazado por
  `data-[state=checked]` etc. en `components/ui/{switch,popover,tooltip,tabs}` (tabs: `data-active` → `data-[state=active]`,
  `data-horizontal` → `data-[orientation=horizontal]`). **Repetir al agregar componentes nuevos.**
- Tabs controlado desde el modelo: usar function binding `bind:value={() => …, (v) => …}`; `value=` + `onValueChange` no se actualizaba.
- `dark:` = `@media (prefers-color-scheme: dark)` (custom-variant en `app.css`), sigue al OS.
- Grid principal necesita `grid-rows-1`, si no el contenido estira la fila y el diff queda fuera de pantalla.
- `$lib` alias requiere que exista `src/lib` antes de `svelte-kit sync`.

## Pendiente
Sidebar (favoritos ya están): renombrar, drag para ordenar (repoOrder), grupos custom, main branch por repo, abrir carpeta
(plugin dialog, también para "Save log…"), scanFolder, sync por fila. Progreso paso a paso del loader.
Ancho de columna del graph ajustable. Probar en Windows (git en PATH, CRLF).
