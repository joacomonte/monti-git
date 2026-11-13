// App state, ported from RepoModel in ContentView.swift: the open repo's graph, the selected commit's files and diff,
// the repos in the sidebar, and the git actions. One class, because those three meet in every flow (an action reloads
// the graph, a reload re-finds the selection, opening a repo resets both).
import { api, WORKTREE_ID, type Card, type Commit, type Deletion, type DiffRow, type Discarded, type History, type HistoryQuery, type LineHits, type Pick, type RepoEntry, type RepoPicks, type CommitKind, type RewriteStep, type SyncResult, type WipMode } from "./api";
import { fileTree, filesInOrder, parentDirs } from "./fileTree";
import { gitCommand, lineName, ranBy } from "./format";
import { prefs, type Sources } from "./prefs";
import { localName } from "./refs";

const PAGE = 300;

export type BranchFilter = "pending" | "safe";

/** Puts things back: switch to the branch you were on, move a branch back (`expected` null: recreate it), bring
 *  discarded changes back, or put a dropped stash back in the list. */
type Undo =
  | { kind: "checkout"; branch: string }
  | { kind: "move"; branch: string; expected: string | null; target: string }
  | { kind: "restore"; discarded: NonNullable<Discarded> }
  | { kind: "stash"; id: string; message: string };

/** What an Undo does, for its button's tooltip. */
export function undoHint(undo: Undo) {
  switch (undo.kind) {
    case "checkout": return `Switch back to ${undo.branch}`;
    case "move": return undo.expected ? `Put ${undo.branch} back where it was` : `Recreate ${undo.branch} (local)`;
    case "restore": return "Bring the discarded changes back (only if those files haven't changed since)";
    case "stash": return "Put the stash back";
  }
}

/** What the app suggests right after an action: push the commit, or bring in the server's commits and push. */
export type Suggestion = { kind: "push" | "pullPush"; repo: string; branch: string };
/** One action done from the app, for the Logs panel. `started`: when its git commands began (they're grouped under it). */
export type Activity = { at: number; started?: number; repo: string; branch?: string; text: string; undo?: Undo; undone?: boolean }; // branch: checked out when it ran (Logs groups by it)

/** How a git action went: its one-line status, or why it failed. */
export type Outcome = { ok: true; status: string } | { ok: false; error: string };

const EMPTY: History = {
  commits: [], rows: [], names: [], hasMore: false, current: null, base: null, environments: [], allBranches: [], local: [],
  onServer: [], fastForwarded: false, conflict: null,
};

/** What a repo looked like when we left it, so coming back shows it at once while `reload()` refreshes it. */
type Snapshot = {
  history: History;
  limit: number;
  view: { showGraph: boolean; selectedId: string | null; fileFilter: string | null; diffOpen: boolean; focus: string[] | null; historyPath: string | null };
};

/** The file in the diff pane: its rows, and whether they're its whole content instead of its diff. */
type ShownDiff = { path: string; rows: DiffRow[]; truncated: boolean; whole: boolean };
/** A search result to land on: this line of this file, highlighted and blamed. */
type Jump = { path: string; line: number; old: boolean };

/** The folder a path sits in (either separator: Windows paths too). */
const parentOf = (path: string) => path.replace(/[\\/][^\\/]*$/, "");

export class RepoModel {
  repo = $state<string | null>(null);
  history = $state.raw<History>(EMPTY);
  error = $state<string | null>(null);
  isLoading = $state(false);
  onlyMine = $state(prefs.get("onlyMine", true)); // hide other people's branches; on by default
  branchesOnly = $state(prefs.get("branchesOnly", true)); // "Commits collapsed", on by default
  focus = $state.raw<string[] | null>(null); // "Only this branch": the branch and its base, the rest of the graph hidden
  historyPath = $state<string | null>(null); // a file's history: the graph shows only the commits that touched it
  showGraph = $state(true); // the graph is the app; the Dashboard tab is a placeholder for what comes next
  dashboardView = $state<"safe" | null>(null); // a view inside the dashboard: "Dashboard › Safe to delete" in the header
  scrollTo = $state.raw<{ id: string } | null>(null); // a row to bring into view, once (CommitList takes it)
  private limit = PAGE;
  private viewEnd = 0; // last row on screen: a refresh near the top only reloads the first page
  private growing = false; // the next load is infinite scroll asking for more: keep the bigger limit
  private cache = new Map<string, Snapshot>();

  /** Newest request of each kind: an older response arriving late is dropped. */
  private latest = new Map<string, number>();
  private ticket(kind: string) {
    const n = (this.latest.get(kind) ?? 0) + 1;
    this.latest.set(kind, n);
    return () => this.latest.get(kind) === n;
  }

  // column + 1: a tip whose lane ends on its own row (collapsed view) sits past both lane lists.
  graphLanes = $derived(this.history.rows.reduce((max, r) => Math.max(max, r.before.length, r.after.length, r.column + 1), 1));
  /** The line tooltip of each commit: its branch, or what the row is. */
  lineNames = $derived(this.history.commits.map((c, i) => lineName(c, this.history.names[i])));

  /** Something is already on screen and a reload is checking it for news: the thin bar at the top. */
  get isRefreshing() {
    return this.isLoading && this.history.commits.length > 0;
  }

  constructor() {
    const last = prefs.get("lastRepo", null);
    if (last && this.isListed(last)) this.open(last, "dashboard"); // the app opens on the dashboard
    if (this.hasSources) this.refreshRepos();
  }

  /** Opens `path`, on `view` if given; otherwise as it was left (the graph the first time). */
  open(path: string, view?: "graph" | "dashboard") {
    if (this.repo && this.repo !== path && !this.error) {
      const { showGraph, selectedId, fileFilter, diffOpen, focus, historyPath } = this;
      this.cache.set(this.repo, { history: this.history, limit: this.limit, view: { showGraph, selectedId, fileFilter, diffOpen, focus, historyPath } });
    }
    this.repo = path;
    prefs.set("lastRepo", path); // reopened on next launch
    this.error = null;
    this.filters = [];
    this.filterRefs = null;
    this.ticket("filters");
    this.isFiltering = false;
    this.syncStatus = "";
    this.syncFailure = null;
    this.pendingCheckout = null;
    // Cached: the same view as when we left, right away, while reload() refreshes it. Not cached: empty, so the
    // loader shows instead of the previous repo's content.
    this.markSeen(path);
    const cached = this.cache.get(path);
    this.history = cached?.history ?? EMPTY;
    this.limit = cached?.limit ?? PAGE;
    this.showGraph = view ? view === "graph" : (cached?.view.showGraph ?? true);
    this.focus = cached?.view.focus ?? null;
    this.historyPath = cached?.view.historyPath ?? null;
    this.selectedId = null;
    this.clearDetail();
    const selected = cached?.view.selectedId;
    if (selected && this.history.commits.some((c) => c.id === selected)) {
      this.selectedId = selected;
      this.fileFilter = cached.view.fileFilter;
      this.diffOpen = cached.view.diffOpen;
      this.scrollTo = { id: selected };
      this.loadTree();
      this.loadDetail();
    }
    this.reload();
  }

  private loading: { repo: string; done: Promise<void> } | null = null;
  private again = false;

  /** Reloads the graph. While one load runs for this repo, more calls (focus, sync, actions in a row) fold into one more
   *  load after it, so they don't stack git processes. */
  reload(): Promise<void> {
    const repo = this.repo;
    if (!repo) return Promise.resolve();
    if (this.loading?.repo === repo) {
      this.again = true;
      return this.loading.done;
    }
    const loading = { repo, done: Promise.resolve() };
    loading.done = (async () => {
      do {
        this.again = false;
        await this.loadOnce();
      } while (this.again && this.repo === repo);
      if (this.loading === loading) this.loading = null;
    })();
    this.loading = loading;
    return loading.done;
  }

  private async loadOnce() {
    const repo = this.repo;
    if (!repo) return;
    const fresh = this.ticket("history");
    this.isLoading = true;
    // Nothing past the first page on screen or selected: a refresh (focus, sync, action) needs only that page again.
    if (!this.growing && this.limit > PAGE && this.viewEnd < PAGE - 50 && this.selectedIndex < PAGE - 50) this.limit = PAGE;
    this.growing = false;
    const query: HistoryQuery = {
      limit: this.limit,
      onlyMine: this.onlyMine,
      branchesOnly: this.branchesOnly,
      preferred: this.picks(repo).preferred,
      focus: this.focus ?? [],
      path: this.historyPath,
      environments: this.picks(repo).environments,
      only: this.branchesOnly && !this.focus ? this.filterRefs : null,
    };
    try {
      const history = await api.loadHistory(repo, query);
      if (!fresh() || this.repo !== repo) return; // a newer load (or another repo) owns the state
      this.history = history;
      this.error = null;
    } catch (error) {
      if (!fresh() || this.repo !== repo) return;
      this.history = EMPTY;
      this.error = String(error);
    }
    this.isLoading = false;
    // The sidebar dot of this repo follows its uncommitted row (a commit or discard from the app cleans it).
    const entry = this.repos.find((r) => r.path === repo);
    // (Focus and file history may hide that row: only the full graph tells.)
    const fullGraph = !this.focus && !this.historyPath;
    if (fullGraph && entry && !!entry.changes !== this.history.commits.some((c) => c.id === WORKTREE_ID)) this.refreshRepos();
    // The selected commit is gone (e.g. the uncommitted row after a commit): nothing selected. Still there: a commit's
    // files never change, the uncommitted row's do.
    if (this.selectedId && !this.selected) this.select(null);
    else if (this.isWorktreeSelected) {
      this.loadTree();
      this.loadDetail();
    }
  }

  /** Infinite scroll: the list asks for more when its last rows come into view. */
  loadMore(): Promise<void> {
    if (!this.history.hasMore || this.isLoading) return Promise.resolve();
    this.limit += PAGE;
    this.growing = true;
    return this.reload(); // --topo-order output is prefix-stable, so rows above don't move
  }

  /** The last row the list shows: keeps the loaded pages it needs, and loads the next one near the end. */
  setViewEnd(row: number) {
    this.viewEnd = row;
    if (row >= this.history.commits.length - 30) this.loadMore();
  }

  /** CommitList brings the requested row into view, once. */
  takeScrollRequest() {
    const request = this.scrollTo;
    this.scrollTo = null;
    return request;
  }

  focusFrom = $state<"graph" | "dashboard">("graph"); // where the focus view was opened from: its Back goes there

  /** Graph shows only `branch` and its base (where it was cut from, else main); null = everything again. */
  async setFocus(branch: string | null, base: string | null = this.history.base, from: "graph" | "dashboard" = "graph") {
    if (branch) this.focusFrom = from;
    this.focus = branch ? [...new Set([branch, base].filter((r): r is string => r != null))] : null;
    this.limit = PAGE;
    await this.reload();
  }

  /** The focus view's Back: the whole graph, or the dashboard when it was opened from there. */
  async leaveFocus() {
    if (this.focusFrom === "dashboard") this.showGraph = false;
    await this.setFocus(null);
  }

  /** Graph shows only the commits that touched `path` (null = everything again), keeping the file open. */
  async setHistory(path: string | null) {
    this.historyPath = path;
    this.showGraph = true;
    this.limit = PAGE;
    await this.reload();
  }

  setBranchesOnly(on: boolean) {
    this.branchesOnly = on;
    prefs.set("branchesOnly", on);
    this.limit = PAGE;
    this.reload();
  }

  setOnlyMine(on: boolean) {
    this.onlyMine = on;
    prefs.set("onlyMine", on);
    return this.applyFilters();
  }

  // MARK: Branch filters (Commits collapsed): the graph keeps only the matching branches, plus the base and environments.

  filters = $state.raw<BranchFilter[]>([]);
  isFiltering = $state(false); // confirming squash / cherry-pick merges in the background
  private filterRefs: string[] | null = null; // the branches they matched, null = no filter

  setFilters(filters: BranchFilter[]) {
    this.filters = filters;
    this.applyFilters();
  }

  /** Where work ends up: the base, then the environments. What "merged", "pending" and "safe to delete" compare to. */
  get targets(): string[] {
    return [...(this.history.base ? [this.history.base] : []), ...this.history.environments];
  }

  /** Fast pass from history (one for-each-ref per target), then the branches it couldn't place are confirmed with
   *  the card check (squash / cherry-pick; cached in Rust), reloading only if the result changed. */
  private async applyFilters() {
    const repo = this.repo, base = this.history.base, fresh = this.ticket("filters");
    this.isFiltering = false;
    if (!repo || !this.filters.length) {
      this.filterRefs = null;
      return this.reload();
    }
    const targets = this.targets;
    const compared = new Set(targets.map(localName));
    const wants = (done: number) =>
      (this.filters.includes("safe") && done === targets.length) || (this.filters.includes("pending") && done > 0 && done < targets.length);
    const { all, merged } = await api.mergedInto(repo, targets, this.onlyMine);
    if (!fresh()) return;
    const sets = merged.map(([, refs]) => new Set(refs));
    const branches = all.filter((ref) => !compared.has(localName(ref)));
    const doneOf = (ref: string) => sets.filter((s) => s.has(ref)).length;
    this.filterRefs = branches.filter((ref) => wants(doneOf(ref)));
    await this.reload();
    const unsure = branches.filter((ref) => doneOf(ref) < targets.length);
    if (!base || !unsure.length || !fresh()) return;
    this.isFiltering = true;
    try {
      const found = await api.branchStatus(repo, unsure, base, targets);
      if (!fresh()) return;
      const sure = branches.filter((ref) => doneOf(ref) === targets.length && wants(targets.length));
      const confirmed = found.filter((s) => wants(s.promotion.filter(([, missing]) => missing === 0).length)).map((s) => s.ref);
      const next = [...new Set([...sure, ...confirmed])];
      if (next.join() !== this.filterRefs?.join()) {
        this.filterRefs = next;
        await this.reload();
      }
    } catch (error) {
      if (fresh()) this.syncStatus = `Couldn't check squash merges: ${firstLine(error)}`;
    } finally {
      if (fresh()) this.isFiltering = false;
    }
  }

  // MARK: Selection: the file tree and the diff

  selectedId = $state<string | null>(null);
  selected = $derived(this.selectedId ? (this.history.commits.find((c) => c.id === this.selectedId) ?? null) : null);
  selectedIndex = $derived(this.selected ? this.history.commits.indexOf(this.selected) : -1);
  get isWorktreeSelected() {
    return this.selectedId === WORKTREE_ID;
  }

  changes = $state.raw<Record<string, string>>({}); // path -> status letter
  counts = $state.raw<Record<string, [number, number]>>({});
  staged = $state.raw(new Set<string>()); // uncommitted-changes row: files in the index (checked in the tree)
  untracked = $state.raw(new Set<string>()); // uncommitted-changes row: new files git doesn't follow (their own section)
  nested = $state.raw(new Set<string>()); // untracked folders that are a git repo of their own
  /** Untracked files listed apart, under the changes git follows: on the uncommitted row, listing only the changes. */
  get untrackedApart() {
    return this.isWorktreeSelected && !this.showAllFiles && this.untracked.size > 0;
  }
  trackedChanges = $derived(Object.keys(this.changes).filter((p) => !this.untracked.has(p)));
  /** Changes already staged once on their own (repo + path), while they stay changed: unchecking one sticks. */
  private autoStaged = new Set<string>();
  /** Untracked paths waiting for the delete dialog: deleting them can't be undone, so it's the only way they go. */
  doomed = $state<string[] | null>(null);
  paths = $state.raw<string[]>([]); // every file the tree lists
  tree = $derived(fileTree(this.paths));
  changedOrder = $derived(filesInOrder(this.tree).filter((p) => p in this.changes)); // changed files in tree order, for ⌥←/⌥→
  changedDirs = $derived(new Set(Object.keys(this.changes).flatMap(parentDirs))); // folders holding a change: highlighted
  // Folder paths; the untracked section's are "untracked:"-prefixed, so each section opens its own "src".
  expandedDirs = $state.raw(new Set<string>());
  showAllFiles = $state(prefs.get("treeShowsAll", false));
  private treeFor: string | null = null; // the selection the folders were opened for: a refresh keeps the user's collapses

  fileFilter = $state<string | null>(null); // file shown in the diff pane
  diffOpen = $state(false); // diff pane under the graph: opens on the first file click in the tree
  diff = $state.raw<ShownDiff | null>(null);
  diffMessage = $state("Select a file");
  jump = $state.raw<Jump | null>(null);

  /** Staged files under `path` ("" = all), and whether all / some of them are: a tree checkbox. */
  stageState = $derived.by(() => {
    const byDir = new Map<string, { files: string[]; staged: number }>();
    for (const file of Object.keys(this.changes)) {
      for (const dir of ["", ...parentDirs(file), file]) {
        const entry = byDir.get(dir) ?? { files: [], staged: 0 };
        entry.files.push(file);
        if (this.staged.has(file)) entry.staged++;
        byDir.set(dir, entry);
      }
    }
    return (path: string) => {
      const { files, staged } = byDir.get(path) ?? { files: [], staged: 0 };
      return { files, all: files.length > 0 && staged === files.length, some: staged > 0 };
    };
  });

  /** Opens the file tree only; the diff pane waits until a file is picked there. Null hides the panes. */
  select(id: string | null) {
    if (id === this.selectedId) return;
    this.selectedId = id;
    this.diffOpen = false;
    this.clearDetail();
    if (!id) return;
    this.diffMessage = "Loading…";
    if (this.historyPath) {
      // A file's history: each commit opens straight on that file's diff.
      this.fileFilter = this.historyPath;
      this.diffOpen = true;
      this.loadDetail();
    }
    this.loadTree();
  }

  private clearDetail() {
    this.ticket("tree");
    this.ticket("diff");
    this.fileFilter = null;
    this.jump = null;
    this.diff = null;
    this.changes = {};
    this.counts = {};
    this.staged = new Set();
    this.untracked = new Set();
    this.nested = new Set();
    this.paths = [];
  }

  /** The selection for the backend: its commit, or null for the uncommitted row. Never `commit.parents`: those are
   *  the graph's links, which a simplified view rewrites. */
  private target(commit: Commit) {
    return commit.id === WORKTREE_ID ? null : commit.id;
  }

  /** Collapsed view: a row is its whole branch, so the files are everything since it left the base (Rust falls back to
   *  the one commit when it's already in the base). */
  private spanBase() {
    return this.branchesOnly && this.focus == null && this.historyPath == null ? this.history.base : null;
  }

  /** Tree search by content, on the selection: diff lines, or whole files (only the changed ones unless the tree shows all). */
  async searchLines(query: string, diffOnly: boolean): Promise<Record<string, LineHits>> {
    if (!this.repo || !this.selected) return {};
    const paths = this.showAllFiles ? [] : Object.keys(this.changes).filter((p) => this.changes[p] !== "D");
    if (!this.showAllFiles && !paths.length) return {};
    return api.searchLines(this.repo, this.target(this.selected), this.spanBase(), query, paths, diffOnly);
  }

  setShowAllFiles(all: boolean) {
    this.showAllFiles = all;
    prefs.set("treeShowsAll", all);
    this.loadTree();
  }

  private async loadTree() {
    const repo = this.repo, commit = this.selected, all = this.showAllFiles, fresh = this.ticket("tree");
    if (!repo || !commit) return;
    try {
      const result = await api.loadChanges(repo, this.target(commit), this.spanBase(), all);
      if (!fresh()) return; // the user moved on while git was running
      this.changes = result.changes;
      this.counts = result.counts;
      this.staged = new Set(result.staged);
      this.untracked = new Set(result.untracked);
      this.nested = new Set(result.nested);
      this.paths = result.paths;
      // Every change starts staged, new files too: the next commit takes them without checking each one. Once per
      // change, so unchecking one sticks until it's committed or discarded. Not a repo inside this one (adding it
      // would commit a link to it, not its files), nor a conflicted file (staging it would mark it resolved).
      if (commit.id === WORKTREE_ID) {
        const key = (p: string) => repo + "\0" + p;
        for (const k of this.autoStaged) if (k.startsWith(repo + "\0") && !(k.slice(repo.length + 1) in result.changes)) this.autoStaged.delete(k);
        const unstaged = Object.keys(result.changes).filter((p) => !this.staged.has(p) && !this.nested.has(p) && result.changes[p] !== "U" && !this.autoStaged.has(key(p)));
        if (unstaged.length) {
          unstaged.forEach((p) => this.autoStaged.add(key(p)));
          return this.setStaged(unstaged, true); // reloads the tree
        }
      }
    } catch (error) {
      if (fresh()) this.diffMessage = `Couldn't read the files: ${firstLine(error)}`;
      return;
    }
    if (this.treeFor !== commit.id) {
      this.treeFor = commit.id;
      this.openDirs(this.changedDirs); // a new selection opens the folders with changes
    }
    if (this.diffOpen && !this.jump && (!this.fileFilter || !(this.fileFilter in this.changes))) {
      this.fileFilter = this.changedOrder[0] ?? null; // new selection: start at the first changed file
      this.loadDetail();
    }
    if (!this.changedOrder.length) this.diffMessage = "No changes";
  }

  /** Shows `path` in the diff pane (a folder: its first changed file). With `at`, lands on that line; without, clicking
   *  the file already shown hides/shows the pane. */
  showFile(path: string, at?: { line: number; old: boolean }) {
    this.jump = at ? { path, ...at } : null;
    if (this.fileFilter === path && !at) {
      this.diffOpen = !this.diffOpen;
      return;
    }
    this.diffOpen = true;
    if (this.fileFilter === path && this.diff?.path === path) return this.landOnJump();
    this.fileFilter = path in this.changes ? path : (this.changedOrder.find((p) => p.startsWith(path + "/")) ?? path);
    this.loadDetail();
  }

  /** Previous/next changed file, opening its folders in the tree so it stays visible. */
  stepFile(offset: number) {
    const index = this.fileFilter ? this.changedOrder.indexOf(this.fileFilter) : -1;
    const path = this.changedOrder[index + offset];
    if (index < 0 || !path) return;
    const section = this.untrackedApart && this.untracked.has(path) ? "untracked:" : "";
    this.expandedDirs = new Set([...this.expandedDirs, ...parentDirs(path).map((dir) => section + dir)]);
    this.showFile(path);
  }

  /** Only these folders open, in both sections. */
  private openDirs(dirs: Iterable<string>) {
    this.expandedDirs = new Set([...dirs].flatMap((dir) => [dir, "untracked:" + dir]));
  }

  /** Every folder the tree lists: Changed lists only changed files, so that's the folders with changes; All files, all. */
  expandAll() {
    this.openDirs(new Set(this.paths.flatMap(parentDirs)));
  }

  toggleDir(path: string) {
    const dirs = new Set(this.expandedDirs);
    if (!dirs.delete(path)) dirs.add(path);
    this.expandedDirs = dirs;
  }

  /** `whole`: the file's full content instead of its diff (it didn't change here, or a searched line is outside the
   *  changed blocks). */
  private async loadDetail(whole = false) {
    const repo = this.repo, commit = this.selected, path = this.fileFilter, fresh = this.ticket("diff");
    if (!repo || !commit || !path) return;
    whole ||= !(path in this.changes) && Object.keys(this.changes).length > 0;
    try {
      const result = await api.loadDiff(repo, path, this.target(commit), this.spanBase(), whole);
      if (!fresh()) return;
      this.diff = { path, ...result, whole };
      this.diffMessage = whole ? "Empty or binary file" : "Binary file or no text changes";
    } catch (error) {
      if (fresh()) {
        this.diff = null;
        this.diffMessage = firstLine(error);
      }
      return;
    }
    this.landOnJump();
  }

  /** A searched line outside the changed blocks: the whole file instead, where every line is. */
  private landOnJump() {
    const jump = this.jump, diff = this.diff;
    if (!jump || !diff || diff.path !== jump.path || diff.whole || jump.old) return;
    if (!diff.rows.some((row) => row.right?.number === jump.line && row.hunk == null)) this.loadDetail(true);
  }

  // MARK: Branches (come with every history load)

  /** A remote-only base ("origin/main") counts as current when its local twin is checked out. */
  isCurrent(name: string) {
    return this.history.current != null && localName(name) === this.history.current;
  }

  /** What the user picked for `repo`: its base and environments (null = detected by name). */
  picks(repo: string): RepoPicks {
    return { path: repo, preferred: prefs.get("mainBranches", {})[repo] ?? null, environments: prefs.get("environments", {})[repo] ?? null };
  }

  private picksVersion = $state(0); // bumped when the picks change: prefs aren't reactive

  /** A repo opened for the first time: neither its base nor its environments picked yet (the setup screen asks). */
  get needsSetup() {
    void this.picksVersion;
    const repo = this.repo;
    return !!repo && this.history.allBranches.length > 0 && !(repo in prefs.get("mainBranches", {})) && !(repo in prefs.get("environments", {}));
  }

  /** The base and the environments in one go, by local name ("production", even when only origin has it: the base
   *  resolves to the local branch, else its copy on the server). The base isn't an environment of its own. */
  configure(base: string, environments: string[]) {
    const repo = this.repo;
    if (!repo) return;
    const name = localName(base);
    prefs.set("mainBranches", { ...prefs.get("mainBranches", {}), [repo]: name });
    prefs.set("environments", { ...prefs.get("environments", {}), [repo]: [...new Set(environments.map(localName))].filter((n) => n !== name) });
    this.picksVersion++;
    this.reload();
  }

  /** `name` becomes the base; the old base stays as an environment. */
  makeBase(name: string) {
    const old = this.history.base;
    this.configure(name, [...this.history.environments, ...(old ? [old] : [])]);
  }

  /** An older commit may sit past the loaded page: keep loading history until it shows up (or there is no more). */
  private async loadUntil(id: string) {
    const repo = this.repo;
    const has = () => this.history.commits.some((c) => c.id === id);
    await this.reload();
    for (let pages = 0; this.repo === repo && pages < 30 && !has() && this.history.hasMore; pages++) await this.loadMore();
    return has();
  }

  /** Global search result: opens its repo on the graph with that commit selected. A commit hidden by
   *  "My branches only" (someone else's branch) turns that filter off, so it can be shown. */
  async revealCommit(repo: string, id: string) {
    if (this.repo !== repo) this.open(repo);
    this.showGraph = true;
    if (this.focus) await this.setFocus(null);
    if (this.historyPath) await this.setHistory(null);
    if (!(await this.loadUntil(id)) && this.onlyMine && this.repo === repo) {
      await this.setOnlyMine(false);
      await this.loadUntil(id);
    }
    if (this.repo !== repo || !this.history.commits.some((c) => c.id === id)) return false;
    this.select(id);
    this.scrollTo = { id };
    return true;
  }

  /** Global code search result: opens the commit it was found at, on that file, landing on the line. */
  async revealLine(repo: string, id: string, path: string, line: number) {
    if (await this.revealCommit(repo, id)) this.showFile(path, { line, old: false });
  }

  // MARK: Repo sources: what the sidebar lists, added by the user (nothing is scanned by default)

  sources = $state<Sources>((({ folders, repos, hidden = [] }) => ({ folders, repos, hidden }))(prefs.get("repoSources", { folders: [], repos: [], hidden: [] })));
  onboarding = $state(false); // keeps the onboarding up after the first repos are added, for its favorites step
  favorites = $state<string[]>(prefs.get("sidebarPrefs", {}).favorites ?? []); // same blob as SidebarPrefs in RepoSidebar.swift

  repos = $state.raw<RepoEntry[]>([]); // what the sidebar lists, refreshed when the sources change and on coming back to the app
  private seenChanges = $state(prefs.get("seenChanges", {}));

  /** Relists the repos (and whether each has uncommitted changes). The open repo's changes count as seen. */
  async refreshRepos() {
    const fresh = this.ticket("repos");
    const list = await api.listRepos(this.sources.folders, this.sources.repos, this.sources.hidden);
    if (!fresh()) return this.repos;
    this.repos = list;
    if (this.repo) this.markSeen(this.repo);
    // A repo back to clean forgets what was seen: its next changes are new.
    const dirty = new Set(list.filter((r) => r.changes).map((r) => r.path));
    const kept = Object.fromEntries(Object.entries(this.seenChanges).filter(([path]) => dirty.has(path)));
    if (Object.keys(kept).length !== Object.keys(this.seenChanges).length) this.saveSeen(kept);
    return list;
  }

  /** The sidebar dot: "new" uncommitted changes (pulsing) until the repo is opened, then "seen"; null when clean. */
  changesDot(entry: RepoEntry): "new" | "seen" | null {
    if (!entry.changes) return null;
    return this.seenChanges[entry.path] === entry.changes ? "seen" : "new";
  }

  private markSeen(path: string) {
    const changes = this.repos.find((r) => r.path === path)?.changes;
    if (changes && this.seenChanges[path] !== changes) this.saveSeen({ ...this.seenChanges, [path]: changes });
  }

  private saveSeen(seen: Record<string, string>) {
    this.seenChanges = seen;
    prefs.set("seenChanges", seen);
  }

  get hasSources() {
    return this.sources.folders.length + this.sources.repos.length > 0;
  }

  /** Still in the sidebar: added on its own, or inside an added folder and not hidden from it. */
  private isListed(path: string) {
    return this.sources.repos.includes(path) || (this.inFolder(path) && !this.sources.hidden.includes(path));
  }

  private inFolder(path: string) {
    return this.sources.folders.includes(parentOf(path));
  }

  /** Repos taken out of an added folder, for the sidebar's Hidden list. */
  get hiddenRepos() {
    return this.sources.hidden.filter((path) => this.inFolder(path));
  }

  toggleFavorite(path: string) {
    this.favorites = this.favorites.includes(path) ? this.favorites.filter((p) => p !== path) : [...this.favorites, path];
    prefs.set("sidebarPrefs", { ...prefs.get("sidebarPrefs", {}), favorites: this.favorites });
  }

  private saveSources(next: Sources) {
    this.sources = next;
    prefs.set("repoSources", next);
    this.refreshRepos();
  }

  /** "repo": each path must be a git repo. "folder": every repo directly inside it, rescanned on refresh. */
  async addRepos(kind: "repo" | "folder", paths: string[]) {
    if (!paths.length) return;
    const s = this.sources;
    if (kind === "folder") return this.saveSources({ ...s, folders: [...new Set([...s.folders, ...paths])] });
    const checks = await Promise.all(paths.map((p) => api.isRepo(p)));
    const repos = paths.filter((_, i) => checks[i]);
    const rejected = paths.filter((_, i) => !checks[i]);
    if (rejected.length) this.syncStatus = `Not a git repository: ${rejected.map((p) => p.split(/[\\/]/).pop()).join(", ")}. Use "Add folder of repos" for a folder that contains several.`;
    if (!repos.length) return;
    this.saveSources({ ...s, repos: [...new Set([...s.repos, ...repos])], hidden: s.hidden.filter((h) => !repos.includes(h)) });
    if (!this.repo) this.open(repos[0]);
  }

  /** Out of the sidebar, wherever it came from: added on its own, or found in an added folder (it stays hidden from
   *  that folder until it's shown again or re-added). */
  removeRepo(path: string) {
    const s = this.sources;
    this.saveSources({ ...s, repos: s.repos.filter((r) => r !== path), hidden: this.inFolder(path) ? [...new Set([...s.hidden, path])] : s.hidden });
  }

  unhideRepo(path: string) {
    this.saveSources({ ...this.sources, hidden: this.sources.hidden.filter((h) => h !== path) });
  }

  /** Its hidden repos go with it: re-adding the folder lists them all again. */
  removeFolder(folder: string) {
    const s = this.sources;
    this.saveSources({ ...s, folders: s.folders.filter((f) => f !== folder), hidden: s.hidden.filter((h) => parentOf(h) !== folder) });
  }

  // MARK: Actions (src-tauri/src/actions.rs): one at a time from the UI; Rust also serializes them per repo.

  isSyncing = $state(false); // a user action is running: sync, checkout, pull, commit…
  isAutoSyncing = $state(false); // the quiet background sync
  syncStatus = $state(""); // last action's one-line result, at the bottom of the sidebar
  syncFailure = $state<{ title: string; reasons: SyncResult["skipped"]; log: string } | null>(null); // stays until dismissed
  pendingCheckout = $state<{ name: string; changes: number; undoing: Activity | null } | null>(null); // dirty tree: a dialog asks what to do
  /** The action running (or just finished), for the bottom toast (ActionToast): what it is, what it does, how it went. */
  /** `logged`: it went into Logs (record), with the git commands that changed the repo: the toast shows them as a terminal
   *  and flies into the Logs button, so it's clear where to see and undo it. */
  toast = $state<{ id: number; title: string; details: string[]; state: "busy" | "done" | "failed"; result: string; started: number; logged?: { commands: string[]; undo: boolean } } | null>(null);

  /** Runs one git action with the busy flag, the status line and the toast, then reloads. `work` returns the status. */
  private async act(busy: string, failed: string, details: string[], work: (repo: string) => Promise<string>): Promise<Outcome & { started: number }> {
    const repo = this.repo, started = Date.now();
    if (!repo) return { ok: false, error: "No repository open", started };
    if (this.isSyncing) {
      this.syncStatus = "Another git action is still running: try again when it's done";
      return { ok: false, error: this.syncStatus, started };
    }
    this.isSyncing = true;
    this.syncStatus = busy;
    const toast = { id: started, title: busy, details, state: "busy" as const, result: "", started };
    this.toast = toast;
    let outcome: Outcome;
    try {
      outcome = { ok: true, status: await work(repo) };
      this.syncStatus = outcome.status;
    } catch (error) {
      outcome = { ok: false, error: String(error) };
      this.syncStatus = `${failed}: ${firstLine(error)}`;
    }
    this.isSyncing = false;
    if (this.toast?.id === toast.id) {
      this.toast = { ...toast, state: outcome.ok ? "done" : "failed", result: this.syncStatus };
      setTimeout(() => { if (this.toast?.id === toast.id) this.toast = null; }, outcome.ok ? 4200 : 9000);
    }
    if (this.repo === repo) await this.reload(); // callers read the new state (a conflict, the new tip) right after
    return { ...outcome, started };
  }

  // MARK: Activity: the relevant git actions done from the app, newest first, some with an Undo.

  activity = $state<Activity[]>(prefs.get("activity", []));

  private record(repo: string, text: string, started: number, undo?: Undo) {
    const entry: Activity = { at: Date.now(), started, repo, branch: (repo === this.repo && this.history.current) || undefined, text, undo };
    this.activity = [entry, ...this.activity].slice(0, 50);
    prefs.set("activity", this.activity);
    api.gitLogs(0).then((logs) => {
      if (this.toast?.id !== started) return;
      const commands = logs.filter((l) => l.write && ranBy(l, entry)).map(gitCommand);
      this.toast = { ...this.toast, logged: { commands: commands.length ? commands : [text], undo: !!undo } };
    }, () => {});
  }

  /** An Undo that moves `branch` back to `before`, if the action moved it. */
  private async moveBack(repo: string, branch: string, before: string | null): Promise<Undo | undefined> {
    const after = await api.branchTip(repo, branch);
    return before && after && before !== after ? { kind: "move", branch, expected: after, target: before } : undefined;
  }

  private markUndone(entry: Activity) {
    this.activity = this.activity.map((e) => (e === entry ? { ...e, undone: true } : e));
    prefs.set("activity", this.activity);
  }

  /** Checkout goes back through `checkout` (it asks about uncommitted changes); a moved branch goes back only if it
   *  hasn't moved since and isn't pushed; discarded changes only if those files haven't changed since (Rust checks). */
  async undo(entry: Activity) {
    const undo = entry.undo;
    if (!undo || entry.undone || entry.repo !== this.repo) return;
    if (undo.kind === "checkout") return this.checkout(undo.branch, entry);
    const [detail, work] =
      undo.kind === "move" ? [`Moving ${undo.branch} back to ${undo.target.slice(0, 7)}, only if it hasn't moved since and isn't on the remote`, (repo: string) => api.moveBranch(repo, undo.branch, undo.expected, undo.target)]
      : undo.kind === "restore" ? ["Bringing the discarded changes back, only if those files haven't changed since", (repo: string) => api.restoreDiscarded(repo, undo.discarded)]
      : ["Putting the stash back in the list", (repo: string) => api.stashRestore(repo, undo.id, undo.message)];
    const result = await this.act("Undoing…", "Undo failed", [detail], async (repo) => (await work(repo), `Undone: ${entry.text}`));
    if (result.ok) this.markUndone(entry);
  }

  /** Fetch --all --prune + safe fast-forwards of every branch that's behind. Skipped branches open the failure popover. */
  async sync() {
    this.syncFailure = null;
    const repo = this.repo;
    const result = await this.act("Syncing…", "Sync failed", ["Fetching every remote", "Fast-forwarding branches that are behind", "Skipping anything that isn't a clean fast-forward"],
      async (repo) => {
        let result: SyncResult;
        try {
          result = await api.sync(repo);
        } catch (error) {
          this.syncFailure = { title: "Sync failed", reasons: [], log: String(error) };
          throw error;
        }
        this.lastSynced = Date.now();
        if (result.skipped.length) this.syncFailure = { title: "Some branches weren't updated", reasons: result.skipped, log: result.log.join("\n\n") };
        const time = new Date().toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
        const parts = [`Synced ${time}`];
        if (result.updated.length) parts.push("updated: " + result.updated.join(", "));
        if (result.skipped.length) parts.push("skipped: " + result.skipped.map((s) => s.branch).join(", "));
        if (!result.updated.length && !result.skipped.length) parts.push("all branches up to date");
        return parts.join(" · ");
      });
    if (result.ok) this.record(repo!, result.status, result.started);
  }

  /** Quiet background sync (every few minutes and on focus): same safe fast-forwards as `sync`, without the status
   *  line or the failure popover; what couldn't be updated shows on its card instead. */
  private lastAutoSync = 0;
  autoSyncOn = $state(prefs.get("autoSync", true));
  lastSynced = $state<number | null>(null); // ms; any successful sync, manual or automatic

  setAutoSync(on: boolean) {
    this.autoSyncOn = on;
    prefs.set("autoSync", on);
    if (on) this.autoSync();
  }

  async autoSync() {
    const repo = this.repo;
    if (!repo || !this.autoSyncOn || this.isSyncing || this.isAutoSyncing || Date.now() - this.lastAutoSync < 60_000) return;
    this.lastAutoSync = Date.now();
    this.isAutoSyncing = true;
    const result = await api.sync(repo).catch(() => null);
    this.isAutoSyncing = false;
    if (result) this.lastSynced = Date.now();
    if (result?.changed && this.repo === repo) this.reload(); // nothing new from origin: focus already reloaded
  }

  /** With uncommitted changes, asks first (stash / WIP commit / move them) via `pendingCheckout`. `undoing`: the
   *  Activity entry this checkout undoes, marked once it's done. */
  async checkout(name: string, undoing: Activity | null = null) {
    const repo = this.repo;
    if (!repo || this.isSyncing) return;
    if (this.isCurrent(name)) return this.runCheckout(name, null, undoing); // same branch: just the pull
    const changes = await api.changedFileCount(repo).catch(() => 0);
    if (this.repo !== repo) return;
    if (changes > 0) this.pendingCheckout = { name, changes, undoing };
    else this.runCheckout(name, null, undoing);
  }

  async runCheckout(name: string, wip: WipMode | null, undoing = this.pendingCheckout?.undoing ?? null) {
    this.pendingCheckout = null;
    const repo = this.repo, previous = this.history.current;
    const result = await this.act(`Checking out ${name}…`, "Checkout failed", [
      ...(wip === "shelve" ? ["Shelving your changes for the branch you're leaving"] : wip === "commit" ? ["Saving your changes as a WIP commit on the branch you're leaving"]
        : wip === "carry" ? ["Stashing your changes to bring them along"] : []),
      "Fetching from origin",
      `Switching to ${name}`,
      "Fast-forwarding it to its latest (never merges)",
      ...(wip === "carry" ? ["Applying your changes on it"] : []),
    ], (repo) => api.checkout(repo, name, wip));
    if (!result.ok) return;
    if (undoing) this.markUndone(undoing);
    const back = previous && previous !== localName(name);
    this.record(repo!, `Checked out ${name}`, result.started, back ? { kind: "checkout", branch: previous } : undefined);
  }

  /** Fetch + fast-forward one branch without switching to it. */
  async pull(name: string) {
    const repo = this.repo;
    const result = await this.act(`Pulling ${name}…`, "Pull failed", ["Fetching from origin", `Fast-forwarding ${name}, without switching`, "Never merges or overwrites"],
      (repo) => api.pull(repo, name));
    if (result.ok) this.record(repo!, result.status, result.started);
  }

  /** Shipped branch clean-up: local only, or local + its remote. Undo brings back the local one. */
  /** Fetches, then whether `name` can be deleted and where it is. Null without a base to compare with. */
  async deletion(name: string): Promise<Deletion | null> {
    const repo = this.repo, base = this.history.base;
    return repo && base ? api.deletion(repo, name, base, this.targets) : null;
  }

  /** Deletes a shipped branch here (and with `remote`, on the server); the checked-out one switches to the base first.
   *  Undo brings the local branch back. */
  async deleteBranch(name: string, remote: boolean) {
    const repo = this.repo, base = this.history.base;
    if (!repo || !base) return;
    const tip = await api.branchTip(repo, name);
    const result = await this.act(`Deleting ${name}…`, "Delete failed",
      ["Checking it's in the base and every environment", ...(this.isCurrent(name) ? [`Switching to ${localName(base)}`] : []), "Deleting the branch", ...(remote ? [`Deleting origin/${name}`] : [])],
      (repo) => api.deleteBranch(repo, name, base, this.targets, remote));
    if (result.ok) this.record(repo, result.status, result.started, tip ? { kind: "move", branch: name, expected: null, target: tip } : undefined);
  }

  /** "Start feature": new branch from an up-to-date `base`, checked out. */
  async startFeature(name: string, base: string, wip: WipMode | null): Promise<Outcome> {
    const repo = this.repo, previous = this.history.current;
    const result = await this.act(`Creating ${name}…`, "Couldn't create the branch", [`Bringing ${base} up to date with origin`, `Creating ${name} from it`, `Checking out ${name}`],
      (repo) => api.startFeature(repo, name, base, wip));
    if (result.ok) this.record(repo!, `Started ${name} from ${base}`, result.started, previous ? { kind: "checkout", branch: previous } : undefined);
    return result;
  }

  /** "Update from main": merge (or rebase) the base into the branch. Undo puts the branch back. */
  async updateFromParent(card: Card, rebase: boolean): Promise<Outcome> {
    const repo = this.repo, parent = this.history.base;
    if (!repo || !parent) return { ok: false, error: "No base branch" };
    const before = await api.branchTip(repo, card.local);
    const text = `${rebase ? "Rebased" : "Merged"} ${parent} into ${card.local}`;
    const result = await this.act(`${rebase ? "Rebasing" : "Merging"} ${parent} into ${card.local}…`, "Update failed",
      [`${rebase ? "Rebasing" : "Merging"} ${parent} into ${card.local}`, "Aborts cleanly on a conflict"],
      async (repo) => (await api.updateBranch(repo, card.local, parent, rebase)) === "done" ? text
        : `Stopped on conflicts: resolve them on the uncommitted row, then Continue (or Abort to go back)`);
    if (result.ok && this.history.conflict) this.select(WORKTREE_ID); // the resolver is on the uncommitted row
    else if (result.ok) this.record(repo, text, result.started, await this.moveBack(repo, card.local, before));
    return result;
  }

  /** Pushes `branch` to its namesake on the server. `behind`: refused because the server has commits it doesn't;
   *  `force` replaces them, but still refuses if they weren't fetched yet (--force-with-lease). */
  async push(branch: string, force = false): Promise<Outcome & { behind: boolean }> {
    const repo = this.repo;
    let behind = false;
    const result = await this.act(force ? `Force-pushing ${branch}…` : `Pushing ${branch}…`, force ? "Force push failed" : "Push failed",
      force ? [`Pushing ${branch} over the server's version`, "Refused if the server has commits you haven't fetched"] : [`Pushing ${branch}`, "Sets it to follow the server's branch the first time"],
      async (repo) => {
        if ((await api.push(repo, branch, force)) === "done") return force ? `Force-pushed ${branch}` : `Pushed ${branch}`;
        behind = true;
        throw `The server has commits ${branch} doesn't${force ? " and you haven't fetched them" : ""}: nothing was pushed`;
      });
    if (result.ok) this.record(repo!, result.status, result.started);
    if (behind && !force) this.suggestion = { kind: "pullPush", repo: repo!, branch };
    return { ...result, behind };
  }

  /** After a refused push: bring the server's commits in (rebase, never a merge commit), then push. */
  async pullAndPush(branch: string) {
    const repo = this.repo;
    const result = await this.act(`Updating ${branch} and pushing…`, "Couldn't push", ["Fetching from origin", `Rebasing ${branch} onto the server's commits`, "Stops for you on a conflict", `Pushing ${branch}`],
      (repo) => api.pullAndPush(repo, branch));
    if (result.ok) this.record(repo!, result.status, result.started);
    if (this.history.conflict) this.select(WORKTREE_ID);
  }

  discardFile(path: string) {
    this.discard(`Discarded changes in ${path}`, (repo) => api.discardFile(repo, path));
  }

  /** Tracked changes only: untracked files stay (deleting them is `deleteUntracked`, after its dialog). */
  discardAll() {
    this.discard("Discarded all changes", (repo) => api.discardAll(repo));
  }

  /** Gone for good (git has no copy): only after its dialog, and without an Undo. */
  async deleteUntracked(paths: string[]) {
    const repo = this.repo;
    const text = paths.length === 1 ? `Deleted ${paths[0]} (untracked)` : `Deleted ${paths.length} untracked files`;
    const result = await this.act("Deleting…", "Delete failed", [text], async (repo) => (await api.deleteUntracked(repo, paths), text));
    if (result.ok) this.record(repo!, text, result.started);
  }

  /** The block shown as `index` with `header` in the diff pane (a file git follows; an untracked one goes by `doomed`). */
  discardHunk(index: number, header: string) {
    const path = this.fileFilter;
    if (path) this.discard(`Discarded a block in ${path}`, (repo) => api.discardHunk(repo, path, index, header));
  }

  /** Undo brings the changes back from the snapshot Rust took first. */
  private async discard(text: string, work: (repo: string) => Promise<Discarded>) {
    const repo = this.repo;
    let discarded: Discarded = null;
    const result = await this.act("Discarding…", "Discard failed", [text], async (repo) => ((discarded = await work(repo)), text));
    if (result.ok) this.record(repo!, text, result.started, discarded ? { kind: "restore", discarded } : undefined);
  }

  // MARK: Stashes (rows in the graph)

  /** Puts a stash's changes back (onto a clean tree); `pop` drops it once applied. */
  async applyStash(stash: Commit, pop: boolean) {
    const repo = this.repo;
    const result = await this.act(pop ? "Popping the stash…" : "Applying the stash…", "Couldn't apply it",
      ["Only onto a clean working tree", "Changes nothing if it conflicts", ...(pop ? ["Drops the stash once applied"] : [])],
      (repo) => api.stashApply(repo, stash.id, pop));
    if (result.ok) this.record(repo!, `${result.status}: ${stash.subject}`, result.started, pop ? { kind: "stash", id: stash.id, message: stash.subject } : undefined);
  }

  async dropStash(stash: Commit) {
    const repo = this.repo;
    let message = "";
    const result = await this.act("Dropping the stash…", "Couldn't drop it", ["Removing it from the stash list (Undo puts it back)"],
      async (repo) => ((message = await api.stashDrop(repo, stash.id)), `Dropped the stash: ${message}`));
    if (result.ok) this.record(repo!, result.status, result.started, { kind: "stash", id: stash.id, message });
  }

  // MARK: Conflicts (history.conflict): resolved on the uncommitted row, file by file

  /** One pick per conflict block of `path`. */
  async resolveConflict(path: string, picks: Pick[]) {
    await this.act(`Resolving ${path}…`, "Couldn't resolve it", ["Writing your picks", "Marking it resolved"],
      async (repo) => (await api.resolveConflict(repo, path, picks), `Resolved ${path}`));
  }

  async takeSide(path: string, mine: boolean) {
    await this.act(`Resolving ${path}…`, "Couldn't resolve it", [mine ? "Keeping your version of the whole file" : "Taking their version of the whole file"],
      async (repo) => (await api.takeSide(repo, path, mine), `Resolved ${path}`));
  }

  async continueOperation() {
    const repo = this.repo;
    const result = await this.act("Continuing…", "Couldn't continue", ["Committing the resolved files", "A rebase moves on to its next commit"], (repo) => api.continueOperation(repo));
    if (result.ok) this.record(repo!, result.status, result.started);
  }

  async abortOperation() {
    const repo = this.repo;
    const result = await this.act("Aborting…", "Couldn't abort", ["Back to where the branch was before it started"], (repo) => api.abortOperation(repo));
    if (result.ok) this.record(repo!, result.status, result.started);
  }

  // MARK: Edit commits (rewrite.rs)

  /** Rebuilds `branch`'s own commits since the base as `steps` say; `rewrite`: some are pushed, force-push after.
   *  Undo puts the branch back (unless it was pushed). */
  async rewriteBranch(branch: string, steps: RewriteStep[], rewrite: boolean): Promise<Outcome> {
    const repo = this.repo, base = this.history.base;
    if (!repo || !base) return { ok: false, error: "No base branch" };
    const before = await api.branchTip(repo, branch);
    const result = await this.act(`Editing ${branch}…`, "Couldn't edit the commits",
      ["Rebuilding the commits in a temporary copy", "Changes nothing if they conflict in that order", `Moving ${branch} to the result`, ...(rewrite ? [`Force-pushing ${branch}`] : [])],
      (repo) => api.rewriteBranch(repo, branch, base, steps, rewrite));
    if (result.ok) this.record(repo, result.status, result.started, rewrite ? undefined : await this.moveBack(repo, branch, before));
    return result;
  }

  /** Stages or unstages files from the tree's checkboxes; only the tree refreshes (no toast: it's a checkbox). */
  async setStaged(paths: string[], on: boolean) {
    const repo = this.repo;
    if (!repo || !paths.length) return;
    try {
      await (on ? api.stage(repo, paths) : api.unstage(repo, paths));
    } catch (error) {
      this.syncStatus = `${on ? "Stage" : "Unstage"} failed: ${firstLine(error)}`;
    }
    if (this.repo === repo) this.loadTree();
  }

  /** Files checked in the tree: what the next commit takes. */
  get stagedCount() {
    return Object.keys(this.changes).filter((p) => this.staged.has(p)).length;
  }

  /** What's staged as a new commit, folded into the last one (an empty title keeps its message), or the branch squashed
   *  into one since its base; pushes too with `push`. Undo puts the branch back where it was, the changes staged.
   *  False if git refused, so the form keeps the message. */
  /** What to do next, offered by NextStepPrompt: push a commit that wasn't pushed, or pull and push a refused one. */
  suggestion = $state<Suggestion | null>(null);

  /** `rewrite`: amend or squash commits that are already pushed, then force-push (the form warns first). */
  async commit(title: string, description: string, { kind = "new" as "new" | "amend" | "squash", push = false, rewrite = false } = {}) {
    const amend = kind === "amend", squash = kind === "squash";
    const repo = this.repo, branch = this.history.current;
    const message = title.trim() ? title.trim() + (description.trim() ? "\n\n" + description.trim() : "") + "\n" : null;
    const before = repo && branch ? await api.branchTip(repo, branch) : null;
    const base = this.history.base;
    if (squash && !base) return false;
    const how: CommitKind = kind === "squash" ? { squash: base! } : kind;
    const verb = squash ? "Squashing" : amend ? "Amending" : "Committing";
    let behind = false;
    const result = await this.act(push ? `${verb} and pushing…` : `${verb}…`, `${squash ? "Squash" : amend ? "Amend" : "Commit"} failed`,
      [squash ? `Making ${branch}'s commits since ${base} and what's staged one commit`
        : amend ? `Folding what's staged into the last commit${message ? ", with the new message" : ""}` : "Committing what's staged",
        ...(push ? [`Pushing ${branch ?? "the branch"} to origin`] : []),
        ...(rewrite ? [`Force-pushing ${branch ?? "the branch"} (refused if the server has commits you haven't fetched)`] : [])],
      async (repo) => {
        const done = await api.commit(repo, message, how, push, rewrite);
        behind = done.behind;
        return done.status;
      });
    if (!result.ok) return false;
    // None once pushed: that would rewrite shared history.
    const undo = !push && !rewrite && branch ? await this.moveBack(repo!, branch, before) : undefined;
    const what = squash ? `Squashed ${branch} into one: “${title.trim()}”`
      : amend ? (message ? `Amended the last commit: “${title.trim()}”` : "Amended the last commit") : `Committed “${title.trim()}”`;
    this.record(repo!, push ? `${what}, pushed` : rewrite ? `${what}, force-pushed` : what, result.started, undo);
    this.suggestion = !branch ? null : behind ? { kind: "pullPush", repo: repo!, branch } : !push && !rewrite ? { kind: "push", repo: repo!, branch } : null;
    return true;
  }
}

/** First line of an error, for the one-line status. */
function firstLine(error: unknown) {
  return String(error).split("\n")[0] || "unknown error";
}
