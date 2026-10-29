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
  runs: [number, number][];
  names: (string | null)[];
  hasMore: boolean;
};

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
  loadHistory: (repo: string, limit: number, onlyMine: boolean, preferred: string | null, environments: string[]) =>
    invoke<History>("load_history", { repo, limit, onlyMine, preferred, environments }),
  loadChanges: (repo: string, base: string | null, target: string | null, all: boolean) =>
    invoke<Changes>("load_changes", { repo, base, target, all }),
  loadDiff: (repo: string, path: string, base: string | null, target: string | null, group: string[]) =>
    invoke<FileDiff>("load_diff", { repo, path, base, target, group }),
  loadDashboard: (repo: string, preferred: string | null, pinned: string[], environments: string[] | null, states: Record<string, string>) =>
    invoke<Dashboard>("load_dashboard", { repo, preferred, pinned, environments, states }),
  branchChain: (repo: string, base: string, reference: string) => invoke<string[]>("branch_chain", { repo, base, reference }),
  // Actions that change the repo (src-tauri/src/actions.rs); they reject with git's stderr.
  sync: (repo: string) => invoke<SyncResult>("sync", { repo }),
  changedFileCount: (repo: string) => invoke<number>("changed_file_count", { repo }),
  checkout: (repo: string, name: string, wip: WipMode | null) => invoke<string>("checkout", { repo, name, wip }),
  pull: (repo: string, name: string) => invoke<string>("pull", { repo, name }),
  deleteBranch: (repo: string, branch: string, remote: boolean) => invoke<string>("delete_branch", { repo, branch, remote }),
  updatePrompt: (repo: string, branch: string) => invoke<string>("update_prompt", { repo, branch }),
  squash: (repo: string, branch: string, base: string, expectedTip: string, message: string) =>
    invoke<void>("squash", { repo, branch, base, expectedTip, message }),
  updateBranch: (repo: string, branch: string, parent: string, rebase: boolean, isCurrent: boolean) =>
    invoke<void>("update_branch", { repo, branch, parent, rebase, isCurrent }),
  forcePush: (repo: string, branch: string) => invoke<void>("force_push", { repo, branch }),
  discardFile: (repo: string, path: string) => invoke<void>("discard_file", { repo, path }),
  discardAll: (repo: string) => invoke<void>("discard_all", { repo }),
  discardHunk: (repo: string, path: string, index: number) => invoke<void>("discard_hunk", { repo, path, index }),
  stage: (repo: string, paths: string[]) => invoke<void>("stage", { repo, paths }),
  unstage: (repo: string, paths: string[]) => invoke<void>("unstage", { repo, paths }),
  lastCommitMessage: (repo: string) => invoke<string>("last_commit_message", { repo }),
  commit: (repo: string, message: string, push: boolean) => invoke<string>("commit", { repo, message, push }),
};
