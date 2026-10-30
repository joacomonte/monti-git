// Typed wrappers over the Rust commands in src-tauri/src/lib.rs.
import { invoke } from "@tauri-apps/api/core";

export const WORKTREE_ID = "WORKTREE";

export type Commit = {
  id: string;
  parents: string[];
  author: string;
  date: number; // unix seconds
  refs: string[]; // raw %D decorations, e.g. "HEAD -> main", "origin/main", "tag: v1.0"
  subject: string;
  shelved?: string; // stash row: the branch whose uncommitted work Monti Git put aside when leaving it
};

/** [target, color, flags]: target = commit index (negative = past the loaded page); flags: 1 behind, 2 current. */
export type Lane = [number, number, number] | null;

export type Row = {
  column: number;
  color: number;
  before: Lane[];
  after: Lane[];
  isCurrent: boolean;
  parents: number[]; // same ids as Lane targets
};

export type History = {
  commits: Commit[];
  rows: Row[];
  names: (string | null)[];
  hasMore: boolean;
};

/** A commit whose title or description matched a global search (see search.rs). */
export type SearchHit = { repo: string; id: string; author: string; date: number; subject: string; snippet: string | null };

/** Everything about one commit (see CommitInfo in src-tauri/src/lib.rs). Dates in unix seconds. */
export type CommitInfo = {
  id: string; author: string; authorEmail: string; authorDate: number; committer: string; committerEmail: string; committerDate: number;
  parents: string[]; refs: string[]; subject: string; body: string;
};

/** Who last touched each line: `lines[n - 1]` indexes `commits` for line n (see blame in src-tauri/src/lib.rs). */
export type Blame = { lines: number[]; commits: { id: string; author: string; date: number; subject: string }[] };

export type RepoEntry = { path: string; owner: string; lastActivity: number; folder: string | null }; // folder: the added folder it came from

export type Changes = {
  changes: Record<string, string>; // path -> status letter
  counts: Record<string, [number, number]>; // path -> added, removed
  paths: string[];
  staged: string[]; // uncommitted-changes row only: files in the index
};

export type Side = { number: number; text: string; changed: [number, number] | null };
export type DiffRow = { hunk?: number; left: Side | null; right: Side | null; isChange: boolean };
export type FileDiff = { rows: DiffRow[]; touching: string[] };

export type Totals = { files: number; added: number; removed: number };

/** One branch on the dashboard (see Card in src-tauri/src/dashboard.rs). */
export type Card = {
  ref: string; // "qa", or "origin/qa" when there is no local branch
  local: string;
  summary: { date: number; ahead: number; behind: number; hasUpstream: boolean } | null; // null = remote-only
  last: Commit | null;
  promotion: [string, number][]; // target, commits not in it yet
  behindBase: number;
  parent: string | null;
  diff: Totals | null;
  uncommitted: Totals | null;
  changed: string[]; // current branch only: uncommitted file paths
  conflicts: string[]; // diverged from its upstream: files that would conflict pulling it
  worktree: [number, number, number] | null; // staged, not staged, untracked
  featureCommits: Commit[];
  squashBase: string | null;
  alreadyIn: string[];
  mergedBefore: string[]; // targets that got an earlier state of it (e.g. squash-merged) but not its newer commits
  behindParent: number;
  parentTip: string | null;
};

export type Dashboard = {
  base: Card | null;
  environments: Card[];
  pinned: Card[];
  features: Card[]; // "Recent activity"
  old: Card[]; // "Hidden"
  current: string | null;
  shelved: string[];
  local: string[]; // every local branch, for "Change base branch"
};

export type SyncResult = { updated: string[]; skipped: string[]; log: string[] };

/** What to do with uncommitted changes when checking out: stash for this branch, WIP commit, or bring along. */
export type WipMode = "shelve" | "commit" | "carry";

export const api = {
  listRepos: (folders: string[], repos: string[]) => invoke<RepoEntry[]>("list_repos", { folders, repos }),
  isRepo: (path: string) => invoke<boolean>("is_repo", { path }),
  loadHistory: (repo: string, limit: number, onlyMine: boolean, preferred: string | null, environments: string[], focus: string[], path: string | null) =>
    invoke<History>("load_history", { repo, limit, onlyMine, preferred, environments, focus, path }),
  searchCommits: (repos: string[], query: string) => invoke<SearchHit[]>("search_commits", { repos, query }),
  commitPlaces: (repo: string, id: string) => invoke<string[]>("commit_places", { repo, id }),
  commitInfo: (repo: string, id: string) => invoke<CommitInfo>("commit_info", { repo, id }),
  commitPatch: (repo: string, id: string) => invoke<string>("commit_patch", { repo, id }),
  blame: (repo: string, path: string, rev: string | null) => invoke<Blame>("blame", { repo, path, rev }),
  loadChanges: (repo: string, base: string | null, target: string | null, all: boolean) =>
    invoke<Changes>("load_changes", { repo, base, target, all }),
  loadDiff: (repo: string, path: string, base: string | null, target: string | null, group: string[], whole: boolean) =>
    invoke<FileDiff>("load_diff", { repo, path, base, target, group, whole }),
  loadDashboard: (repo: string, preferred: string | null, pinned: string[], environments: string[] | null, states: Record<string, string>) =>
    invoke<Dashboard>("load_dashboard", { repo, preferred, pinned, environments, states }),
  branchChain: (repo: string, base: string, reference: string) => invoke<string[]>("branch_chain", { repo, base, reference }),
  // Actions that change the repo (src-tauri/src/actions.rs); they reject with git's stderr.
  sync: (repo: string) => invoke<SyncResult>("sync", { repo }),
  changedFileCount: (repo: string) => invoke<number>("changed_file_count", { repo }),
  checkout: (repo: string, name: string, wip: WipMode | null) => invoke<string>("checkout", { repo, name, wip }),
  pull: (repo: string, name: string) => invoke<string>("pull", { repo, name }),
  deleteBranch: (repo: string, branch: string, remote: boolean) => invoke<string>("delete_branch", { repo, branch, remote }),
  openFile: (repo: string, path: string, reveal: boolean) => invoke<void>("open_file", { repo, path, reveal }),
  updatePrompt: (repo: string, branch: string) => invoke<string>("update_prompt", { repo, branch }),
  /** Fetches, then how `base` compares with origin/`base` (`remote` false: no copy there). */
  baseStatus: (repo: string, base: string) => invoke<{ ahead: number; behind: number; remote: boolean }>("base_status", { repo, base }),
  startFeature: (repo: string, name: string, base: string, wip: WipMode | null) =>
    invoke<string>("start_feature", { repo, name, base, wip }),
  updateBranch: (repo: string, branch: string, parent: string, rebase: boolean, isCurrent: boolean) =>
    invoke<void>("update_branch", { repo, branch, parent, rebase, isCurrent }),
  forcePush: (repo: string, branch: string) => invoke<void>("force_push", { repo, branch }),
  moveBranch: (repo: string, branch: string, expected: string | null, target: string) =>
    invoke<void>("move_branch", { repo, branch, expected, target }),
  branchTip: (repo: string, branch: string) => invoke<string | null>("branch_tip", { repo, branch }),
  discardFile: (repo: string, path: string) => invoke<void>("discard_file", { repo, path }),
  discardAll: (repo: string) => invoke<void>("discard_all", { repo }),
  discardHunk: (repo: string, path: string, index: number) => invoke<void>("discard_hunk", { repo, path, index }),
  stage: (repo: string, paths: string[]) => invoke<void>("stage", { repo, paths }),
  unstage: (repo: string, paths: string[]) => invoke<void>("unstage", { repo, paths }),
  lastCommitMessage: (repo: string) => invoke<string>("last_commit_message", { repo }),
  commit: (repo: string, message: string, push: boolean) => invoke<string>("commit", { repo, message, push }),
};
