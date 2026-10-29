// App state, ported from RepoModel in ContentView.swift. Prefs keep the Swift UserDefaults key names.
import { open as pickFolders } from "@tauri-apps/plugin-dialog";
import { api, WORKTREE_ID, type Card, type Commit, type Dashboard, type DiffRow, type Row, type WipMode } from "./api";

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

/** What a repo looked like when we left it, so coming back shows it at once while `reload()` refreshes it. */
type Snapshot = Pick<RepoModel, "commits" | "rows" | "runs" | "names" | "expanded" | "limit" | "hasMore" | "dashboard">;

export class RepoModel {
  repo = $state<string | null>(null);
  commits = $state.raw<Commit[]>([]);
  rows = $state.raw<Row[]>([]);
  runs = $state.raw(new Map<number, number>()); // collapsible groups: first commit index -> last
  names = $state.raw<(string | null)[]>([]); // branch each commit belongs to, for the line tooltip
  expanded = $state.raw(new Set<string>()); // hash of the first commit of each expanded group
  selected = $state.raw<Range | null>(null);
  error = $state<string | null>(null);
  limit = PAGE;
  hasMore = $state(false);
  isLoading = $state(false);
  onlyMine = $state(prefs.get("onlyMine", true)); // hide other people's branches; on by default
  private cache = new Map<string, Snapshot>();

  /** Table rows: a range longer than 1 is a collapsed group. */
  visible = $derived.by(() => {
    const result: Range[] = [];
    for (let i = 0; i < this.commits.length; ) {
      const last = this.runs.get(i);
      if (last != null && !this.expanded.has(this.commits[i].id)) {
        result.push([i, last]);
        i = last + 1;
      } else {
        result.push([i, i]);
        i += 1;
      }
    }
    return result;
  });

  graphLanes = $derived(Math.max(1, ...this.rows.map((r) => Math.max(r.before.length, r.after.length))));

  /** Something is already on screen and a reload is checking it for news: the thin bar at the top. */
  get isRefreshing() {
    return (this.isLoading || this.isLoadingDashboard) && (this.dashboard != null || this.commits.length > 0);
  }

  constructor() {
    const last = prefs.get<string | null>("lastRepo", null);
    if (last) this.open(last);
  }

  open(path: string) {
    if (this.repo && this.repo !== path && !this.error) {
      const { commits, rows, runs, names, expanded, limit, hasMore, dashboard } = this;
      this.cache.set(this.repo, { commits, rows, runs, names, expanded, limit, hasMore, dashboard });
    }
    this.repo = path;
    prefs.set("lastRepo", path); // reopened on next launch
    this.selected = null;
    this.error = null;
    this.highlightChain = new Set();
    this.syncStatus = "";
    this.syncFailure = null;
    this.pendingCheckout = null;
    this.clearDetail();
    // Cached: show it right away. Not cached: empty, so the loader shows instead of the previous repo's content.
    const cached = this.cache.get(path);
    this.commits = cached?.commits ?? [];
    this.rows = cached?.rows ?? [];
    this.runs = cached?.runs ?? new Map();
    this.names = cached?.names ?? [];
    this.expanded = cached?.expanded ?? new Set();
    this.limit = cached?.limit ?? PAGE;
    this.hasMore = cached?.hasMore ?? false;
    this.dashboard = cached?.dashboard ?? null;
    this.reload();
  }

  async reload() {
    const repo = this.repo;
    if (!repo) return;
    this.loadDashboard();
    this.isLoading = true;
    const selectedID = this.selected ? this.commits[this.selected[0]].id : null; // indices go stale: re-find by hash
    const preferred = prefs.get<Record<string, string>>("mainBranches", {})[repo] ?? null;
    try {
      this.historyEnvironments = this.environmentRefs();
      const history = await api.loadHistory(repo, this.limit, this.onlyMine, preferred, this.historyEnvironments);
      if (this.repo !== repo) return; // switched repos meanwhile: the newer load owns the state
      this.commits = history.commits;
      this.rows = history.rows;
      this.runs = new Map(history.runs);
      this.names = history.names;
      this.hasMore = history.hasMore;
      this.error = null;
    } catch (error) {
      if (this.repo !== repo) return;
      this.commits = [];
      this.rows = [];
      this.runs = new Map();
      this.error = String(error);
    }
    this.isLoading = false;
    // Same commit still there (e.g. a refresh on focus): keep it open and refresh its files/diff.
    const range = selectedID ? this.visible.find(([first]) => this.commits[first].id === selectedID) : undefined;
    this.selected = range ?? null;
    if (range) {
      this.loadTree();
      this.loadDetail();
    }
  }

  /** Infinite scroll: the list asks for more when its last rows come into view. */
  loadMore() {
    if (!this.hasMore || this.isLoading) return;
    this.limit += PAGE;
    this.reload(); // --topo-order output is prefix-stable, so rows above don't move
  }

  setOnlyMine(on: boolean) {
    this.onlyMine = on;
    prefs.set("onlyMine", on);
    this.reload();
  }

  /** Expands or collapses the group starting at `first`, keeping the selection on it. */
  toggle(first: number) {
    const last = this.runs.get(first);
    if (last == null) return;
    const id = this.commits[first].id;
    const expanded = new Set(this.expanded);
    const collapsing = expanded.delete(id);
    if (!collapsing) expanded.add(id);
    this.expanded = expanded;
    const sel = this.selected;
    if (sel && sel[0] <= last && sel[1] >= first) this.select(collapsing ? [first, last] : [first, first]);
  }

  select(range: Range | null) {
    if (range && this.selected && range[0] === this.selected[0] && range[1] === this.selected[1]) return;
    this.selected = range;
    if (!range) return; // deselected: the panes hide, nothing to load
    this.clearDetail();
    this.diffMessage = "Loading…";
    this.loadTree(); // then shows the first changed file
  }

  // MARK: File tree and diff

  changes = $state.raw<Record<string, string>>({});
  counts = $state.raw<Record<string, [number, number]>>({});
  staged = $state.raw(new Set<string>()); // uncommitted-changes row: files in the index (checked in the tree)
  tree = $state.raw<FileNode[]>([]);
  changedDirs = $state.raw(new Set<string>()); // folders containing a change, highlighted and expanded
  expandedDirs = $state.raw(new Set<string>());
  changedOrder = $state.raw<string[]>([]); // changed files in tree order, for ⌥←/⌥→
  fileFilter = $state<string | null>(null); // file shown in the diff pane
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
  }

  /** base/target of the selection: a group diffs its oldest commit's parent against its newest. */
  private span(range: Range) {
    const newest = this.commits[range[0]];
    const base = (range[0] === range[1] ? newest : this.commits[range[1]]).parents[0] ?? null;
    return { base, target: newest.id === WORKTREE_ID ? null : newest.id };
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
    const dirs = new Set<string>();
    for (const path of Object.keys(result.changes)) {
      const parts = path.split("/").slice(0, -1);
      parts.forEach((_, i) => dirs.add(parts.slice(0, i + 1).join("/")));
    }
    this.changedDirs = dirs;
    this.expandedDirs = new Set(dirs);
    const files = (nodes: FileNode[]): string[] => nodes.flatMap((n) => (n.children ? files(n.children) : [n.path]));
    this.changedOrder = files(this.tree).filter((p) => p in result.changes);
    if (!this.fileFilter || !(this.fileFilter in result.changes)) {
      // New selection: start at the first changed file.
      this.fileFilter = this.changedOrder[0] ?? null;
      this.loadDetail();
    }
    if (!this.changedOrder.length) this.diffMessage = "No changes";
  }

  /** Picks the file shown in the diff. A folder jumps to its first changed file. */
  filter(path: string) {
    if (this.fileFilter === path) return;
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
    const result = await api.loadDiff(repo, path, base, target, group.map((c) => c.id));
    if (this.selected !== range || this.fileFilter !== path) return;
    this.fileCommits = group.filter((c) => result.touching.includes(c.id));
    this.diffRows = result.rows;
    this.diffMessage = path in this.changes ? "Binary file or no text changes" : "No changes in this file";
  }

  // MARK: Dashboard

  dashboard = $state.raw<Dashboard | null>(null);
  isLoadingDashboard = $state(false);
  showGraph = $state(false); // every launch opens on the dashboard
  highlightChain = $state.raw(new Set<string>()); // commits of the branch shown from the dashboard: its graph line glows
  dismissedUpdates = $state.raw<Record<string, string>>({}); // branch -> parent tip when its "new commits" notice was hidden
  scrollRow = $state<{ row: number } | null>(null); // table row to bring into view; a new object every request

  setShowGraph(value: boolean) {
    this.showGraph = value;
    if (!value) this.highlightChain = new Set();
  }

  /** A remote-only base ("origin/main") counts as current when its local twin is checked out. */
  isCurrent(name: string) {
    const current = this.dashboard?.current;
    return current != null && (name === current || name.endsWith("/" + current));
  }

  isUpdateDismissed(card: Card) {
    return card.parentTip != null && this.dismissedUpdates[card.local] === card.parentTip;
  }

  dismissUpdate(card: Card) {
    if (!this.repo || !card.parentTip) return;
    this.dismissedUpdates = { ...this.dismissedUpdates, [card.local]: card.parentTip };
    prefs.set("dismissedUpdates", { ...prefs.get("dismissedUpdates", {}), [this.repo]: this.dismissedUpdates });
  }

  /** Base, environments, pins and hide/show are saved per repo; changing one only reloads the dashboard. */
  private savePref<T>(key: string, value: T) {
    if (!this.repo) return;
    prefs.set(key, { ...prefs.get(key, {}), [this.repo]: value });
    this.loadDashboard();
  }

  get pinnedBranches(): string[] {
    return this.repo ? (prefs.get<Record<string, string[]>>("pinnedBranches", {})[this.repo] ?? []) : [];
  }

  togglePin(branch: string) {
    const pins = this.pinnedBranches;
    this.savePref("pinnedBranches", pins.includes(branch) ? pins.filter((p) => p !== branch) : [...pins, branch]);
  }

  isEnvironment(local: string) {
    return this.dashboard?.environments.some((c) => c.local === local) ?? false;
  }

  /** Saves the whole list on the first change, so from then on this repo's environments are exactly the user's pick. */
  toggleEnvironment(local: string) {
    const list = this.dashboard?.environments.map((c) => c.local) ?? [];
    this.savePref("environmentBranches", list.includes(local) ? list.filter((l) => l !== local) : [...list, local]);
  }

  /** Dashboard feature pref: shown in Recent activity or Hidden. */
  setFeatureOld(branch: string, old: boolean) {
    if (!this.repo) return;
    const states = prefs.get<Record<string, Record<string, string>>>("featureStates", {})[this.repo] ?? {};
    this.savePref("featureStates", { ...states, [branch]: old ? "old" : "active" });
  }

  /** Same "mainBranches" store as the graph's base: the whole repo reloads. */
  setBaseBranch(branch: string) {
    if (!this.repo) return;
    prefs.set("mainBranches", { ...prefs.get("mainBranches", {}), [this.repo]: branch });
    this.reload();
  }

  private async loadDashboard() {
    const repo = this.repo;
    if (!repo) return;
    const mine = <T>(key: string, fallback: T) => prefs.get<Record<string, T>>(key, {})[repo] ?? fallback;
    this.isLoadingDashboard = true;
    this.dismissedUpdates = mine("dismissedUpdates", {});
    const dashboard = await api.loadDashboard(repo, mine<string | null>("mainBranches", null), mine("pinnedBranches", []),
      mine<string[] | null>("environmentBranches", null), mine("featureStates", {}));
    if (this.repo !== repo) return;
    this.dashboard = dashboard;
    this.isLoadingDashboard = false;
    // "My branches only" keeps the environments, which come from the dashboard: reload once if they arrived late or changed.
    if (this.onlyMine && this.environmentRefs().join() !== this.historyEnvironments.join()) this.reload();
  }

  private historyEnvironments: string[] = [];
  private environmentRefs() {
    return this.dashboard?.environments.map((c) => c.ref) ?? [];
  }

  /** Scrolls the graph to a branch's tip (a remote-only base matches its origin/ ref). */
  reveal(name: string) {
    this.setShowGraph(true);
    const local = /^(origin|upstream)\//.test(name) ? name.slice(name.indexOf("/") + 1) : name;
    const index = this.commits.findIndex((c) => c.refs.some((r) => r === name || r === local || r === "HEAD -> " + local));
    const row = this.visible.findIndex(([first, last]) => first <= index && index <= last);
    if (index >= 0 && row >= 0) this.scrollRow = { row };
  }

  /** Dashboard card: opens the graph at the branch tip with the branch's own line lit up (first-parent commits
   *  from the tip down to where it was cut). Cards without a cut point (main, environments) just scroll there. */
  async showInGraph(card: Card) {
    const repo = this.repo;
    if (!repo) return;
    const chain = card.squashBase ? await api.branchChain(repo, card.squashBase, card.ref) : [];
    // An older branch may sit past the loaded page: keep loading history until its tip shows up (or there is no more).
    const tip = card.last?.id;
    for (let pages = 0; tip && this.repo === repo && pages < 30 && !this.commits.some((c) => c.id === tip); pages++) {
      while (this.isLoading) await new Promise((r) => setTimeout(r, 100));
      if (!this.hasMore) break;
      this.loadMore();
      while (this.isLoading) await new Promise((r) => setTimeout(r, 100));
    }
    if (this.repo !== repo) return;
    this.reveal(card.ref);
    this.highlightChain = new Set(chain);
  }

  /** Dashboard "uncommitted changes" banner: opens the graph on the working-tree row, with its files and diffs. */
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
  pendingCheckout = $state<string | null>(null); // dirty tree: the strip under the top row asks what to do with the changes
  pendingChangeCount = $state(0);

  /** Runs one git action with the shared busy flag and status line, then reloads. */
  private async act(busy: string, failed: string, work: (repo: string) => Promise<string>) {
    const repo = this.repo;
    if (!repo || this.isSyncing) return;
    this.isSyncing = true;
    this.syncStatus = busy;
    try {
      this.syncStatus = await work(repo);
    } catch (error) {
      this.syncStatus = `${failed}: ${String(error).split("\n")[0] || "unknown error"}`;
    }
    this.isSyncing = false;
    if (this.repo === repo) this.reload();
  }

  /** Fetch --all --prune + safe fast-forwards of every branch that's behind. Skipped branches open the failure popover. */
  sync() {
    this.syncFailure = null;
    return this.act("Syncing…", "Sync failed", async (repo) => {
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
    });
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
    if (result && this.repo === repo) this.reload();
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

  runCheckout(name: string, wip: WipMode | null) {
    this.pendingCheckout = null;
    return this.act(`Checking out ${name}…`, "Checkout failed", (repo) => api.checkout(repo, name, wip));
  }

  /** Fetch + fast-forward one branch without switching to it. */
  pull(name: string) {
    return this.act(`Pulling ${name}…`, "Pull failed", (repo) => api.pull(repo, name));
  }

  /** Shipped branch clean-up: local only, or local + its remote. */
  deleteBranch(name: string, remote: boolean) {
    return this.act(`Deleting ${name}…`, "Delete failed", (repo) => api.deleteBranch(repo, name, remote));
  }

  /** Dashboard squash: the feature's own commits become one. Throws so the panel can show why. */
  async squash(card: Card, message: string) {
    const repo = this.repo, tip = card.last?.id;
    if (!repo || !card.squashBase || !tip) return;
    try {
      await api.squash(repo, card.local, card.squashBase, tip, message);
    } finally {
      this.reload();
    }
  }

  /** "Update from main": merge (or rebase) the feature's parent into it. Throws so the panel can show why. */
  async updateFromParent(card: Card, rebase: boolean) {
    const repo = this.repo;
    if (!repo || !card.parent) return;
    try {
      await api.updateBranch(repo, card.local, card.parent, rebase, this.isCurrent(card.ref));
    } finally {
      this.reload();
    }
  }

  async forcePush(branch: string) {
    const repo = this.repo;
    if (!repo) return;
    try {
      await api.forcePush(repo, branch);
    } finally {
      this.reload();
    }
  }

  discardFile(path: string) {
    this.discard((repo) => api.discardFile(repo, path));
  }

  discardAll() {
    this.discard((repo) => api.discardAll(repo));
  }

  discardHunk(index: number) {
    const path = this.fileFilter;
    if (path) this.discard((repo) => api.discardHunk(repo, path, index));
  }

  /** Stages or unstages files from the tree's checkboxes; only the tree and the dashboard counts refresh. */
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
    this.loadDashboard();
  }

  /** Commits what's staged (and pushes with `push`). Resolves to false if git refused, so the form keeps the message. */
  async commit(message: string, push: boolean) {
    let failed = false;
    await this.act(push ? "Committing and pushing…" : "Committing…", "Commit failed", async (repo) => {
      try {
        return await api.commit(repo, message, push);
      } catch (error) {
        failed = true;
        throw error;
      }
    });
    return !failed;
  }

  private async discard(work: (repo: string) => Promise<void>) {
    const repo = this.repo;
    if (!repo) return;
    try {
      await work(repo);
    } catch (error) {
      this.syncStatus = "Discard failed: " + String(error);
    }
    this.reload();
  }
}
