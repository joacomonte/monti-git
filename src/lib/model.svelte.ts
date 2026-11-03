// App state, ported from RepoModel in ContentView.swift. Prefs keep the Swift UserDefaults key names.
import { open as pickFolders } from "@tauri-apps/plugin-dialog";
import { api, WORKTREE_ID, type Card, type Commit, type DiffRow, type Row, type WipMode } from "./api";

export const prefs = {
  get<T>(key: string, fallback: T): T {
    const raw = localStorage.getItem(key);
    return raw == null ? fallback : (JSON.parse(raw) as T);
  },
  set(key: string, value: unknown) {
    localStorage.setItem(key, JSON.stringify(value));
  },
};

export type Range = [number, number]; // first, last commit index of a table row; longer than 1 = collapsed group

/** "a/b/c.ts" paths -> nested nodes, folders first, then files, each sorted Finder-style. */
export type FileNode = { name: string; path: string; children: FileNode[] | null };

const byName = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" }).compare;

export function fileTree(paths: Iterable<string>, prefix = ""): FileNode[] {
  const files: string[] = [];
  const folders = new Map<string, string[]>();
  for (const path of paths) {
    const slash = path.indexOf("/");
    if (slash < 0) files.push(path);
    else folders.set(path.slice(0, slash), [...(folders.get(path.slice(0, slash)) ?? []), path.slice(slash + 1)]);
  }
  const dirs = [...folders.keys()].sort(byName).map((name) => {
    let node: FileNode = { name, path: prefix + name, children: fileTree(folders.get(name)!, prefix + name + "/") };
    // Folders whose only child is a folder collapse into one row: "src/main/java".
    while (node.children?.length === 1 && node.children[0].children) {
      const only = node.children[0];
      node = { name: node.name + "/" + only.name, path: only.path, children: only.children };
    }
    return node;
  });
  return [...dirs, ...files.sort(byName).map((name) => ({ name, path: prefix + name, children: null }))];
}

const PAGE = 300;

export type BranchFilter = "pending" | "safe" | "wip";

/** What a repo looked like when we left it, so coming back shows it at once while `reload()` refreshes it. */
/** `view`: where the user was (tab, commit, file, focus), restored on coming back to compare across repos. */
type Snapshot = Pick<RepoModel, "commits" | "rows" | "names" | "limit" | "hasMore" | "current" | "base" | "environments" | "localBranches"> & {
  view: {
    showGraph: boolean; selectedId: string | null; fileFilter: string | null; diffOpen: boolean; focus: string[] | null; historyPath: string | null;
  };
};

export class RepoModel {
  repo = $state<string | null>(null);
  commits = $state.raw<Commit[]>([]);
  rows = $state.raw<Row[]>([]);
  names = $state.raw<(string | null)[]>([]); // branch each commit belongs to, for the line tooltip
  selected = $state.raw<Range | null>(null);
  error = $state<string | null>(null);
  limit = PAGE;
  viewEnd = 0; // last row on screen (set by the list): a refresh near the top only reloads the first page
  private growing = false; // the next load is infinite scroll asking for more: keep the bigger limit
  hasMore = $state(false);
  isLoading = $state(false);
  onlyMine = $state(prefs.get("onlyMine", true)); // hide other people's branches; on by default
  branchesOnly = $state(prefs.get("branchesOnly", true)); // "Commits collapsed", on by default
  focus = $state.raw<string[] | null>(null); // "Only this branch": the branch and its base, the rest of the graph hidden
  historyPath = $state<string | null>(null); // a file's history: the graph shows only the commits that touched it
  private cache = new Map<string, Snapshot>();

  /** Table rows, one per commit. */
  visible = $derived(this.commits.map((_, i): Range => [i, i]));

  // column + 1: a tip whose lane ends on its own row (collapsed view) sits past both lane lists.
  graphLanes = $derived(Math.max(1, ...this.rows.map((r) => Math.max(r.before.length, r.after.length, r.column + 1))));

  /** Something is already on screen and a reload is checking it for news: the thin bar at the top. */
  get isRefreshing() {
    return this.isLoading && this.commits.length > 0;
  }

  constructor() {
    const last = prefs.get<string | null>("lastRepo", null);
    if (last) this.open(last);
  }

  open(path: string) {
    if (this.repo && this.repo !== path && !this.error) {
      const { commits, rows, names, limit, hasMore, current, base, environments, localBranches, showGraph, fileFilter, diffOpen, focus, historyPath } = this;
      const selectedId = this.selected ? commits[this.selected[0]].id : null;
      this.cache.set(this.repo, { commits, rows, names, limit, hasMore, current, base, environments, localBranches, view: { showGraph, selectedId, fileFilter, diffOpen, focus, historyPath } });
    }
    this.repo = path;
    prefs.set("lastRepo", path); // reopened on next launch
    this.selected = null;
    this.error = null;
    this.focus = null;
    this.historyPath = null;
    this.filters = [];
    this.filterRefs = null;
    this.filterRun++;
    this.isFiltering = false;
    this.syncStatus = "";
    this.syncFailure = null;
    this.pendingCheckout = null;
    this.clearDetail();
    // Cached: show it right away. Not cached: empty, so the loader shows instead of the previous repo's content.
    const cached = this.cache.get(path);
    this.commits = cached?.commits ?? [];
    this.rows = cached?.rows ?? [];
    this.names = cached?.names ?? [];
    this.limit = cached?.limit ?? PAGE;
    this.hasMore = cached?.hasMore ?? false;
    this.current = cached?.current ?? null;
    this.base = cached?.base ?? null;
    this.environments = cached?.environments ?? [];
    this.localBranches = cached?.localBranches ?? [];
    this.allBranches = [];
    // Same view as when we left this repo, shown from the cache right away while reload() refreshes it in the
    // background. First visit: the graph.
    const view = cached?.view;
    this.showGraph = view?.showGraph ?? true;
    if (view) {
      this.focus = view.focus;
      this.historyPath = view.historyPath;
      this.fileFilter = view.fileFilter;
      this.diffOpen = view.diffOpen;
    }
    this.restoreId = view?.selectedId ?? null;
    const row = this.restoreId ? this.visible.findIndex(([first]) => this.commits[first].id === this.restoreId) : -1;
    if (row >= 0) {
      this.selected = this.visible[row];
      this.scrollRow = { row };
      this.loadTree();
      this.loadDetail();
    }
    this.reload();
  }

  private restoreId: string | null = null; // commit to select once loaded, coming back to a repo

  private loading: { repo: string; done: Promise<void> } | null = null;
  private again = false;

  /** Reloads the graph. While one load runs for this repo, more calls (focus, sync, actions in a row) fold into one more
   *  load after it, so they don't stack git processes or land out of order. */
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
    this.isLoading = true;
    // Nothing past the first page on screen or selected: a refresh (focus, sync, action) needs only that page again.
    const selectedRow = this.selected?.[0] ?? -1;
    if (!this.growing && !this.restoreId && this.limit > PAGE && this.viewEnd < PAGE - 50 && selectedRow < PAGE - 50) this.limit = PAGE;
    this.growing = false;
    const selectedID = this.selected ? this.commits[this.selected[0]].id : this.restoreId; // indices go stale: re-find by hash
    const preferred = prefs.get<Record<string, string>>("mainBranches", {})[repo] ?? null;
    const environments = prefs.get<Record<string, string[]>>("environments", {})[repo] ?? null;
    try {
      const only = this.branchesOnly && !this.focus ? this.filterRefs : null;
      const history = await api.loadHistory(repo, this.limit, this.onlyMine, this.branchesOnly, preferred, this.focus ?? [], this.historyPath, environments, only);
      if (this.repo !== repo) return; // switched repos meanwhile: the newer load owns the state
      this.commits = history.commits;
      this.rows = history.rows;
      this.names = history.names;
      this.hasMore = history.hasMore;
      this.current = history.current;
      this.base = history.base;
      this.environments = history.environments;
      this.localBranches = history.local;
      this.allBranches = history.allBranches;
      this.fastForwarded = history.fastForwarded;
      this.shelvedBranches = history.shelvedBranches;
      this.error = null;
    } catch (error) {
      if (this.repo !== repo) return;
      this.commits = [];
      this.rows = [];
      this.error = String(error);
    }
    this.isLoading = false;
    // Same commit still there (e.g. a refresh on focus): keep it open and refresh its files/diff.
    const range = selectedID ? this.visible.find(([first]) => this.commits[first].id === selectedID) : undefined;
    this.selected = range ?? null;
    if (range) {
      this.loadTree();
      this.loadDetail();
      if (this.restoreId) this.scrollRow = { row: this.visible.indexOf(range) };
      this.restoreId = null;
    }
  }

  /** Infinite scroll: the list asks for more when its last rows come into view. */
  loadMore() {
    if (!this.hasMore || this.isLoading) return;
    this.limit += PAGE;
    this.growing = true;
    this.reload(); // --topo-order output is prefix-stable, so rows above don't move
  }

  /** Graph shows only `branch` and its base (where it was cut from, else main); null = everything again. */
  async setFocus(branch: string | null, base: string | null = this.base) {
    this.focus = branch ? [...new Set([branch, base].filter((r): r is string => r != null))] : null;
    this.limit = PAGE;
    await this.reload();
  }

  /** Graph shows only the commits that touched `path` (null = everything again), keeping the file open. */
  async setHistory(path: string | null) {
    this.historyPath = path;
    this.setShowGraph(true);
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
    if (this.filters.length) this.applyFilters();
    else this.reload();
  }

  // Commits collapsed filters: the graph keeps only the matching branches (plus the base and the environments).
  filters = $state.raw<BranchFilter[]>([]);
  filterRefs = $state.raw<string[] | null>(null); // the branches they matched, null = no filter
  isFiltering = $state(false); // confirming squash / cherry-pick merges in the background
  private filterRun = 0;

  setFilters(filters: BranchFilter[]) {
    this.filters = filters;
    this.applyFilters();
  }

  /** Fast pass from history (one for-each-ref per target), then the branches it couldn't place are confirmed with
   *  the card check (squash / cherry-pick; cached in Rust), reloading only if the result changed. */
  private async applyFilters() {
    const repo = this.repo, base = this.base, run = ++this.filterRun;
    if (!repo || !this.filters.length) {
      this.filterRefs = null;
      this.isFiltering = false;
      return this.reload();
    }
    const local = (ref: string) => (/^(origin|upstream)\//.test(ref) ? ref.slice(ref.indexOf("/") + 1) : ref);
    const targets = [...(base ? [base] : []), ...this.environments];
    const compared = new Set(targets.map(local));
    const wants = (done: number) =>
      (this.filters.includes("safe") && done === targets.length) || (this.filters.includes("pending") && done > 0 && done < targets.length);
    const wip = this.filters.includes("wip")
      ? [...this.shelvedBranches, ...(this.current && this.commits.some((c) => c.id === WORKTREE_ID) ? [this.current] : [])]
      : [];
    const { all, merged } = await api.mergedInto(repo, targets, this.onlyMine);
    if (run !== this.filterRun) return;
    const sets = merged.map(([, refs]) => new Set(refs));
    const branches = all.filter((ref) => !compared.has(local(ref)));
    const doneOf = (ref: string) => sets.filter((s) => s.has(ref)).length;
    this.filterRefs = [...new Set([...branches.filter((ref) => wants(doneOf(ref))), ...wip])];
    await this.reload();
    const unsure = branches.filter((ref) => doneOf(ref) < targets.length);
    if (!base || !unsure.length || !this.filters.some((f) => f !== "wip")) return;
    this.isFiltering = true;
    const found = await api.branchStatus(repo, unsure, base, targets).catch(() => []);
    if (run !== this.filterRun) return;
    this.isFiltering = false;
    const sure = branches.filter((ref) => doneOf(ref) === targets.length && wants(targets.length));
    const confirmed = found.filter((s) => wants(s.promotion.filter(([, missing]) => missing === 0).length)).map((s) => s.ref);
    const next = [...new Set([...sure, ...confirmed, ...wip])];
    if (next.join() !== this.filterRefs?.join()) {
      this.filterRefs = next;
      await this.reload();
    }
  }


  /** Opens the file tree only; the diff pane waits until a file is picked there. */
  select(range: Range | null) {
    if (range && this.selected && range[0] === this.selected[0] && range[1] === this.selected[1]) return;
    this.selected = range;
    this.restoreId = null;
    this.diffOpen = false;
    if (!range) return; // deselected: the panes hide, nothing to load
    this.clearDetail();
    this.diffMessage = "Loading…";
    if (this.historyPath) {
      // A file's history: each commit opens straight on that file's diff.
      this.fileFilter = this.historyPath;
      this.diffOpen = true;
      this.loadDetail();
    }
    this.loadTree();
  }

  // MARK: File tree and diff

  changes = $state.raw<Record<string, string>>({});
  counts = $state.raw<Record<string, [number, number]>>({});
  staged = $state.raw(new Set<string>()); // uncommitted-changes row: files in the index (checked in the tree)
  tree = $state.raw<FileNode[]>([]);
  treePaths = $state.raw<string[]>([]); // every file the tree lists, for the tree search
  changedDirs = $state.raw(new Set<string>()); // folders containing a change, highlighted and expanded
  expandedDirs = $state.raw(new Set<string>());
  changedOrder = $state.raw<string[]>([]); // changed files in tree order, for ⌥←/⌥→
  fileFilter = $state<string | null>(null); // file shown in the diff pane
  diffOpen = $state(false); // diff pane under the graph: opens on the first file click in the tree
  showAllFiles = $state(prefs.get("treeShowsAll", false));
  diffRows = $state.raw<DiffRow[]>([]);
  diffMessage = $state("Select a file");
  fileCommits = $state.raw<Commit[]>([]); // commits of the selection that touched `fileFilter`, newest first

  get isWorktreeSelected() {
    return this.selected != null && this.commits[this.selected[0]].id === WORKTREE_ID;
  }

  private clearDetail() {
    this.fileFilter = null;
    this.diffRows = [];
    this.fileCommits = [];
    this.tree = [];
    this.treePaths = [];
  }

  /** base/target of the selection: a group diffs its oldest commit's parent against its newest. */
  private span(range: Range) {
    const newest = this.commits[range[0]];
    const base = (range[0] === range[1] ? newest : this.commits[range[1]]).parents[0] ?? null;
    return { base, target: newest.id === WORKTREE_ID ? null : newest.id };
  }

  /** Tree search by content, on the selection: diff lines, or whole files (only the changed ones unless the tree shows all). */
  async searchLines(query: string, diffOnly: boolean): Promise<Record<string, number>> {
    if (!this.repo || !this.selected) return {};
    const { base, target } = this.span(this.selected);
    const paths = this.showAllFiles ? null : Object.keys(this.changes).filter((p) => this.changes[p] !== "D");
    if (paths && !paths.length) return {};
    return api.searchLines(this.repo, base, target, query, paths, diffOnly);
  }

  setShowAllFiles(all: boolean) {
    this.showAllFiles = all;
    prefs.set("treeShowsAll", all);
    this.loadTree();
  }

  private async loadTree() {
    const repo = this.repo, range = this.selected, all = this.showAllFiles;
    if (!repo || !range) return;
    const { base, target } = this.span(range);
    const result = await api.loadChanges(repo, base, target, all);
    if (this.selected !== range || this.showAllFiles !== all) return; // user moved on while git was running
    this.changes = result.changes;
    this.counts = result.counts;
    this.staged = new Set(result.staged);
    this.tree = fileTree(result.paths);
    this.treePaths = result.paths;
    const dirs = new Set<string>();
    for (const path of Object.keys(result.changes)) {
      const parts = path.split("/").slice(0, -1);
      parts.forEach((_, i) => dirs.add(parts.slice(0, i + 1).join("/")));
    }
    this.changedDirs = dirs;
    this.expandedDirs = new Set(dirs);
    const files = (nodes: FileNode[]): string[] => nodes.flatMap((n) => (n.children ? files(n.children) : [n.path]));
    this.changedOrder = files(this.tree).filter((p) => p in result.changes);
    if (this.diffOpen && (!this.fileFilter || !(this.fileFilter in result.changes))) {
      // New selection: start at the first changed file.
      this.fileFilter = this.changedOrder[0] ?? null;
      this.loadDetail();
    }
    if (!this.changedOrder.length) this.diffMessage = "No changes";
  }

  /** Picks the file shown in the diff. A folder jumps to its first changed file. */
  /** Clicking the file already shown hides/shows the bottom pane. */
  filter(path: string) {
    if (this.fileFilter === path) {
      this.diffOpen = !this.diffOpen;
      return;
    }
    this.diffOpen = true;
    this.fileFilter = path in this.changes ? path : (this.changedOrder.find((p) => p.startsWith(path + "/")) ?? path);
    this.loadDetail();
  }

  /** Previous/next changed file, opening its folders in the tree so it stays visible. */
  stepFile(offset: number) {
    const index = this.fileFilter ? this.changedOrder.indexOf(this.fileFilter) : -1;
    const path = this.changedOrder[index + offset];
    if (index < 0 || !path) return;
    const dirs = new Set(this.expandedDirs);
    const parts = path.split("/").slice(0, -1);
    parts.forEach((_, i) => dirs.add(parts.slice(0, i + 1).join("/")));
    this.expandedDirs = dirs;
    this.filter(path);
  }

  private async loadDetail() {
    const repo = this.repo, range = this.selected, path = this.fileFilter;
    if (!repo || !range || !path) return;
    const { base, target } = this.span(range);
    const group = this.commits.slice(range[0], range[1] + 1).filter((c) => c.id !== WORKTREE_ID);
    const result = await api.loadDiff(repo, path, base, target, group.map((c) => c.id), !(path in this.changes));
    if (this.selected !== range || this.fileFilter !== path) return;
    this.fileCommits = group.filter((c) => result.touching.includes(c.id));
    this.diffRows = result.rows;
    this.diffMessage = path in this.changes ? "Binary file or no text changes" : "Empty or binary file";
  }

  // MARK: Branches (come with every history load)

  current = $state<string | null>(null); // checked-out branch
  base = $state<string | null>(null); // the branch pinned to the left
  environments = $state.raw<string[]>([]); // qa, develop…
  localBranches = $state.raw<string[]>([]); // for "Change base"
  allBranches = $state.raw<string[]>([]); // local and origin names, for the environments picker
  shelvedBranches = $state.raw<string[]>([]); // branches with WIP the app put aside
  fastForwarded = $state(false); // "Only this branch" on a branch the base fast-forwarded to: no merge to draw
  showGraph = $state(true); // the graph is the app; the Dashboard tab is a placeholder for what comes next
  scrollRow = $state<{ row: number } | null>(null); // table row to bring into view; a new object every request

  setShowGraph(value: boolean) {
    this.showGraph = value;
  }

  /** A remote-only base ("origin/main") counts as current when its local twin is checked out. */
  isCurrent(name: string) {
    const current = this.current;
    return current != null && (name === current || name.endsWith("/" + current));
  }

  /** Same "mainBranches" store as the graph's base: the whole repo reloads. */
  setBaseBranch(branch: string) {
    if (!this.repo) return;
    prefs.set("mainBranches", { ...prefs.get("mainBranches", {}), [this.repo]: branch });
    this.reload();
  }

  /** The user's environments for this repo, by branch name; null goes back to detecting them by name (qa, develop…). */
  setEnvironments(names: string[] | null) {
    if (!this.repo) return;
    const { [this.repo]: _, ...others } = prefs.get<Record<string, string[]>>("environments", {});
    prefs.set("environments", names ? { ...others, [this.repo]: names } : others);
    this.reload();
  }

  /** An older commit may sit past the loaded page: keep loading history until it shows up (or there is no more). */
  private async loadUntil(id: string) {
    const repo = this.repo;
    const idle = async () => { while (this.isLoading) await new Promise((r) => setTimeout(r, 100)); };
    await idle();
    for (let pages = 0; this.repo === repo && pages < 30 && !this.commits.some((c) => c.id === id); pages++) {
      if (!this.hasMore) break;
      this.loadMore();
      await idle();
    }
    return this.commits.some((c) => c.id === id);
  }

  /** Global search result: opens its repo on the graph with that commit selected. A commit hidden by
   *  "My branches only" (someone else's branch) turns that filter off, so it can be shown. */
  async revealCommit(repo: string, id: string) {
    if (this.repo !== repo) this.open(repo);
    this.setShowGraph(true);
    if (this.focus) await this.setFocus(null);
    if (this.historyPath) await this.setHistory(null);
    if (!(await this.loadUntil(id)) && this.onlyMine && this.repo === repo) {
      this.setOnlyMine(false);
      await this.loadUntil(id);
    }
    if (this.repo !== repo) return;
    const index = this.commits.findIndex((c) => c.id === id);
    const row = this.visible.findIndex(([first, last]) => first <= index && index <= last);
    if (index < 0 || row < 0) return;
    this.select(this.visible[row]);
    this.scrollRow = { row };
  }

  /** Opens the graph on the working-tree row, with its files. */
  revealWorktree() {
    this.setShowGraph(true);
    const row = this.visible.findIndex(([first]) => this.commits[first].id === WORKTREE_ID);
    if (row < 0) return;
    this.select(this.visible[row]);
    this.scrollRow = { row };
  }

  // MARK: Repo sources: what the sidebar lists, added by the user (nothing is scanned by default)

  sources = $state<{ folders: string[]; repos: string[]; hidden: string[] }>(
    prefs.get("repoSources", { folders: [], repos: [], hidden: [] }), // hidden: repos of an added folder taken out of the list
  );

  onboarding = $state(false); // keeps the onboarding up after the first repos are added, for its favorites step

  // Same blob as SidebarPrefs in RepoSidebar.swift; only `favorites` is used so far.
  favorites = $state<string[]>(prefs.get<{ favorites?: string[] }>("sidebarPrefs", {}).favorites ?? []);

  toggleFavorite(path: string) {
    this.favorites = this.favorites.includes(path) ? this.favorites.filter((p) => p !== path) : [...this.favorites, path];
    prefs.set("sidebarPrefs", { ...prefs.get<object>("sidebarPrefs", {}), favorites: this.favorites });
  }

  get hasSources() {
    return this.sources.folders.length + this.sources.repos.length > 0;
  }

  private saveSources(next: typeof this.sources) {
    this.sources = next;
    prefs.set("repoSources", next);
  }

  /** Native folder picker. "repo": each pick must be a git repo. "folder": every repo directly inside it, rescanned on refresh. */
  async addRepos(kind: "repo" | "folder") {
    const picked = await pickFolders({ directory: true, multiple: true, title: kind === "repo" ? "Add repositories" : "Add a folder of repositories" });
    const paths = picked == null ? [] : Array.isArray(picked) ? picked : [picked];
    if (!paths.length) return;
    const s = this.sources;
    const unhide = (added: string[]) => s.hidden.filter((h) => !added.includes(h));
    if (kind === "folder") {
      this.saveSources({ ...s, folders: [...new Set([...s.folders, ...paths])] });
      return;
    }
    const checks = await Promise.all(paths.map((p) => api.isRepo(p)));
    const repos = paths.filter((_, i) => checks[i]);
    const rejected = paths.filter((_, i) => !checks[i]);
    if (rejected.length) this.syncStatus = `Not a git repository: ${rejected.map((p) => p.split(/[\\/]/).pop()).join(", ")}. Use "Add folder of repos" for a folder that contains several.`;
    if (!repos.length) return;
    this.saveSources({ ...s, repos: [...new Set([...s.repos, ...repos])], hidden: unhide(repos) });
    if (!this.repo) this.open(repos[0]);
  }

  removeRepo(path: string) {
    this.saveSources({ ...this.sources, repos: this.sources.repos.filter((r) => r !== path) });
  }

  hideRepo(path: string) {
    this.saveSources({ ...this.sources, hidden: [...this.sources.hidden, path] });
  }

  removeFolder(folder: string) {
    this.saveSources({ ...this.sources, folders: this.sources.folders.filter((f) => f !== folder) });
  }

  // MARK: Actions (see src-tauri/src/actions.rs)

  isSyncing = $state(false); // one git action at a time: sync, checkout or pull
  syncStatus = $state(""); // last action's one-line result, at the bottom of the sidebar
  syncFailure = $state<{ title: string; reasons: string[]; log: string } | null>(null); // stays until dismissed
  pendingCheckout = $state<string | null>(null); // dirty tree: a dialog asks what to do with the changes
  pendingChangeCount = $state(0);
  /** The action running (or just finished), for the bottom toast (ActionToast): what it is, what it does, how it went. */
  toast = $state<{ id: number; title: string; details: string[]; state: "busy" | "done" | "failed"; result: string; started: number } | null>(null);

  /** Runs one git action with the shared busy flag and status line, then reloads. True if it worked. */
  private async act(busy: string, failed: string, work: (repo: string) => Promise<string>, details: string[] = []) {
    const repo = this.repo;
    if (!repo || this.isSyncing) return false;
    this.isSyncing = true;
    this.syncStatus = busy;
    this.actStarted = Date.now();
    const toast = { id: Date.now(), title: busy, details, state: "busy" as const, result: "", started: Date.now() };
    this.toast = toast;
    let done = false;
    try {
      this.syncStatus = await work(repo);
      done = true;
    } catch (error) {
      this.syncStatus = `${failed}: ${String(error).split("\n")[0] || "unknown error"}`;
    }
    this.isSyncing = false;
    if (this.toast?.id === toast.id) {
      this.toast = { ...toast, state: done ? "done" : "failed", result: this.syncStatus };
      setTimeout(() => { if (this.toast?.id === toast.id) this.toast = null; }, done ? 3500 : 9000);
    }
    if (this.repo === repo) this.reload();
    return done;
  }

  // MARK: Activity: the relevant git actions done from the app, newest first, some with an Undo.

  activity = $state<Activity[]>(prefs.get("activity", []));

  private actStarted = 0; // start of the last act()/rewrite(), stamped on its Activity entry
  private record(repo: string, text: string, undo?: Undo) {
    this.activity = [{ at: Date.now(), started: this.actStarted || undefined, repo, text, undo }, ...this.activity].slice(0, 50);
    prefs.set("activity", this.activity);
  }

  /** Checkout goes back through `checkout` (it asks about uncommitted changes); the rest move a branch back,
   *  which Rust refuses if it moved since or was pushed. */
  async undo(entry: Activity) {
    const undo = entry.undo;
    if (!undo || entry.undone || entry.repo !== this.repo) return;
    if (undo.kind === "checkout") return this.checkout(undo.branch);
    const done = await this.act("Undoing…", "Undo failed", async (repo) => {
      await api.moveBranch(repo, undo.branch, undo.expected, undo.target);
      return `Undone: ${entry.text}`;
    }, [`Moving ${undo.branch} back to ${undo.target.slice(0, 7)}`, "Only if it hasn't moved since and isn't on the remote"]);
    if (!done) return;
    this.activity = this.activity.map((e) => (e === entry ? { ...e, undone: true } : e));
    prefs.set("activity", this.activity);
  }

  /** Fetch --all --prune + safe fast-forwards of every branch that's behind. Skipped branches open the failure popover. */
  async sync() {
    this.syncFailure = null;
    const repo = this.repo;
    if (await this.act("Syncing…", "Sync failed", async (repo) => {
      try {
        const result = await api.sync(repo);
        this.lastSynced = Date.now();
        if (result.skipped.length) this.syncFailure = { title: "Some branches weren't updated", reasons: result.skipped, log: result.log.join("\n\n") };
        const time = new Date().toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
        const parts = [`Synced ${time}`];
        if (result.updated.length) parts.push("updated: " + result.updated.join(", "));
        if (result.skipped.length) parts.push("skipped: " + result.skipped.join(", "));
        if (!result.updated.length && !result.skipped.length) parts.push("all branches up to date");
        return parts.join(" · ");
      } catch (error) {
        this.syncFailure = { title: "Sync failed", reasons: ["Couldn't reach the remote or run git."], log: String(error) };
        throw error;
      }
    }, ["Fetching every remote", "Fast-forwarding branches that are behind", "Skipping anything that isn't a clean fast-forward"])) this.record(repo!, this.syncStatus);
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
    if (!repo || !this.autoSyncOn || this.isSyncing || Date.now() - this.lastAutoSync < 60_000) return;
    this.lastAutoSync = Date.now();
    this.isSyncing = true;
    const result = await api.sync(repo).catch(() => null);
    this.isSyncing = false;
    if (result) this.lastSynced = Date.now();
    if (result?.changed && this.repo === repo) this.reload(); // nothing new from origin: focus already reloaded
  }

  /** With uncommitted changes, asks first (stash / WIP commit / move them) via `pendingCheckout`. */
  async checkout(name: string) {
    const repo = this.repo;
    if (!repo || this.isSyncing) return;
    if (this.isCurrent(name)) return this.runCheckout(name, null); // same branch: just the pull
    const count = await api.changedFileCount(repo);
    if (this.repo !== repo) return;
    if (count > 0) {
      this.pendingChangeCount = count;
      this.pendingCheckout = name;
    } else {
      this.runCheckout(name, null);
    }
  }

  async runCheckout(name: string, wip: WipMode | null) {
    this.pendingCheckout = null;
    const repo = this.repo, previous = this.current;
    if (!(await this.act(`Checking out ${name}…`, "Checkout failed", (repo) => api.checkout(repo, name, wip), [
      ...(wip === "shelve" ? ["Shelving your changes for the branch you're leaving"] : wip === "commit" ? ["Saving your changes as a WIP commit on the branch you're leaving"]
        : wip === "carry" ? ["Stashing your changes to bring them along"] : []),
      "Fetching from origin",
      `Switching to ${name}`,
      "Fast-forwarding it to its latest (never merges)",
      ...(wip === "carry" ? ["Applying your changes on it"] : []),
    ]))) return;
    const back = previous && previous !== name.replace(/^(origin|upstream)\//, "");
    this.record(repo!, `Checked out ${name}`, back ? { kind: "checkout", branch: previous } : undefined);
  }

  /** Fetch + fast-forward one branch without switching to it. */
  async pull(name: string) {
    const repo = this.repo;
    if (await this.act(`Pulling ${name}…`, "Pull failed", (repo) => api.pull(repo, name),
      ["Fetching from origin", `Fast-forwarding ${name}, without switching`, "Never merges or overwrites"])) this.record(repo!, this.syncStatus);
  }

  /** Shipped branch clean-up: local only, or local + its remote. Undo brings back the local one. */
  async deleteBranch(name: string, remote: boolean) {
    const repo = this.repo;
    const tip = repo ? await api.branchTip(repo, name) : null;
    if (!(await this.act(`Deleting ${name}…`, "Delete failed", (repo) => api.deleteBranch(repo, name, remote),
      [`Deleting the local branch (git refuses if it isn't merged)`, ...(remote ? [`Deleting origin/${name}`] : [])]))) return;
    this.record(repo!, this.syncStatus, tip ? { kind: "move", branch: name, expected: null, target: tip } : undefined);
  }

  /** Runs a branch-rewriting action and logs it with an Undo back to where the branch was. */
  private async rewrite(branch: string, text: string, work: (repo: string) => Promise<unknown>) {
    const repo = this.repo;
    if (!repo) return;
    this.actStarted = Date.now();
    const before = await api.branchTip(repo, branch);
    try {
      await work(repo);
    } finally {
      this.reload();
    }
    const after = await api.branchTip(repo, branch);
    this.record(repo, text, before && after && before !== after ? { kind: "move", branch, expected: after, target: before } : undefined);
  }

  /** "Start feature": new branch from an up-to-date `base`, checked out. False if git refused (status line says why). */
  async startFeature(name: string, base: string, wip: WipMode | null) {
    const repo = this.repo, previous = this.current;
    const done = await this.act(`Creating ${name}…`, "Couldn't create the branch", (repo) => api.startFeature(repo, name, base, wip),
      [`Bringing ${base} up to date with origin`, `Creating ${name} from it`, `Checking out ${name}`]);
    if (done) this.record(repo!, `Started ${name} from ${base}`, previous ? { kind: "checkout", branch: previous } : undefined);
    return done;
  }

  /** "Update from main": merge (or rebase) the base into the branch. Throws so the panel can show why. */
  async updateFromParent(card: Card, rebase: boolean) {
    const parent = this.base;
    if (!parent) return;
    await this.rewrite(card.local, `${rebase ? "Rebased" : "Merged"} ${parent} into ${card.local}`,
      (repo) => api.updateBranch(repo, card.local, parent, rebase, this.isCurrent(card.ref)));
  }

  async forcePush(branch: string) {
    const repo = this.repo;
    if (!repo) return;
    try {
      await api.forcePush(repo, branch);
      this.record(repo, `Force-pushed ${branch}`);
    } finally {
      this.reload();
    }
  }

  discardFile(path: string) {
    this.discard(`Discarded changes in ${path}`, (repo) => api.discardFile(repo, path));
  }

  discardAll() {
    this.discard("Discarded all changes", (repo) => api.discardAll(repo));
  }

  discardHunk(index: number) {
    const path = this.fileFilter;
    if (path) this.discard(`Discarded a block in ${path}`, (repo) => api.discardHunk(repo, path, index));
  }

  /** Stages or unstages files from the tree's checkboxes; only the tree refreshes. */
  async setStaged(paths: string[], on: boolean) {
    const repo = this.repo;
    if (!repo || !paths.length) return;
    try {
      await (on ? api.stage(repo, paths) : api.unstage(repo, paths));
    } catch (error) {
      this.syncStatus = `${on ? "Stage" : "Unstage"} failed: ` + String(error);
    }
    if (this.repo !== repo) return;
    this.loadTree();
  }

  /** Commits what's staged (and pushes with `push`). Resolves to false if git refused, so the form keeps the message. */
  async commit(message: string, push: boolean) {
    const repo = this.repo, branch = this.current;
    const before = repo && branch ? await api.branchTip(repo, branch) : null;
    let after: string | null = null;
    const done = await this.act(push ? "Committing and pushing…" : "Committing…", "Commit failed", async (repo) => {
      const status = await api.commit(repo, message, push);
      if (branch) after = await api.branchTip(repo, branch);
      return status;
    }, ["Committing what's staged", ...(push ? [`Pushing ${branch ?? "the branch"} to origin`] : [])]);
    if (done) {
      const subject = message.split("\n")[0];
      // Undo keeps the changes (staged); none once pushed: that would rewrite shared history.
      this.record(repo!, `${push ? "Committed and pushed" : "Committed"} “${subject}”`,
        !push && before && after && branch ? { kind: "move", branch, expected: after, target: before } : undefined);
    }
    return done;
  }

  private async discard(text: string, work: (repo: string) => Promise<void>) {
    const repo = this.repo;
    if (!repo) return;
    try {
      await work(repo);
      this.record(repo, text);
    } catch (error) {
      this.syncStatus = "Discard failed: " + String(error);
    }
    this.reload();
  }
}

/** Puts things back: switch to the branch you were on, or move a branch back (`expected` null: recreate it). */
type Undo = { kind: "checkout"; branch: string } | { kind: "move"; branch: string; expected: string | null; target: string };
export type Activity = { at: number; started?: number; repo: string; text: string; undo?: Undo; undone?: boolean }; // started: when its git commands began (Logs panel groups them)
