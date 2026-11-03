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
  current: string | null; // checked-out branch, null when detached
  base: string | null; // the branch pinned to the left
  environments: string[]; // qa, develop…
  allBranches: string[]; // every branch name, local or on origin: the environments picker
  local: string[]; // every local branch, for "Change base"
  shelvedBranches: string[]; // branches with WIP the app put aside: the "With WIP" filter
  fastForwarded: boolean; // "Only this branch": reached the base by a fast-forward, no merge to draw
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

/** A branch card in "Commits collapsed" (Status in src-tauri/src/branches.rs), fetched only for the rows on screen. */
export type Card = {
  ref: string; // "qa", or "origin/qa" when there is no local branch
  local: string;
  behindBase: number;
  // target, own commits not in it yet, and how they got there when 0 ("" while missing)
  promotion: [string, number, "merged" | "fast-forwarded" | "cherry-picked" | "squashed" | ""][];
  firstParentCount: number; // commits "Only this branch" shows: matches the graph when focused
  hasUpstream: boolean;
};

export type SyncResult = { updated: string[]; skipped: string[]; log: string[]; changed: boolean };

/** One git command the app ran (src-tauri/src/git.rs `LogEntry`), for the Logs panel. */
export type GitLog = {
  id: number; at: number; repo: string; args: string[]; ok: boolean; code: number | null; ms: number;
  stdout: string; stderr: string; write: boolean; truncated: boolean;
};

/** What to do with uncommitted changes when checking out: stash for this branch, WIP commit, or bring along. */
export type WipMode = "shelve" | "commit" | "carry";

export const api = {
  listRepos: (folders: string[], repos: string[]) => invoke<RepoEntry[]>("list_repos", { folders, repos }),
  isRepo: (path: string) => invoke<boolean>("is_repo", { path }),
  loadHistory: (repo: string, limit: number, onlyMine: boolean, branchesOnly: boolean, preferred: string | null, focus: string[], path: string | null,
    environments: string[] | null, only: string[] | null) =>
    invoke<History>("load_history", { repo, limit, onlyMine, branchesOnly, preferred, focus, path, environments, only }),
  /** Filters, fast pass: every branch and, per target, the ones already in it by history (see branches.rs). */
  mergedInto: (repo: string, targets: string[], onlyMine: boolean) =>
    invoke<{ all: string[]; merged: [string, string[]][] }>("merged_into", { repo, targets, onlyMine }),
  searchCommits: (repos: string[], query: string) => invoke<SearchHit[]>("search_commits", { repos, query }),
  commitPlaces: (repo: string, id: string) => invoke<string[]>("commit_places", { repo, id }),
  commitInfo: (repo: string, id: string) => invoke<CommitInfo>("commit_info", { repo, id }),
  commitPatch: (repo: string, id: string) => invoke<string>("commit_patch", { repo, id }),
  blame: (repo: string, path: string, rev: string | null) => invoke<Blame>("blame", { repo, path, rev }),
  loadChanges: (repo: string, base: string | null, target: string | null, all: boolean) =>
    invoke<Changes>("load_changes", { repo, base, target, all }),
  /** Tree search by content: path -> matching lines. See search_lines in src-tauri/src/lib.rs. */
  searchLines: (repo: string, base: string | null, target: string | null, query: string, paths: string[] | null, diffOnly: boolean) =>
    invoke<Record<string, number>>("search_lines", { repo, base, target, query, paths, diffOnly }),
  loadDiff: (repo: string, path: string, base: string | null, target: string | null, group: string[], whole: boolean) =>
    invoke<FileDiff>("load_diff", { repo, path, base, target, group, whole }),
  branchStatus: (repo: string, refs: string[], base: string, targets: string[]) =>
    invoke<Omit<Card, "local">[]>("branch_status", { repo, refs, base, targets }),
  gitLogs: (since: number) => invoke<GitLog[]>("git_logs", { since }),
  // Actions that change the repo (src-tauri/src/actions.rs); they reject with git's stderr.
  sync: (repo: string) => invoke<SyncResult>("sync", { repo }),
  changedFileCount: (repo: string) => invoke<number>("changed_file_count", { repo }),
  checkout: (repo: string, name: string, wip: WipMode | null) => invoke<string>("checkout", { repo, name, wip }),
  pull: (repo: string, name: string) => invoke<string>("pull", { repo, name }),
  deleteBranch: (repo: string, branch: string, remote: boolean) => invoke<string>("delete_branch", { repo, branch, remote }),
  openFile: (repo: string, path: string, reveal: boolean) => invoke<void>("open_file", { repo, path, reveal }),
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
